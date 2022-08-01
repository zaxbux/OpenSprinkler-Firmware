mod opensprinkler;
mod utils;
use core::time;
use std::thread;

use opensprinkler::events::{push_message, FlowSensor as FlowSensorEvent, ProgramSched, Reboot, WeatherUpdate};
use opensprinkler::log;
use opensprinkler::program::{ProgramData, RuntimeQueueStruct};
use opensprinkler::sensor::{SensorType, FLOW_COUNT_REALTIME_WINDOW};
use opensprinkler::station::SHIFT_REGISTER_LINES;
use opensprinkler::weather::{check_weather, WeatherUpdateFlag};
use opensprinkler::OpenSprinkler;
use opensprinkler::{loop_fns, RebootCause};
use utils::{water_time_decode_signed, water_time_resolve};

/// Robert Hillman (RAH)'s implementation of flow sensor
///
/// @todo Move into [OpenSprinkler] to simplify main loop calls.
/// - turn_on_station()
/// - turn_off_station()
/// - schedule_all_stations()
/// -
#[derive(Default)]
pub struct FlowSensor {
    /// time when valve turns on
    pub flow_begin: u32,
    /// time when flow starts being measured (i.e. 2 mins after flow_begin approx
    pub flow_start: u32,
    /// time when valve turns off (last rising edge pulse detected before off)
    pub flow_stop: u32,
    /// total # of gallons+1 from flow_start to flow_stop
    pub flow_gallons: u32,
    /// last flow rate measured (averaged over flow_gallons) from last valve stopped (used to write to log file).
    pub flow_last_gpm: f32,

    /// current flow count
    pub flow_count: u32,

    pub prev_flow_state: Option<rppal::gpio::Level>,
}

fn main() {
    // OpenSprinkler initialization
    let mut open_sprinkler = OpenSprinkler::new();
    // Setup options
    open_sprinkler.options_setup();

    // ProgramData initialization
    let mut program_data = ProgramData::new();

    let mut flow_state = FlowSensor::default();

    //let mut reboot_timer = 0; // use open_sprinkler.status.reboot_timer

    // open_sprinkler.start_network() was here!

    //open_sprinkler.mqtt.init();
    open_sprinkler.status.req_mqtt_restart = true;

    let mut last_seconds = 0;
    let mut last_minute = 0;

    let mut flow_poll_timeout = 0;

    let mut flow_count_rt_start: u32 = 0;

    let mut reboot_notification = true;

    loop {
        // handle flow sensor using polling every 1ms (maximum freq 1/(2*1ms)=500Hz)
        if open_sprinkler.get_sensor_type(0) == SensorType::Flow {
            let now_millis = chrono::Utc::now().timestamp_millis();

            if now_millis != flow_poll_timeout {
                flow_poll_timeout = now_millis;
                loop_fns::flow_poll(&open_sprinkler, &mut flow_state);
            }
        }

        open_sprinkler.status.mas = open_sprinkler.iopts.mas;
        open_sprinkler.status.mas2 = open_sprinkler.iopts.mas2;

        let now_seconds = chrono::Utc::now().timestamp();

        // Start MQTT when there is a network connection
        if open_sprinkler.status.req_mqtt_restart && open_sprinkler.network_connected() {
            println!("req_mqtt_restart");
            //open_sprinkler.mqtt.begin();
            open_sprinkler.status.req_mqtt_restart = false;
        }
        //open_sprinkler.mqtt.loop();

        // The main control loop runs once every second
        if now_seconds > last_seconds {
            last_seconds = now_seconds;

            // Check rain delay status
            loop_fns::check_rain_delay(&mut open_sprinkler, &program_data, now_seconds);

            // Check binary sensor status (e.g. rain, soil)
            loop_fns::check_binary_sensor_status(&mut open_sprinkler, &program_data, now_seconds);

            // Check program switch status
            loop_fns::check_program_switch_status(&mut open_sprinkler, &mut flow_state, &mut program_data, now_seconds);

            // Schedule program data
            // region: Schedule program data
            let curr_minute = now_seconds / 60;
            let mut match_found = false;

            // since the granularity of start time is minute
            // we only need to check once every minute
            if curr_minute > last_minute {
                last_minute = curr_minute;

                // check through all programs
                for program_index in 0..program_data.nprograms {
                    let program = program_data.read(program_index).unwrap();
                    if program.check_match(&open_sprinkler, now_seconds) {
                        // program match found
                        // check and process special program command
                        if loop_fns::process_special_program_command(&mut open_sprinkler, now_seconds, &program.name) {
                            continue;
                        }

                        // process all selected stations
                        for station_index in 0..open_sprinkler.nstations {
                            //let bid = station_index >> 3; // or `station_index / 8`
                            //let s = station_index & 0x07; // 0..7

                            // skip if the station is a master station (because master cannot be scheduled independently
                            if (open_sprinkler.status.mas.unwrap_or(0) == station_index + 1) || (open_sprinkler.status.mas2.unwrap_or(0) == station_index + 1) {
                                continue;
                            }

                            // if station has non-zero water time and the station is not disabled
                            if program.durations[station_index] > 0 && !open_sprinkler.stations[station_index].attrib.dis {
                            //if program.durations[station_index] > 0 && !(open_sprinkler.attrib_dis[bid] & (1 << s)) {
                                // water time is scaled by watering percentage
                                let mut water_time = water_time_resolve(program.durations[station_index], open_sprinkler.get_sunrise_time(), open_sprinkler.get_sunset_time());
                                // if the program is set to use weather scaling
                                if program.use_weather == 1 {
                                    let wl = open_sprinkler.iopts.wl;
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
                            push_message(
                                &open_sprinkler,
                                ProgramSched::new(program_index, program.name, program.use_weather == 0, if program.use_weather != 0 { open_sprinkler.iopts.wl } else { 100 }),
                            );
                        }
                    }
                }

                // calculate start and end time
                if match_found {
                    loop_fns::schedule_all_stations(&mut open_sprinkler, &mut flow_state, &mut program_data, now_seconds as i64);
                }
            }

            // ====== Run program data ======
            // Check if a program is running currently
            // If so, do station run-time keeping
            if open_sprinkler.status.program_busy {
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
                for bid in 0..open_sprinkler.get_board_count() {
                    let bitvalue = open_sprinkler.station_bits[bid];
                    for s in 0..SHIFT_REGISTER_LINES {
                        let station_index = bid * 8 + s;

                        // skip master station
                        if (open_sprinkler.status.mas.unwrap_or(0) == station_index + 1) || (open_sprinkler.status.mas2.unwrap_or(0) == station_index + 1) {
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
                                loop_fns::turn_off_station(&mut open_sprinkler, &mut flow_state, &mut program_data, now_seconds, station_index);
                            }
                        }
                        // if current station is not running, check if we should turn it on
                        if !((bitvalue >> s) & 1 != 0) {
                            if now_seconds >= q.start_time && now_seconds < q.start_time + q.water_time {
                                loop_fns::turn_on_station(&mut open_sprinkler, &mut flow_state, station_index);
                            } // if curr_time > scheduled_start_time
                        } // if current station is not running
                    } // end_s
                } // end_bid

                // finally, go through the queue again and clear up elements marked for removal
                clean_queue(&mut program_data, now_seconds);

                // process dynamic events
                loop_fns::process_dynamic_events(&mut open_sprinkler, &mut program_data, &mut flow_state, now_seconds);

                // activate / deactivate valves
                open_sprinkler.apply_all_station_bits();

                // check through runtime queue, calculate the last stop time of sequential stations
                program_data.last_seq_stop_time = None;
                //let sequential_stop_time: i64;
                let re = open_sprinkler.iopts.re == 1;

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
                        if open_sprinkler.stations[station_index].attrib.seq && !re {
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
                        log::write_log_message(&open_sprinkler, &log::message::FlowSenseMessage::new(flow_state.flow_count - open_sprinkler.flow_count_log_start, now_seconds), now_seconds);
                        push_message(
                            &open_sprinkler,
                            FlowSensorEvent::new(
                                u32::try_from(flow_state.flow_count - open_sprinkler.flow_count_log_start).unwrap_or(0),
                                /* if flow_state.flow_count > open_sprinkler.flow_count_log_start {flow_state.flow_count - open_sprinkler.flow_count_log_start} else {0}, */
                                open_sprinkler.get_flow_pulse_rate(),
                            ),
                        );
                    }

                    // in case some options have changed while executing the program
                    open_sprinkler.status.mas = open_sprinkler.iopts.mas; // update master station
                    open_sprinkler.status.mas2 = open_sprinkler.iopts.mas2; // update master2 station
                }
            }

            // handle master
            handle_master(MasterStation::ONE, &mut open_sprinkler, &program_data, now_seconds);
            /* if open_sprinkler.status.mas > 0 {
                let mas_on_adj: i64 = water_time_decode_signed(open_sprinkler.iopts.mton).into();
                let mas_off_adj: i64 = water_time_decode_signed(open_sprinkler.iopts.mtof).into();
                let mut masbit = false;

                for station_index in 0..open_sprinkler.nstations {
                    // skip if this is the master station
                    if open_sprinkler.status.mas == station_index + 1 {
                        continue;
                    }
                    //let bid = sid >> 3;
                    //let s = sid & 0x07;

                    // if this station is running and is set to activate master
                    //if (open_sprinkler.station_bits[bid] & (1 << s) != 0) && (open_sprinkler.attrib_mas[bid] & (1 << s) != 0) {
                    if open_sprinkler.is_station_running(station_index) && open_sprinkler.stations[station_index].attrib.mas {
                        let q = program_data.queue.get(program_data.station_qid[station_index]).unwrap();
                        // check if timing is within the acceptable range
                        if now_seconds >= q.start_time + mas_on_adj && now_seconds <= q.start_time + q.water_time + mas_off_adj {
                            masbit = true;
                            break;
                        }
                    }
                }
                open_sprinkler.set_station_bit(open_sprinkler.status.mas - 1, masbit);
            } */
            // handle master2
            handle_master(MasterStation::TWO, &mut open_sprinkler, &program_data, now_seconds);
            /* if open_sprinkler.status.mas2 > 0 {
                let mas_on_adj_2: i64 = water_time_decode_signed(open_sprinkler.iopts.mton2).into();
                let mas_off_adj_2: i64 = water_time_decode_signed(open_sprinkler.iopts.mtof2).into();
                let mut masbit2 = false;
                for sid in 0..open_sprinkler.nstations {
                    // skip if this is the master station
                    if open_sprinkler.status.mas2 == sid + 1 {
                        continue;
                    }
                    let bid = sid >> 3;
                    let s = sid & 0x07;
                    // if this station is running and is set to activate master
                    if (open_sprinkler.station_bits[bid] & (1 << s) != 0) && (open_sprinkler.attrib_mas2[bid] & (1 << s) != 0) {
                        let q = program_data.queue.get(program_data.station_qid[sid]).unwrap();
                        // check if timing is within the acceptable range
                        if now_seconds >= q.start_time + mas_on_adj_2 && now_seconds <= q.start_time + q.water_time + mas_off_adj_2 {
                            masbit2 = true;
                            break;
                        }
                    }
                }
                open_sprinkler.set_station_bit(open_sprinkler.status.mas2 - 1, masbit2);
            } */

            // endregion

            // Process dynamic events
            loop_fns::process_dynamic_events(&mut open_sprinkler, &mut program_data, &mut flow_state, now_seconds);

            // Actuate valves
            open_sprinkler.apply_all_station_bits();

            // Handle reboot request
            if open_sprinkler.status.safe_reboot && (now_seconds > open_sprinkler.status.reboot_timer) {
                // if no program is running at the moment and if no program is scheduled to run in the next minute
                if !open_sprinkler.status.program_busy && !program_pending_soon(&open_sprinkler, &program_data, now_seconds + 60) {
                    open_sprinkler.reboot_dev(open_sprinkler.nvdata.reboot_cause);
                }
            } else if open_sprinkler.status.reboot_timer != 0 && (now_seconds > open_sprinkler.status.reboot_timer) {
                open_sprinkler.reboot_dev(RebootCause::Timer);
            }

            // Push reboot notification on startup
            // @todo move outside of loop?
            if reboot_notification {
                reboot_notification = false;
                push_message(&open_sprinkler, Reboot::new(true));
            }

            // Realtime flow count
            
            if open_sprinkler.iopts.sn1t == SensorType::Flow as u8 && now_seconds % FLOW_COUNT_REALTIME_WINDOW == 0 {
                open_sprinkler.flowcount_rt = if flow_state.flow_count > flow_count_rt_start { flow_state.flow_count - flow_count_rt_start } else { 0 };
                flow_count_rt_start = flow_state.flow_count;
            }

            // Check weather
            check_weather(&mut open_sprinkler, &|open_sprinkler, weather_update_flag| {
                // at the moment, we only send notification if water level or external IP changed
                // the other changes, such as sunrise, sunset changes are ignored for notification
                match weather_update_flag {
                    WeatherUpdateFlag::EIP => push_message(&open_sprinkler, WeatherUpdate::new(Some(open_sprinkler.iopts.wl), None)),
                    WeatherUpdateFlag::WL => push_message(&open_sprinkler, WeatherUpdate::new(None, open_sprinkler.nvdata.external_ip)),
                    _ => (),
                }
            });
        }

        // For OSPI/LINUX, sleep 1 ms to minimize CPU usage
        thread::sleep(time::Duration::from_millis(1));
    }
}

/// Clean Queue
/// 
/// This removes queue elements if:
/// - water_time is not greater than zero; or
/// - if current time is greater than element duration
fn clean_queue(program_data: &mut ProgramData, now_seconds: i64) {
    let mut qi = program_data.queue.len() - 1;
    while qi >= 0 {
        let q = program_data.queue.get(qi).unwrap();
        
        if !(q.water_time > 0) || now_seconds >= q.start_time + q.water_time {
            program_data.dequeue(qi);
        }
        qi -= 1;
    }
}

fn program_pending_soon(open_sprinkler: &OpenSprinkler, program_data: &ProgramData, timestamp: i64) -> bool {
    let mut program_pending_soon = false;
    // @todo iter over programs directly
    for program_index in 0..program_data.nprograms {
        if program_data.read(program_index).unwrap().check_match(&open_sprinkler, timestamp) {
            program_pending_soon = true;
            break;
        }
    }

    program_pending_soon
}

enum MasterStation {
    ONE,
    TWO,
}

/// Actuate master stations based on need
/// 
/// This function iterates over all stations and activates the necessary "master" station.
fn handle_master(master: MasterStation, open_sprinkler: &mut OpenSprinkler, program_data: &ProgramData, now_seconds: i64) {
    let mas = match master {
        MasterStation::ONE => open_sprinkler.status.mas.unwrap_or(0),
        MasterStation::TWO => open_sprinkler.status.mas2.unwrap_or(0),
    };
    let mas_on_adj: i64 = water_time_decode_signed(match master {
        MasterStation::ONE => open_sprinkler.iopts.mton,
        MasterStation::TWO => open_sprinkler.iopts.mton2,
    }).into();
    let mas_off_adj: i64 = water_time_decode_signed(match master {
        MasterStation::ONE => open_sprinkler.iopts.mtof,
        MasterStation::TWO => open_sprinkler.iopts.mtof2,
    }).into();

    let mut value = false;

    for station_index in 0..open_sprinkler.nstations {
        // skip if this is the master station
        if mas == station_index + 1 {
            continue;
        }

        let use_master = match master {
            MasterStation::ONE => open_sprinkler.stations[station_index].attrib.mas,
            MasterStation::TWO => open_sprinkler.stations[station_index].attrib.mas2,
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