use super::{OpenSprinkler, program::ProgramQueue, station::SHIFT_REGISTER_LINES, events, sensor::SensorType, log, controller, loop_fns};

pub fn do_time_keeping(open_sprinkler: &mut OpenSprinkler, program_data: &mut ProgramQueue, now_seconds: i64) {
	// first, go through run time queue to assign queue elements to stations
	let mut qid = 0;
	for q in program_data.queue.iter() {
		let sid = q.sid;
		let sqi = program_data.station_qid[sid];
		// skip if station is already assigned a queue element
		// and that queue element has an earlier start time
		if sqi < 255 && program_data.queue[sqi].start_time < q.start_time {
			continue;
		}
		// otherwise assign the queue element to station
		program_data.station_qid[sid] = qid;
		qid += 1;
	}
	// next, go through the stations and perform time keeping
	for board_index in 0..open_sprinkler.get_board_count() {
		let bitvalue = open_sprinkler.station_bits[board_index];
		for s in 0..SHIFT_REGISTER_LINES {
			let station_index = board_index * 8 + s;

			// skip master station
			//if open_sprinkler.get_master_station_index(0) == Some(station_index) || open_sprinkler.get_master_station_index(1) == Some(station_index) {
			if open_sprinkler.is_master_station(station_index) {
				continue;
			}

			if program_data.station_qid[station_index] == 255 {
				continue;
			}

			let q = program_data.queue[program_data.station_qid[station_index]].clone();
			// check if this station is scheduled, either running or waiting to run
			if q.start_time > 0 {
				// if so, check if we should turn it off
				if now_seconds >= q.start_time + q.water_time {
					//loop_fns::turn_off_station(&mut open_sprinkler, &mut flow_state, &mut program_data, now_seconds, station_index);
					controller::turn_off_station(open_sprinkler, program_data, now_seconds, station_index);
				}
			}
			// if current station is not running, check if we should turn it on
			if !((bitvalue >> s) & 1 != 0) {
				if now_seconds >= q.start_time && now_seconds < q.start_time + q.water_time {
					//loop_fns::turn_on_station(&mut open_sprinkler, &mut flow_state, station_index);
					controller::turn_on_station(open_sprinkler, station_index);
				} // if curr_time > scheduled_start_time
			} // if current station is not running
		} // end_s
	} // end_bid

	// finally, go through the queue again and clear up elements marked for removal
	clean_queue(program_data, now_seconds);

	// process dynamic events
	//loop_fns::process_dynamic_events(&mut open_sprinkler, &mut program_data, &mut flow_state, now_seconds);
	loop_fns::process_dynamic_events(open_sprinkler, program_data, now_seconds);

	// activate / deactivate valves
	open_sprinkler.apply_all_station_bits();

	// check through runtime queue, calculate the last stop time of sequential stations
	program_data.last_seq_stop_time = None;
	//let sequential_stop_time: i64;
	//let re = open_sprinkler.iopts.re == 1;

	for q in program_data.queue.iter() {
		let station_index = q.sid;
		//let bid = station_index >> 3;
		//let s = station_index & 0x07;

		// check if any sequential station has a valid stop time
		// and the stop time must be larger than curr_time
		let sequential_stop_time = (q.start_time + q.water_time) as i64;
		if sequential_stop_time > now_seconds {
			// only need to update last_seq_stop_time for sequential stations
			//if open_sprinkler.attrib_seq[bid] & (1 << s) && !re {
			//if open_sprinkler.stations[station_index].attrib.seq && !re {
			if open_sprinkler.controller_config.stations[station_index].attrib.seq && !open_sprinkler.controller_config.re {
				program_data.last_seq_stop_time = if sequential_stop_time > program_data.last_seq_stop_time.unwrap() {
					Some(sequential_stop_time)
				} else {
					program_data.last_seq_stop_time
				};
			}
		}
	}

	// if the runtime queue is empty, reset all stations
	if program_data.queue.is_empty() {
		// turn off all stations
		open_sprinkler.clear_all_station_bits();
		open_sprinkler.apply_all_station_bits();
		// reset runtime
		program_data.reset_runtime();
		// reset program busy bit
		open_sprinkler.status.program_busy = false;
		// log flow sensor reading if flow sensor is used
		if open_sprinkler.get_sensor_type(0) == SensorType::Flow {
			let _ = log::write_log_message(&open_sprinkler, &log::message::FlowSenseMessage::new(open_sprinkler.get_flow_log_count(), now_seconds), now_seconds);
			events::push_message(
				&open_sprinkler,
				&events::FlowSensorEvent::new(
					//u32::try_from(flow_state.flow_count - open_sprinkler.flow_count_log_start).unwrap_or(0),
					open_sprinkler.get_flow_log_count(),
					/* if flow_state.flow_count > open_sprinkler.flow_count_log_start {flow_state.flow_count - open_sprinkler.flow_count_log_start} else {0}, */
					open_sprinkler.get_flow_pulse_rate(),
				),
			);
		}

		// in case some options have changed while executing the program
		//open_sprinkler.status.mas = open_sprinkler.iopts.mas; // update master station
		//open_sprinkler.status.mas = open_sprinkler.controller_config.mas; // update master station
		//open_sprinkler.status.mas2 = open_sprinkler.iopts.mas2; // update master2 station
		//open_sprinkler.status.mas2 = open_sprinkler.controller_config.mas2;
		// update master2 station
	}
}

/// Clean Queue
///
/// This removes queue elements if:
/// - water_time is not greater than zero; or
/// - if current time is greater than element duration
fn clean_queue(program_data: &mut ProgramQueue, now_seconds: i64) {
    /* let mut qi = program_data.queue.len() as i64 - 1;
    while qi >= 0 {
        let q = program_data.queue.get(qi).unwrap();

        if !(q.water_time > 0) || now_seconds >= q.start_time + q.water_time {
            program_data.dequeue(qi);
        }
        qi -= 1;
    } */

    for qi in 0..program_data.queue.len() {
        let q = program_data.queue.get(qi).unwrap();
        if !(q.water_time > 0) || now_seconds >= q.start_time + q.water_time {
            program_data.dequeue(qi);
        }
    }
}