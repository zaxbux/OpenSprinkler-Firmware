use crate::{utils::{water_time_decode_signed, water_time_resolve}, opensprinkler::events};

use super::{
    demo,
    log::{self, LogDataType},
    program::{Program, RuntimeQueueStruct, MANUAL_PROGRAM_ID, TEST_PROGRAM_ID},
    sensor::MAX_SENSORS,
    OpenSprinkler, SensorType, REBOOT_DELAY, SHIFT_REGISTER_LINES, controller,
};

use super::program::ProgramData;

//pub fn flow_poll(open_sprinkler: &OpenSprinkler, flow_state: &mut FlowSensor) {
pub fn flow_poll(open_sprinkler: &mut OpenSprinkler) {
    #[cfg(not(feature = "demo"))]
    let sensor1_pin = open_sprinkler.gpio.get(super::gpio::pin::SENSOR_1).and_then(|pin| Ok(pin.into_input()));
    #[cfg(feature = "demo")]
    let sensor1_pin = demo::get_gpio_pin(super::gpio::pin::SENSOR_1);
    if let Err(ref error) = sensor1_pin {
        tracing::error!("Failed to obtain sensor input pin (flow): {:?}", error);
        return;
    } else if sensor1_pin.is_ok() {
        // Perform calculations using the current state of the sensor
        open_sprinkler.flow_state.poll(sensor1_pin.unwrap().read());
    }
}

pub fn check_rain_delay(open_sprinkler: &mut OpenSprinkler, now_seconds: i64) {
    if open_sprinkler.status.rain_delayed {
        //if now_seconds >= open_sprinkler.nvdata.rd_stop_time.unwrap_or(0) {
        if now_seconds >= open_sprinkler.controller_config.nv.rd_stop_time.unwrap_or(0) {
            // rain delay is over
            open_sprinkler.rain_delay_stop();
        }
    } else {
        //if open_sprinkler.nvdata.rd_stop_time.unwrap_or(0) > now_seconds {
        if open_sprinkler.controller_config.nv.rd_stop_time.unwrap_or(0) > now_seconds {
            // rain delay starts now
            open_sprinkler.rain_delay_start();
        }
    }

    // Check controller status changes and write log
    if open_sprinkler.old_status.rain_delayed != open_sprinkler.status.rain_delayed {
        if open_sprinkler.status.rain_delayed {
            // rain delay started, record time
            open_sprinkler.raindelay_on_last_time = now_seconds.try_into().unwrap();
            /* push_message(&open_sprinkler, NotifyEvent::RainDelay, RainDelay::new(true)); */
        } else {
            // rain delay stopped, write log
            let _ = log::write_log_message(&open_sprinkler, &log::message::SensorMessage::new(LogDataType::RainDelay, now_seconds), now_seconds);
            /* push_message(&open_sprinkler, NotifyEvent::RainDelay, RainDelay::new(false)); */
        }
        events::push_message(&open_sprinkler, &events::RainDelayEvent::new(true));
        open_sprinkler.old_status.rain_delayed = open_sprinkler.status.rain_delayed;
    }
}

pub fn check_binary_sensor_status(open_sprinkler: &mut OpenSprinkler, now_seconds: i64) {
    open_sprinkler.detect_binary_sensor_status(now_seconds);

    if open_sprinkler.old_status.sensors[0].active != open_sprinkler.status.sensors[0].active {
        // send notification when sensor becomes active
        if open_sprinkler.status.sensors[0].active {
            open_sprinkler.sensor_status[0].active_last_time = Some(now_seconds);
        } else {
            let _ = log::write_log_message(&open_sprinkler, &log::message::SensorMessage::new(log::LogDataType::Sensor1, now_seconds), now_seconds);
        }
        events::push_message(&open_sprinkler, &events::BinarySensorEvent::new(0, open_sprinkler.status.sensors[0].active));
    }
    open_sprinkler.old_status.sensors[0].active = open_sprinkler.status.sensors[0].active;
}

//pub fn check_program_switch_status(open_sprinkler: &mut OpenSprinkler, flow_state: &mut FlowSensor, program_data: &mut ProgramData) {
pub fn check_program_switch_status(open_sprinkler: &mut OpenSprinkler, program_data: &mut ProgramData) {
    let program_switch = open_sprinkler.detect_program_switch_status();
    if program_switch[0] == true || program_switch[1] == true {
        reset_all_stations_immediate(open_sprinkler, program_data); // immediately stop all stations
    }

    for i in 0..MAX_SENSORS {
        //if program_data.nprograms > i {
        if open_sprinkler.controller_config.programs.len() > i {
            //manual_start_program(open_sprinkler, flow_state, program_data, i + 1, false);
            manual_start_program(open_sprinkler, program_data, i + 1, false);
        }
    }
}

/// Process dynamic events
///
/// Processes events such as: Rain delay, rain sensing, station state changes, etc.
//pub fn process_dynamic_events(open_sprinkler: &mut OpenSprinkler, program_data: &mut ProgramData, flow_state: &mut FlowSensor, now_seconds: i64) {
pub fn process_dynamic_events(open_sprinkler: &mut OpenSprinkler, program_data: &mut ProgramData, now_seconds: i64) {
    // Check if rain is detected
    /* let sn1 = if (open_sprinkler.iopts.sn1t == SensorType::Rain as u8 || open_sprinkler.iopts.sn1t == SensorType::Soil as u8) && open_sprinkler.status.sensors[0].active {
        true
    } else {
        false
    }; */
    let sn1 = (open_sprinkler.get_sensor_type(0) == SensorType::Rain || open_sprinkler.get_sensor_type(0) == SensorType::Soil) && open_sprinkler.status.sensors[0].active;

    /* let sn2 = if (open_sprinkler.iopts.sn2t == SensorType::Rain as u8 || open_sprinkler.iopts.sn2t == SensorType::Soil as u8) && open_sprinkler.status.sensors[1].active {
        true
    } else {
        false
    }; */
    let sn2 = (open_sprinkler.get_sensor_type(1) == SensorType::Rain || open_sprinkler.get_sensor_type(1) == SensorType::Soil) && open_sprinkler.status.sensors[1].active;

    //let rd = open_sprinkler.status.rain_delayed;
    //let en = open_sprinkler.status.enabled;

    for board_id in 0..open_sprinkler.get_board_count() {
        //let igs1 = open_sprinkler.attrib_igs[board_id];
        //let igs2 = open_sprinkler.attrib_igs2[board_id];
        //let igrd = open_sprinkler.attrib_igrd[board_id];

        for s in 0..SHIFT_REGISTER_LINES {
            let station_index = board_id * SHIFT_REGISTER_LINES + s;

            // Ignore master stations because they are handles separately
            if (open_sprinkler.status.mas.unwrap_or(0) == station_index + 1) || (open_sprinkler.status.mas2.unwrap_or(0) == station_index + 1) {
                continue;
            }

            // If this is a normal program (not a run-once or test program)
            // and either the controller is disabled, or
            // if raining and ignore rain bit is cleared
            // @FIXME
            let qid = program_data.station_qid[station_index];
            if qid == 255 {
                continue;
            }

            let q = program_data.queue.get(qid).unwrap();

            if q.pid >= TEST_PROGRAM_ID {
                // This is a manually started program, skip
                continue;
            }

            // If system is disabled, turn off zone
            if !open_sprinkler.status.enabled {
                //turn_off_station(open_sprinkler, flow_state, program_data, now_seconds, station_index);
                controller::turn_off_station(open_sprinkler, program_data, now_seconds, station_index);
            }

            // if rain delay is on and zone does not ignore rain delay, turn it off
            //if rd && !(igrd & (1 << s)) {
            //if rd && !open_sprinkler.stations[station_index].attrib.igrd {
            if open_sprinkler.status.rain_delayed && !open_sprinkler.controller_config.stations[station_index].attrib.igrd {
                //turn_off_station(open_sprinkler, flow_state, program_data, now_seconds, station_index);
                controller::turn_off_station(open_sprinkler, program_data, now_seconds, station_index);
            }

            // if sensor1 is on and zone does not ignore sensor1, turn it off
            //if sn1 && !(igs1 & (1 << s)) {
            //if sn1 && !open_sprinkler.stations[station_index].attrib.igs {
            if sn1 && !open_sprinkler.controller_config.stations[station_index].attrib.igs {
                //turn_off_station(open_sprinkler, flow_state, program_data, now_seconds, station_index);
                controller::turn_off_station(open_sprinkler, program_data, now_seconds, station_index);
            }

            // if sensor2 is on and zone does not ignore sensor2, turn it off
            //if sn2 && !(igs2 & (1 << s)) {
            //if sn2 && !open_sprinkler.stations[station_index].attrib.igs2 {
            if sn2 && !open_sprinkler.controller_config.stations[station_index].attrib.igs2 {
                //turn_off_station(open_sprinkler, flow_state, program_data, now_seconds, station_index);
                controller::turn_off_station(open_sprinkler, program_data, now_seconds, station_index);
            }
        }
    }
}

const SPECIAL_CMD_REBOOT: &'static str = ":>reboot";
const SPECIAL_CMD_REBOOT_NOW: &'static str = ":>reboot_now";

/// Check and process special program command
fn process_special_program_command(open_sprinkler: &mut OpenSprinkler, now_seconds: i64, program_name: &String) -> bool {
    if !program_name.starts_with(':') {
        return false;
    }

    if program_name == SPECIAL_CMD_REBOOT_NOW || program_name == SPECIAL_CMD_REBOOT {
        // reboot regardless of program status
        open_sprinkler.status.safe_reboot = match program_name.as_str() {
            SPECIAL_CMD_REBOOT_NOW => false,
            SPECIAL_CMD_REBOOT => true,
            _ => true,
        };
        // set a timer to reboot in 65 seconds
        open_sprinkler.status.reboot_timer = now_seconds + REBOOT_DELAY;
        // this is to avoid the same command being executed again right after reboot
        return true;
    }

    false
}

/// Scheduler
///
/// This function loops through the queue and schedules the start time of each station
//pub fn schedule_all_stations(open_sprinkler: &mut OpenSprinkler, flow_state: &mut FlowSensor, program_data: &mut ProgramData, now_seconds: i64) {
fn schedule_all_stations(open_sprinkler: &mut OpenSprinkler, program_data: &mut ProgramData, now_seconds: i64) {
    tracing::trace!("Scheduling all stations");
    let mut con_start_time = now_seconds + 1; // concurrent start time
    let mut seq_start_time = con_start_time; // sequential start time

    //let station_delay: i64 = water_time_decode_signed(open_sprinkler.iopts.sdt).into();
    let station_delay: i64 = water_time_decode_signed(open_sprinkler.controller_config.iopts.sdt).into();

    // if the sequential queue has stations running
    if program_data.last_seq_stop_time.unwrap_or(0) > now_seconds {
        seq_start_time = program_data.last_seq_stop_time.unwrap_or(0) + station_delay;
    }

    //let q: &RuntimeQueueStruct = program_data.queue;
    //let re = open_sprinkler.iopts.re == 1;

    for q in program_data.queue.iter_mut() {
        if q.start_time > 0 {
            // if this queue element has already been scheduled, skip
            continue;
        }
        if q.water_time == 0 {
            continue; // if the element has been marked to reset, skip
        }

        let station_index = q.sid;
        //let bid = sid >> 3;
        //let s = sid & 0x07;

        // if this is a sequential station and the controller is not in remote extension mode
        // use sequential scheduling. station delay time apples
        //if (open_sprinkler.attrib_seq[bid] & (1 << s) !=0) && !re {
        //if open_sprinkler.stations[station_index].attrib.seq && !re {
        if open_sprinkler.controller_config.stations[station_index].attrib.seq && !open_sprinkler.is_remote_extension() {
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

        if !open_sprinkler.status.program_busy {
            open_sprinkler.status.program_busy = true; // set program busy bit

            // start flow count
            if open_sprinkler.get_sensor_type(0) == SensorType::Flow {
                // if flow sensor is connected
                //open_sprinkler.flow_count_log_start = flow_state.flow_count;
                //open_sprinkler.flow_count_log_start = open_sprinkler.flow_state.get_flow_count();
                open_sprinkler.start_flow_log_count();
                open_sprinkler.sensor_status[0].active_last_time = Some(now_seconds);
            }
        }
    }
}

/// Immediately reset all stations
///
/// No log records will be written
fn reset_all_stations_immediate(open_sprinkler: &mut OpenSprinkler, program_data: &mut ProgramData) {
    open_sprinkler.clear_all_station_bits();
    open_sprinkler.apply_all_station_bits();
    program_data.reset_runtime();
}

/// Reset all stations
///
/// This function sets the duration of every station to 0, which causes all stations to turn off in the next processing cycle.
/// Stations will be logged
/// @todo Move into [ProgramData]
fn reset_all_stations(program_data: &mut ProgramData) {
    // go through runtime queue and assign water time to 0
    for q in program_data.queue.iter_mut() {
        q.water_time = 0;
    }
}

/// Manually start a program
///
/// - If `pid == 0`,	this is a test program (1 minute per station)
/// - If `pid == 255`,	this is a short test program (2 second per station)
/// - If `pid > 0`,		run program `pid - 1`
//fn manual_start_program(open_sprinkler: &mut OpenSprinkler, flow_state: &mut FlowSensor, program_data: &mut ProgramData, pid: usize, uwt: bool) {
fn manual_start_program(open_sprinkler: &mut OpenSprinkler, program_data: &mut ProgramData, pid: usize, uwt: bool) {
    let mut match_found = false;
    reset_all_stations_immediate(open_sprinkler, program_data);
    //let sid: u8;
    //let bid: usize;
    //let s: usize;

    //prog = program_data.read(pid - 1).unwrap();
    let program = match pid {
        0 => Program::test_program(60),
        255 => Program::test_program(2),
        _ => open_sprinkler.controller_config.programs[pid - 1].clone(),
    };

    if pid > 0 && pid < 255 {
        //events::push_message(open_sprinkler, &events::ProgramSchedEvent::new(pid - 1, prog.name, !uwt, if uwt { open_sprinkler.iopts.wl } else { 100 }));
        events::push_message(open_sprinkler, &events::ProgramSchedEvent::new(pid - 1, &program.name, !uwt, if uwt { open_sprinkler.controller_config.iopts.wl } else { 100 }));
    }

    for station_index in 0..open_sprinkler.get_station_count() {
        // bid = sid >> 3;
        // s = sid & 0x07;
        // skip if the station is a master station (because master cannot be scheduled independently
        if (open_sprinkler.status.mas.unwrap_or(0) == station_index + 1) || (open_sprinkler.status.mas2.unwrap_or(0) == station_index + 1) {
            continue;
        }
        let mut water_time = 60;
        if pid == MANUAL_PROGRAM_ID + 1 {
            water_time = 2;
        } else if pid > 0 {
            water_time = water_time_resolve(program.durations[station_index], open_sprinkler.get_sunrise_time(), open_sprinkler.get_sunset_time());
        }
        if uwt {
            //water_time = water_time * (i64::try_from(open_sprinkler.iopts.wl).unwrap() / 100);
            water_time = water_time * (i64::try_from(open_sprinkler.controller_config.iopts.wl).unwrap() / 100);
        }
        //if water_time > 0 && !(open_sprinkler.attrib_dis[bid] & (1 << s)) {
        //if water_time > 0 && !open_sprinkler.stations[sid].attrib.dis {
        if water_time > 0 && !open_sprinkler.controller_config.stations.get(station_index).unwrap().attrib.dis {
            if program_data
                .enqueue(RuntimeQueueStruct {
                    start_time: 0,
                    water_time,
                    sid: station_index,
                    pid: 254,
                })
                .is_ok()
            {
                match_found = true;
            }
        }
    }
    if match_found {
        //let now: i64 = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs() as i64;
        //schedule_all_stations(open_sprinkler, flow_state, program_data, chrono::Utc::now().timestamp());
        schedule_all_stations(open_sprinkler, program_data, chrono::Utc::now().timestamp());
    }
}

pub fn check_program_schedule(open_sprinkler: &mut OpenSprinkler, program_data: &mut ProgramData, now_seconds: i64) {
    tracing::trace!("Checking program schedule");
    let mut match_found = false;

    // check through all programs
    //for program_index in 0..program_data.nprograms {
    let programs = open_sprinkler.controller_config.programs.clone();
    for (program_index, program) in programs.iter().enumerate() {
        //let program = program_data.read(program_index).unwrap();
        //let program = open_sprinkler.controller_config.programs.get(program_index).unwrap();

        if program.check_match(&open_sprinkler, now_seconds) {
            // program match found
            // check and process special program command
            if process_special_program_command(open_sprinkler, now_seconds, &program.name) {
                continue;
            }

            // process all selected stations
            for station_index in 0..open_sprinkler.get_station_count() {
                //let bid = station_index >> 3; // or `station_index / 8`
                //let s = station_index & 0x07; // 0..7

                // skip if the station is a master station (because master cannot be scheduled independently
                if (open_sprinkler.status.mas.unwrap_or(0) == station_index + 1) || (open_sprinkler.status.mas2.unwrap_or(0) == station_index + 1) {
                    continue;
                }

                // if station has non-zero water time and the station is not disabled
                //if program.durations[station_index] > 0 && !open_sprinkler.stations[station_index].attrib.dis {
                if program.durations[station_index] > 0 && !open_sprinkler.controller_config.stations[station_index].attrib.dis {
                    //if program.durations[station_index] > 0 && !(open_sprinkler.attrib_dis[bid] & (1 << s)) {
                    // water time is scaled by watering percentage
                    let mut water_time = water_time_resolve(program.durations[station_index], open_sprinkler.get_sunrise_time(), open_sprinkler.get_sunset_time());
                    // if the program is set to use weather scaling
                    if program.use_weather != 0 {
                        //let wl = open_sprinkler.iopts.wl;
                        let wl = open_sprinkler.controller_config.iopts.wl;
                        water_time = water_time * i64::from(wl) / 100;
                        if wl < 20 && water_time < 10 {
                            // if water_percentage is less than 20% and water_time is less than 10 seconds
                            // do not water
                            water_time = 0;
                        }
                    }

                    if water_time > 0 {
                        // check if water time is still valid
                        // because it may end up being zero after scaling
                        let q = program_data.enqueue(RuntimeQueueStruct {
                            start_time: 0,
                            water_time,
                            sid: station_index,
                            pid: program_index + 1,
                        });
                        if q.is_ok() {
                            match_found = true;
                        } else {
                            // queue is full
                        }
                    }
                }
            }
            if match_found {
                tracing::trace!("Program {{id = {}, name = {}}} scheduled", program_index, program.name);
                events::push_message(
                    &open_sprinkler,
                    &events::ProgramSchedEvent::new(program_index, &program.name, program.use_weather == 0, if program.use_weather != 0 { open_sprinkler.controller_config.iopts.wl } else { 100 }),
                );
            }
        }
    }

    // calculate start and end time
    if match_found {
        //loop_fns::schedule_all_stations(&mut open_sprinkler, &mut flow_state, &mut program_data, now_seconds as i64);
        schedule_all_stations(open_sprinkler, program_data, now_seconds as i64);
    }
}
