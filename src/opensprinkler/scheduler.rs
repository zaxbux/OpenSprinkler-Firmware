use crate::utils;

use super::{controller, events, log, program, OpenSprinkler};

pub fn do_time_keeping(open_sprinkler: &mut OpenSprinkler, now_seconds: i64) {
    // first, go through run time queue to assign queue elements to stations
    let mut qid = 0;
    for q in open_sprinkler.state.program.queue.queue.iter() {
        let station_index = q.station_index;
        if let Some(sqi) = open_sprinkler.state.program.queue.station_qid[station_index] {
            // skip if station is already assigned a queue element and that queue element has an earlier start time
            if open_sprinkler.state.program.queue.queue[sqi].start_time < q.start_time {
                continue;
            }
        }
        
        // otherwise assign the queue element to station
        open_sprinkler.state.program.queue.station_qid[station_index] = Some(qid);
        qid += 1;
    }
    // next, go through the stations and perform time keeping
    for board_index in 0..open_sprinkler.get_board_count() {
        //let bitvalue = open_sprinkler.station_bits[board_index];
        let board_active = open_sprinkler.state.station.active[board_index];
        for s in 0..controller::SHIFT_REGISTER_LINES {
            let station_index = board_index * 8 + s;

            // skip master station
            if open_sprinkler.is_master_station(station_index) {
                continue;
            }

            if let Some(qid) = open_sprinkler.state.program.queue.station_qid[station_index] {
                let q = open_sprinkler.state.program.queue.queue[qid].clone();
                // check if this station is scheduled, either running or waiting to run
                if q.start_time > 0 {
                    // if so, check if we should turn it off
                    if now_seconds >= q.start_time + q.water_time {
                        controller::turn_off_station(open_sprinkler, now_seconds, station_index);
                    }
                }
                // if current station is not running, check if we should turn it on
                if board_active[s] == false {
                    if now_seconds >= q.start_time && now_seconds < q.start_time + q.water_time {
                        controller::turn_on_station(open_sprinkler, station_index);
                    }
                }
            }
        }
    }

    // finally, go through the queue again and clear up elements marked for removal
    open_sprinkler.state.program.queue.clean_queue(now_seconds);

    // process dynamic events
    open_sprinkler.process_dynamic_events(now_seconds);

    // activate / deactivate valves
    open_sprinkler.apply_all_station_bits();

    // check through runtime queue, calculate the last stop time of sequential stations
    open_sprinkler.state.program.queue.last_seq_stop_time = None;

    for q in open_sprinkler.state.program.queue.queue.iter() {
        let station_index = q.station_index;

        // check if any sequential station has a valid stop time
        // and the stop time must be larger than curr_time
        let sequential_stop_time = (q.start_time + q.water_time) as i64;
        if sequential_stop_time > now_seconds {
            // only need to update last_seq_stop_time for sequential stations
            if open_sprinkler.config.stations[station_index].attrib.is_sequential && !open_sprinkler.is_remote_extension() {
                open_sprinkler.state.program.queue.last_seq_stop_time = if sequential_stop_time > open_sprinkler.state.program.queue.last_seq_stop_time.unwrap() {
                    Some(sequential_stop_time)
                } else {
                    open_sprinkler.state.program.queue.last_seq_stop_time
                };
            }
        }
    }

    // if the runtime queue is empty, reset all stations
    if open_sprinkler.state.program.queue.queue.is_empty() {
        // turn off all stations
        open_sprinkler.clear_all_station_bits();
        open_sprinkler.apply_all_station_bits();
        // reset runtime
        open_sprinkler.state.program.queue.reset_runtime();
        // reset program busy bit
        open_sprinkler.state.program.busy = false;
        // log flow sensor reading if flow sensor is used
        if open_sprinkler.is_flow_sensor_enabled() {
            let _ = log::write_log_message(&open_sprinkler, &log::message::FlowSenseMessage::new(open_sprinkler.get_flow_log_count(), now_seconds), now_seconds);
            events::push_message(
                &open_sprinkler,
                &events::FlowSensorEvent::new(
                    open_sprinkler.get_flow_log_count(),
                    open_sprinkler.get_flow_pulse_rate(),
                ),
            );
        }
    }
}

pub fn check_program_schedule(open_sprinkler: &mut OpenSprinkler, now_seconds: i64) {
    tracing::trace!("Checking program schedule");
    let mut match_found = false;

    // check through all programs
    let programs = open_sprinkler.config.programs.clone();
    for (program_index, program) in programs.iter().enumerate() {

        if program.check_match(&open_sprinkler, now_seconds) {
            // program match found
            // check and process special program command
            if open_sprinkler.process_special_program_command(now_seconds, &program.name) {
                continue;
            }

            let water_scale = if program.use_weather {
                open_sprinkler.config.water_scale
            } else {
                1.0
            };

            // process all selected stations
            for station_index in 0..open_sprinkler.get_station_count() {

                // skip if the station is a master station (because master cannot be scheduled independently
                if open_sprinkler.is_master_station(station_index) {
                    continue;
                }

                // if station has non-zero water time and the station is not disabled
                if program.durations[station_index] > 0 && !open_sprinkler.config.stations[station_index].attrib.is_disabled {
                    // water time is scaled by watering percentage
                    let mut water_time = water_scale * utils::water_time_resolve(program.durations[station_index], open_sprinkler.get_sunrise_time(), open_sprinkler.get_sunset_time());
                    // if the program is set to use weather scaling
                    if program.use_weather {
                        if water_scale < 0.2 && water_time < 10.0 {
                            // if water_percentage is less than 20% and water_time is less than 10 seconds, do not water
                            water_time = 0.0;
                        }
                    }

                    if water_time > 0.0 {
                        // check if water time is still valid
                        // because it may end up being zero after scaling
                        let q = open_sprinkler.state.program.queue.enqueue(program::QueueElement::new(
                            0,
                            water_time as i64,
                            station_index,
                            program::ProgramStart::User(program_index),
                        ));
                        if q.is_ok() {
                            match_found = true;
                        } else {
                            // queue is full
                        }
                    }
                }
            }
            if match_found {
                let program_name = program.name.clone();
                tracing::trace!("Program {{id = {}, name = {}}} scheduled", program_index, program_name);
                events::push_message(
                    &open_sprinkler,
                    &events::ProgramStartEvent::new(program_index, program_name, water_scale),
                );
            }
        }
    }

    // calculate start and end time
    if match_found {
        schedule_all_stations(open_sprinkler, now_seconds as i64);
    }
}

/// Scheduler
///
/// This function loops through the queue and schedules the start time of each station
fn schedule_all_stations(open_sprinkler: &mut OpenSprinkler, now_seconds: i64) {
    tracing::trace!("Scheduling all stations");
    let mut con_start_time = now_seconds + 1; // concurrent start time
    let mut seq_start_time = con_start_time; // sequential start time

    let station_delay: i64 = utils::water_time_decode_signed(open_sprinkler.config.station_delay_time).into();

    // if the sequential queue has stations running
    if open_sprinkler.state.program.queue.last_seq_stop_time.unwrap_or(0) > now_seconds {
        seq_start_time = open_sprinkler.state.program.queue.last_seq_stop_time.unwrap_or(0) + station_delay;
    }

    for qi in 0..open_sprinkler.state.program.queue.queue.len() {
        let mut q = &mut open_sprinkler.state.program.queue.queue[qi];

        if q.start_time > 0 {
            // if this queue element has already been scheduled, skip
            continue;
        }
        if q.water_time == 0 {
            continue; // if the element has been marked to reset, skip
        }

        let station_index = q.station_index;

        // if this is a sequential station and the controller is not in remote extension mode, use sequential scheduling. station delay time apples
        if open_sprinkler.config.stations[station_index].attrib.is_sequential && open_sprinkler.config.enable_remote_ext_mode /* !open_sprinkler.is_remote_extension() */ {
            // sequential scheduling
            q.start_time = seq_start_time;
            seq_start_time += q.water_time;
            seq_start_time += station_delay; // add station delay time
        } else {
            // otherwise, concurrent scheduling
            q.start_time = con_start_time;
            // stagger concurrent stations by 1 second
            con_start_time += 1;
        }

        if !open_sprinkler.state.program.busy {
            open_sprinkler.state.program.busy = true;

            // start flow count
            if open_sprinkler.is_flow_sensor_enabled() {
                // if flow sensor is connected
                open_sprinkler.start_flow_log_count();
                open_sprinkler.state.sensor.set_timestamp_activated(0, Some(now_seconds));
            }
        }
    }
}

/// Manually start a program
pub fn manual_start_program(open_sprinkler: &mut OpenSprinkler, program_start: program::ProgramStart, use_water_scale: bool) {
    let mut match_found = false;
    open_sprinkler.reset_all_stations_immediate();

    let program = match program_start {
        program::ProgramStart::Test => program::Program::test_program(60),
        program::ProgramStart::TestShort => program::Program::test_program(2),
        program::ProgramStart::RunOnce => todo!(),
        program::ProgramStart::User(index) => open_sprinkler.config.programs[index].clone(),
    };

    let water_scale = if use_water_scale {
        open_sprinkler.config.water_scale
    } else {
        1.0
    };

    if let program::ProgramStart::User(index) = program_start {
        events::push_message(open_sprinkler, &events::ProgramStartEvent::new(index, program.name,  water_scale));
    }

    for station_index in 0..open_sprinkler.get_station_count() {
        // skip if the station is a master station (because master cannot be scheduled independently
        if open_sprinkler.is_master_station(station_index) {
            continue;
        }

        let water_time = water_scale * match program_start {
            program::ProgramStart::Test => 60.0,
            program::ProgramStart::TestShort => 2.0,
            program::ProgramStart::RunOnce => todo!(),
            program::ProgramStart::User(_) => utils::water_time_resolve(program.durations[station_index], open_sprinkler.get_sunrise_time(), open_sprinkler.get_sunset_time()),
        };

        //water_time = water_time * water_scale;

        if water_time > 0.0 && !open_sprinkler.config.stations.get(station_index).unwrap().attrib.is_disabled {
            if open_sprinkler.state.program.queue
                .enqueue(program::QueueElement::new(
                    0,
                    water_time as i64,
                    station_index,
                    program::ProgramStart::Test,
                ))
                .is_ok()
            {
                match_found = true;
            }
        }
    }
    if match_found {
        schedule_all_stations(open_sprinkler, chrono::Utc::now().timestamp());
    }
}
