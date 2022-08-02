use std::time::SystemTime;

use crate::{
    utils::{water_time_decode_signed, water_time_resolve},
    FlowSensor,
};

use super::{
    events,
    log::{self, message::StationMessage, LogDataType},
    program::{Program, RuntimeQueueStruct, MANUAL_PROGRAM_ID, TEST_PROGRAM_ID},
    sensor::MAX_SENSORS,
    OpenSprinkler, SensorType, StationBitChange, REBOOT_DELAY, SHIFT_REGISTER_LINES,
};

use super::program::ProgramData;

pub fn flow_poll(open_sprinkler: &OpenSprinkler, flow_state: &mut FlowSensor) {
    #[cfg(not(feature = "demo"))]
    {
        let sensor1_pin = open_sprinkler.gpio.get(super::gpio::pin::SENSOR_1).and_then(|pin| Ok(pin.into_input()));
        if let Err(ref error) = sensor1_pin {
            tracing::error!("Failed to obtain sensor input pin (flow): {:?}", error);
            return;
        }

        let curr_flow_state = sensor1_pin.unwrap().read();
    }
    
    #[cfg(feature = "demo")]
    let curr_flow_state = rppal::gpio::Level::Low;

    if !(flow_state.prev_flow_state.is_some() && flow_state.prev_flow_state.unwrap() == rppal::gpio::Level::High && curr_flow_state == rppal::gpio::Level::Low) {
        // only record on falling edge
        flow_state.prev_flow_state = Some(curr_flow_state);
        return;
    }
    flow_state.prev_flow_state = Some(curr_flow_state);
    let curr: u32 = SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis().try_into().unwrap();
    flow_state.flow_count += 1;

    /* RAH implementation of flow sensor */
    if flow_state.flow_start == 0 {
        flow_state.flow_gallons = 0;
        flow_state.flow_start = curr;
    } // if first pulse, record time
    if (curr - flow_state.flow_start) < 90000 {
        flow_state.flow_gallons = 0;
    }
    // wait 90 seconds before recording flow_begin
    else {
        if flow_state.flow_gallons == 1 {
            flow_state.flow_begin = curr;
        }
    }
    flow_state.flow_stop = curr; // get time in ms for stop
    flow_state.flow_gallons += 1; // increment gallon count for each poll
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

pub fn check_program_switch_status(open_sprinkler: &mut OpenSprinkler, flow_state: &mut FlowSensor, program_data: &mut ProgramData) {
    let program_switch = open_sprinkler.detect_program_switch_status();
    if program_switch[0] == true || program_switch[1] == true {
        reset_all_stations_immediate(open_sprinkler, program_data); // immediately stop all stations
    }

    for i in 0..MAX_SENSORS {
        if program_data.nprograms > i {
            manual_start_program(open_sprinkler, flow_state, program_data, i + 1, false);
        }
    }
}

/// Process dynamic events
///
/// Processes events such as: Rain delay, rain sensing, station state changes, etc.
pub fn process_dynamic_events(open_sprinkler: &mut OpenSprinkler, program_data: &mut ProgramData, flow_state: &mut FlowSensor, now_seconds: i64) {
    // Check if rain is detected
    /* let sn1 = if (open_sprinkler.iopts.sn1t == SensorType::Rain as u8 || open_sprinkler.iopts.sn1t == SensorType::Soil as u8) && open_sprinkler.status.sensors[0].active {
        true
    } else {
        false
    }; */
    let sn1 = (open_sprinkler.get_sensor_type(0) == SensorType::Rain || open_sprinkler.get_sensor_type(0)== SensorType::Soil) && open_sprinkler.status.sensors[0].active;

    /* let sn2 = if (open_sprinkler.iopts.sn2t == SensorType::Rain as u8 || open_sprinkler.iopts.sn2t == SensorType::Soil as u8) && open_sprinkler.status.sensors[1].active {
        true
    } else {
        false
    }; */
    let sn2 = (open_sprinkler.get_sensor_type(1) == SensorType::Rain || open_sprinkler.get_sensor_type(1)== SensorType::Soil) && open_sprinkler.status.sensors[1].active;
    
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
                turn_off_station(open_sprinkler, flow_state, program_data, now_seconds, station_index);
            }

            // if rain delay is on and zone does not ignore rain delay, turn it off
            //if rd && !(igrd & (1 << s)) {
            //if rd && !open_sprinkler.stations[station_index].attrib.igrd {
            if open_sprinkler.status.rain_delayed && !open_sprinkler.controller_config.stations[station_index].attrib.igrd {
                turn_off_station(open_sprinkler, flow_state, program_data, now_seconds, station_index);
            }

            // if sensor1 is on and zone does not ignore sensor1, turn it off
            //if sn1 && !(igs1 & (1 << s)) {
            //if sn1 && !open_sprinkler.stations[station_index].attrib.igs {
            if sn1 && !open_sprinkler.controller_config.stations[station_index].attrib.igs {
                turn_off_station(open_sprinkler, flow_state, program_data, now_seconds, station_index);
            }

            // if sensor2 is on and zone does not ignore sensor2, turn it off
            //if sn2 && !(igs2 & (1 << s)) {
            //if sn2 && !open_sprinkler.stations[station_index].attrib.igs2 {
            if sn2 && !open_sprinkler.controller_config.stations[station_index].attrib.igs2 {
                turn_off_station(open_sprinkler, flow_state, program_data, now_seconds, station_index);
            }
        }
    }
}

const SPECIAL_CMD_REBOOT: &'static str = ":>reboot";
const SPECIAL_CMD_REBOOT_NOW: &'static str = ":>reboot_now";

/// Check and process special program command
pub fn process_special_program_command(open_sprinkler: &mut OpenSprinkler, now_seconds: i64, program_name: &String) -> bool {
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

/// Turn on a station
pub fn turn_on_station(open_sprinkler: &mut OpenSprinkler, flow_state: &mut FlowSensor, station_id: usize) {
    // RAH implementation of flow sensor
    flow_state.flow_start = 0;

    if open_sprinkler.set_station_bit(station_id, true) == StationBitChange::On {
        //let station_name = open_sprinkler.stations.get(station_id).unwrap().name.to_string();
        let station_name = open_sprinkler.controller_config.stations.get(station_id).unwrap().name.to_string();
        //let station_name = open_sprinkler.get_station_name(station_id).as_str();
        events::push_message(
            open_sprinkler,
            &events::StationEvent {
                station_id,
                station_name,
                state: true,
                duration: None,
                flow: None,
            },
        );
    }
}

/// Turn off a station
///
/// Turns off a scheduled station, writes a log record, and pushes a notification event.
///
/// @todo Make member of [OpenSprinkler]
pub fn turn_off_station(open_sprinkler: &mut OpenSprinkler, flow_state: &mut FlowSensor, program_data: &mut ProgramData, now_seconds: i64, station_id: usize) {
    open_sprinkler.set_station_bit(station_id, false);

    let qid = program_data.station_qid[station_id];

    // ignore if we are turning off a station that is not running or is not scheduled to run
    if qid >= program_data.queue.len() {
        return;
    }

    // RAH implementation of flow sensor
    if flow_state.flow_gallons > 1 {
        // RAH calculate GPM, 1 pulse per gallon

        if flow_state.flow_stop <= flow_state.flow_begin {
            flow_state.flow_last_gpm = 0.0;
        } else {
            flow_state.flow_last_gpm = (60000 / ((flow_state.flow_stop - flow_state.flow_begin) / (flow_state.flow_gallons - 1))) as f32;
        }
    } else {
        // RAH if not one gallon (two pulses) measured then record 0 gpm
        flow_state.flow_last_gpm = 0.0;
    }

    let q = program_data.queue.get(qid).unwrap();

    // check if the current time is past the scheduled start time,
    // because we may be turning off a station that hasn't started yet
    if now_seconds > q.start_time.into() {
        // record lastrun log (only for non-master stations)
        if (open_sprinkler.status.mas.unwrap_or(0) != station_id + 1) && (open_sprinkler.status.mas2.unwrap_or(0) != station_id + 1) {
            let duration = u16::try_from(now_seconds - q.start_time).unwrap();

            // log station run
            let mut message = StationMessage::new(
                q.pid,
                station_id,
                duration, // @fixme Maximum duration is 18 hours (64800 seconds), which fits into a [u16]
                now_seconds,
            );

            // Keep a copy for web
            program_data.last_run = Some(message);

            //if open_sprinkler.iopts.sn1t == SensorType::Flow as u8 {
            if open_sprinkler.get_sensor_type(0) == SensorType::Flow {
                message.with_flow(flow_state.flow_last_gpm);
            }
            let _ = log::write_log_message(open_sprinkler, &message, now_seconds);

            //let station_name = open_sprinkler.stations[station_id].name.clone();
            let station_name = open_sprinkler.controller_config.stations.get(station_id).unwrap().name;
            events::push_message(
                open_sprinkler,
                &events::StationEvent::new(
                    station_id,
                    station_name,
                    false,
                    Some(duration.into()),
                    //if open_sprinkler.iopts.sn1t == SensorType::Flow as u8 { Some(flow_state.flow_last_gpm) } else { None },
                    if open_sprinkler.get_sensor_type(0) == SensorType::Flow { Some(flow_state.flow_last_gpm) } else { None },
                ),
            );
        }
    }

    // dequeue the element
    program_data.dequeue(qid);
    program_data.station_qid[station_id] = 0xFF;
}

/// Scheduler
///
/// This function loops through the queue and schedules the start time of each station
pub fn schedule_all_stations(open_sprinkler: &mut OpenSprinkler, flow_state: &mut FlowSensor, program_data: &mut ProgramData, now_seconds: i64) {
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
                open_sprinkler.flow_count_log_start = flow_state.flow_count;
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
fn manual_start_program(open_sprinkler: &mut OpenSprinkler, flow_state: &mut FlowSensor, program_data: &mut ProgramData, pid: usize, uwt: bool) {
    let mut match_found = false;
    reset_all_stations_immediate(open_sprinkler, program_data);
    let prog: Program;
    //let sid: u8;
    //let bid: usize;
    //let s: usize;

    if pid > 0 && pid < 255 {
        prog = program_data.read(pid - 1).unwrap();
        //events::push_message(open_sprinkler, &events::ProgramSchedEvent::new(pid - 1, prog.name, !uwt, if uwt { open_sprinkler.iopts.wl } else { 100 }));
        events::push_message(open_sprinkler, &events::ProgramSchedEvent::new(pid - 1, prog.name, !uwt, if uwt { open_sprinkler.controller_config.iopts.wl } else { 100 }));
    } else if pid == 255 {
        prog = Program::test_program(2);
    } else {
        prog = Program::test_program(60);
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
            water_time = water_time_resolve(prog.durations[station_index], open_sprinkler.get_sunrise_time(), open_sprinkler.get_sunset_time());
        }
        if uwt {
            //water_time = water_time * (i64::try_from(open_sprinkler.iopts.wl).unwrap() / 100);
            water_time = water_time * (i64::try_from(open_sprinkler.controller_config.iopts.wl).unwrap() / 100);
        }
        //if water_time > 0 && !(open_sprinkler.attrib_dis[bid] & (1 << s)) {
        //if water_time > 0 && !open_sprinkler.stations[sid].attrib.dis {
        if water_time > 0 && !open_sprinkler.controller_config.stations.get(station_index).unwrap().attrib.dis {
            if program_data.enqueue(RuntimeQueueStruct { start_time: 0, water_time, sid: station_index, pid: 254 }).is_ok() {
                match_found = true;
            }
        }
    }
    if match_found {
        //let now: i64 = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs() as i64;
        schedule_all_stations(open_sprinkler, flow_state, program_data, chrono::Utc::now().timestamp());
    }
}
