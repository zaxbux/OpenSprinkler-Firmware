use super::{events, log, state, station, OpenSprinkler};

/// Stations/Zones per board
pub const SHIFT_REGISTER_LINES: usize = 8;

/// Turn on a station
pub fn turn_on_station(open_sprinkler: &mut OpenSprinkler, station_index: station::StationIndex) {
    // RAH implementation of flow sensor
    open_sprinkler.state.flow.reset();

    //if open_sprinkler.set_station_bit(station_id, true) == StationBitChange::On {
    if open_sprinkler.state.station.set_active(station_index, true) == state::StationChange::Change(true) {
        let station_name = open_sprinkler.config.stations.get(station_index).unwrap().name.to_string();
        open_sprinkler.push_event(
            events::StationEvent {
                station_index,
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
pub fn turn_off_station(open_sprinkler: &mut OpenSprinkler, now_seconds: i64, station_index: station::StationIndex) {
    //open_sprinkler.set_station_bit(station_index, false);
    open_sprinkler.state.station.set_active(station_index, false);

    if let Some(qid) = open_sprinkler.state.program.queue.station_qid[station_index] {
        // ignore if we are turning off a station that is not running or is not scheduled to run
        if let Some(q) = open_sprinkler.state.program.queue.queue.get(qid) {

            // RAH implementation of flow sensor
            let flow_volume = open_sprinkler.state.flow.measure();

            // check if the current time is past the scheduled start time,
            // because we may be turning off a station that hasn't started yet
            if now_seconds > q.start_time {
                // record lastrun log (only for non-master stations)
                if !open_sprinkler.is_master_station(station_index) {
                    let duration = now_seconds - q.start_time;

                    // log station run
                    let mut message = log::message::StationMessage::new(
                        q.program_index,
                        station_index,
                        duration, // Maximum duration is 18 hours (64800 seconds), which fits into a [u16]
                        now_seconds,
                    );

                    // Keep a copy for web
                    open_sprinkler.state.program.queue.last_run = Some(message);

                    if open_sprinkler.is_flow_sensor_enabled() {
                        message.with_flow(flow_volume);
                    }
                    let _ = log::write_log_message(open_sprinkler, &message, now_seconds);

                    //let station_name = open_sprinkler.stations[station_id].name.clone();
                    open_sprinkler.push_event(
                        events::StationEvent::new(
                            station_index,
                            open_sprinkler.config.stations[station_index].name.clone(),
                            false,
                            Some(duration.into()),
                            if open_sprinkler.is_flow_sensor_enabled() {
                                Some(flow_volume)
                            } else {
                                None
                            },
                        ),
                    );
                }
            }

            // dequeue the element
            open_sprinkler.state.program.queue.dequeue(qid);
            open_sprinkler.state.program.queue.station_qid[station_index] = None;
        }
    }
}

/// Actuate master stations based on need
///
/// This function iterates over all stations and activates the necessary "master" station.
pub fn activate_master_station(master_station: usize, open_sprinkler: &mut OpenSprinkler, now_seconds: i64) {
    let config = open_sprinkler.get_master_station(master_station);

    if let Some(station_index_master) = config.station {
        let adjusted_on = config.get_adjusted_on_time();
        let adjusted_off = config.get_adjusted_off_time();

        for station_index in 0..open_sprinkler.get_station_count() {
            // skip if this is the master station
            if station_index_master == station_index {
                continue;
            }

            // if this station is running and is set to activate master
            if open_sprinkler.is_station_running(station_index) && open_sprinkler.config.stations[station_index].attrib.use_master[master_station] {
                if let Some(qid) = open_sprinkler.state.program.queue.station_qid[station_index] {
                    if let Some(q) = open_sprinkler.state.program.queue.queue.get(qid) {
                        // check if timing is within the acceptable range
                        let start_time = q.start_time + adjusted_on;
                        let stop_time = q.start_time + q.water_time + adjusted_off;
                        if now_seconds >= start_time && now_seconds <= stop_time {
                            open_sprinkler.state.station.set_active(station_index_master, true);
                            return;
                        }
                    } else {
                        panic!("This should not happen");
                    }
                } else {
                    panic!("This should not happen");
                }
            }
        }
        open_sprinkler.state.station.set_active(station_index_master, false);
    }
}
