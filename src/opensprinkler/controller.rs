use crate::utils;

use super::{OpenSprinkler, StationBitChange, events, program::ProgramData, sensor::SensorType, log};

/// Turn on a station
//pub fn turn_on_station(open_sprinkler: &mut OpenSprinkler, flow_state: &mut FlowSensor, station_id: usize) {
pub fn turn_on_station(open_sprinkler: &mut OpenSprinkler, station_id: usize) {
    // RAH implementation of flow sensor
    //flow_state.flow_start = 0;
    open_sprinkler.flow_state.reset();

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
//pub fn turn_off_station(open_sprinkler: &mut OpenSprinkler, flow_state: &mut FlowSensor, program_data: &mut ProgramData, now_seconds: i64, station_id: usize) {
pub fn turn_off_station(open_sprinkler: &mut OpenSprinkler, program_data: &mut ProgramData, now_seconds: i64, station_id: usize) {
    open_sprinkler.set_station_bit(station_id, false);

    let qid = program_data.station_qid[station_id];

    // ignore if we are turning off a station that is not running or is not scheduled to run
    if qid >= program_data.queue.len() {
        return;
    }

    // RAH implementation of flow sensor
    let flow_volume = open_sprinkler.flow_state.measure();

    let q = program_data.queue.get(qid).unwrap();

    // check if the current time is past the scheduled start time,
    // because we may be turning off a station that hasn't started yet
    if now_seconds > q.start_time {
        // record lastrun log (only for non-master stations)
        if (open_sprinkler.status.mas.unwrap_or(0) != station_id + 1) && (open_sprinkler.status.mas2.unwrap_or(0) != station_id + 1) {
            let duration = u16::try_from(now_seconds - q.start_time).unwrap();

            // log station run
            let mut message = log::message::StationMessage::new(
                q.pid,
                station_id,
                duration, // @fixme Maximum duration is 18 hours (64800 seconds), which fits into a [u16]
                now_seconds,
            );

            // Keep a copy for web
            program_data.last_run = Some(message);

            //if open_sprinkler.iopts.sn1t == SensorType::Flow as u8 {
            if open_sprinkler.get_sensor_type(0) == SensorType::Flow {
                //message.with_flow(flow_state.flow_last_gpm);
                message.with_flow(flow_volume);
            }
            let _ = log::write_log_message(open_sprinkler, &message, now_seconds);

            //let station_name = open_sprinkler.stations[station_id].name.clone();
            let station_name = &open_sprinkler.controller_config.stations[station_id].name;
            events::push_message(
                open_sprinkler,
                &events::StationEvent::new(
                    station_id,
                    station_name,
                    false,
                    Some(duration.into()),
                    //if open_sprinkler.iopts.sn1t == SensorType::Flow as u8 { Some(flow_state.flow_last_gpm) } else { None },
                    //if open_sprinkler.get_sensor_type(0) == SensorType::Flow { Some(flow_state.flow_last_gpm) } else { None },
                    if open_sprinkler.get_sensor_type(0) == SensorType::Flow { Some(flow_volume) } else { None },
                ),
            );
        }
    }

    // dequeue the element
    program_data.dequeue(qid);
    program_data.station_qid[station_id] = 0xFF;
}

/// Actuate master stations based on need
///
/// This function iterates over all stations and activates the necessary "master" station.
pub fn activate_master_station(master: usize, open_sprinkler: &mut OpenSprinkler, program_data: &ProgramData, now_seconds: i64) {
    let mas = match master {
        0 => open_sprinkler.status.mas.unwrap_or(0),
        1 => open_sprinkler.status.mas2.unwrap_or(0),
		_ => todo!(),
    };

    if mas == 0 {
        return;
    }

    let mas_on_adj: i64 = utils::water_time_decode_signed(match master {
        0 => open_sprinkler.controller_config.mton,
        //0 => open_sprinkler.iopts.mton,
        //1 => open_sprinkler.iopts.mton2,
        1 => open_sprinkler.controller_config.mton2,
		_ => todo!(),
    })
    .into();
    let mas_off_adj: i64 = utils::water_time_decode_signed(match master {
        0 => open_sprinkler.controller_config.mtof,
        //0 => open_sprinkler.iopts.mtof,
        //1 => open_sprinkler.iopts.mtof2,
        1 => open_sprinkler.controller_config.mtof2,
		_ => todo!(),
    })
    .into();

    let mut value = false;

    for station_index in 0..open_sprinkler.get_station_count() {
        // skip if this is the master station
        if mas == station_index + 1 {
            continue;
        }

        let use_master = match master {
            //0 => open_sprinkler.stations[station_index].attrib.mas,
            0 => open_sprinkler.controller_config.stations[station_index].attrib.mas,
            //1 => open_sprinkler.stations[station_index].attrib.mas2,
            1 => open_sprinkler.controller_config.stations[station_index].attrib.mas2,
			_ => todo!(),
        };

        // if this station is running and is set to activate master
        if open_sprinkler.is_station_running(station_index) && use_master {
            let q = program_data.queue.get(program_data.station_qid[station_index]).unwrap();
            // check if timing is within the acceptable range
            let start_time = q.start_time + mas_on_adj;
            let stop_time = q.start_time + q.water_time + mas_off_adj;
            if now_seconds >= start_time && now_seconds <= stop_time {
                value = true;
                break;
            }
        }
    }
    open_sprinkler.set_station_bit(mas - 1, value);
}