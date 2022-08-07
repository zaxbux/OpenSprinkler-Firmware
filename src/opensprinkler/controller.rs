use super::{OpenSprinkler, StationBitChange, events, program, sensor, log, station};

/// Stations/Zones per board
pub const SHIFT_REGISTER_LINES: usize = 8;

/// Turn on a station
pub fn turn_on_station(open_sprinkler: &mut OpenSprinkler, station_id: station::StationIndex) {
    // RAH implementation of flow sensor
    open_sprinkler.flow_state.reset();

    if open_sprinkler.set_station_bit(station_id, true) == StationBitChange::On {
        let station_name = open_sprinkler.config.stations.get(station_id).unwrap().name.to_string();
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
pub fn turn_off_station(open_sprinkler: &mut OpenSprinkler, program_data: &mut program::ProgramQueue, now_seconds: i64, station_index: station::StationIndex) {
    open_sprinkler.set_station_bit(station_index, false);

    let qid = program_data.station_qid[station_index];

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
        if !open_sprinkler.is_master_station(station_index) {
            let duration = u16::try_from(now_seconds - q.start_time).unwrap();

            // log station run
            let mut message = log::message::StationMessage::new(
                q.pid,
                station_index,
                duration, // @fixme Maximum duration is 18 hours (64800 seconds), which fits into a [u16]
                now_seconds,
            );

            // Keep a copy for web
            program_data.last_run = Some(message);

            if open_sprinkler.get_sensor_type(0).unwrap_or(sensor::SensorType::None) == sensor::SensorType::Flow {
                message.with_flow(flow_volume);
            }
            let _ = log::write_log_message(open_sprinkler, &message, now_seconds);

            //let station_name = open_sprinkler.stations[station_id].name.clone();
            let station_name = &open_sprinkler.config.stations[station_index].name;
            events::push_message(
                open_sprinkler,
                &events::StationEvent::new(
                    station_index,
                    station_name,
                    false,
                    Some(duration.into()),
                    if open_sprinkler.get_sensor_type(0).unwrap_or(sensor::SensorType::None) == sensor::SensorType::Flow { Some(flow_volume) } else { None },
                ),
            );
        }
    }

    // dequeue the element
    program_data.dequeue(qid);
    program_data.station_qid[station_index] = 0xFF;
}

/// Actuate master stations based on need
///
/// This function iterates over all stations and activates the necessary "master" station.
pub fn activate_master_station(i: usize, open_sprinkler: &mut OpenSprinkler, program_data: &program::ProgramQueue, now_seconds: i64) {
    let master_station = open_sprinkler.get_master_station(i);

    if master_station.station.is_none() {
        return;
    }

    let master_station_index = master_station.station.unwrap();
    let adjusted_on = master_station.get_adjusted_on_time();
    let adjusted_off = master_station.get_adjusted_off_time();

    for station_index in 0..open_sprinkler.get_station_count() {
        // skip if this is the master station
        if master_station_index == station_index {
            continue;
        }

        // if this station is running and is set to activate master
        if open_sprinkler.is_station_running(station_index) && open_sprinkler.config.stations[station_index].attrib.use_master[i] {
            let q = program_data.queue.get(program_data.station_qid[station_index]).unwrap();
            // check if timing is within the acceptable range
            let start_time = q.start_time + adjusted_on;
            let stop_time = q.start_time + q.water_time + adjusted_off;
            if now_seconds >= start_time && now_seconds <= stop_time {
                open_sprinkler.set_station_bit(master_station_index, true);
                return;
            }
        }
    }
    open_sprinkler.set_station_bit(master_station_index, false);
}