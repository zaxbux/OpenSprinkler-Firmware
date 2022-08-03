#![allow(dead_code)]

mod opensprinkler;
mod utils;

use clap::Parser;
use core::time;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use tracing_subscriber::FmtSubscriber;

use opensprinkler::events::{push_message, RebootEvent, WeatherUpdateEvent};
use opensprinkler::program::ProgramData;
use opensprinkler::sensor::SensorType;
use opensprinkler::weather::{check_weather, WeatherUpdateFlag};
use opensprinkler::OpenSprinkler;
use opensprinkler::{loop_fns, RebootCause};

#[cfg(unix)]
const CONFIG_FILE_PATH: &'static str = "/etc/opt/config.dat";

#[cfg(not(unix))]
const CONFIG_FILE_PATH: &'static str = "./config.dat";

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Binary config file path
    #[clap(short = 'c', long = "config", default_value = CONFIG_FILE_PATH, parse(from_os_str))]
    config: std::path::PathBuf,
}

fn setup_tracing() {
    // a builder for `FmtSubscriber`.
    let subscriber = FmtSubscriber::builder()
        // all spans/events with a level higher than TRACE (e.g, debug, info, warn, etc.)
        // will be written to stdout.
        .with_max_level(tracing::Level::TRACE)
        // completes the builder.
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
}

fn main() {
    let args = Args::parse();

    // region: SIGNALS
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");
    // endregion SIGNALS

    // region: TRACING
    setup_tracing();

    #[cfg(feature = "demo")]
    tracing::info!("DEMO MODE");
    // endregion TRACING

    tracing::info!("Using config file: {}", args.config.display());

    // OpenSprinkler initialization
    tracing::trace!("Initialize controller");
    let mut open_sprinkler = OpenSprinkler::new(args.config);
    // Setup options
    // @todo move into ::new()
    open_sprinkler.options_setup();

    // ProgramData initialization
    let mut program_data = ProgramData::new();

    //let mut flow_state = FlowSensor::default();

    //let mut reboot_timer = 0; // use open_sprinkler.status.reboot_timer

    // open_sprinkler.start_network() was here!

    //open_sprinkler.mqtt.init();
    //open_sprinkler.status.req_mqtt_restart = true;

    // Time-keeping
    let mut now_seconds: i64;
    let mut last_seconds = 0;
    let mut now_minute: i64;
    let mut last_minute = 0;
    let mut now_millis: i64;
    let mut last_millis = 0;

    // Do-once flags
    let mut reboot_notification = true;
    let mut start_mqtt = true;

    // Main loop
    while running.load(Ordering::SeqCst) {
        // handle flow sensor using polling every 1ms (maximum freq 1/(2*1ms)=500Hz)
        if open_sprinkler.get_sensor_type(0) == SensorType::Flow {
            now_millis = chrono::Utc::now().timestamp_millis();

            if now_millis > last_millis {
                last_millis = now_millis;
                //loop_fns::flow_poll(&open_sprinkler, &mut flow_state);
                loop_fns::flow_poll(&mut open_sprinkler);
            }
        }

        //open_sprinkler.status.mas = open_sprinkler.iopts.mas;
        open_sprinkler.status.mas = open_sprinkler.controller_config.iopts.mas;
        //open_sprinkler.status.mas2 = open_sprinkler.iopts.mas2;
        open_sprinkler.status.mas2 = open_sprinkler.controller_config.iopts.mas2;

        now_seconds = chrono::Utc::now().timestamp();

        // The main control loop runs once every second
        if now_seconds > last_seconds {
            last_seconds = now_seconds;
            now_minute = now_seconds / 60;

            // Start MQTT when there is a network connection
            //if open_sprinkler.status.req_mqtt_restart && open_sprinkler.network_connected() {
            // @todo use [paho_mqtt::async_client::AsyncClient#is_connected()] instead of start_mqtt
            if start_mqtt && open_sprinkler.is_mqtt_enabled() && open_sprinkler.network_connected() {
                tracing::debug!("Network is OK, starting MQTT");
                //open_sprinkler.mqtt.begin(); @todo
                //open_sprinkler.status.req_mqtt_restart = false;
                start_mqtt = false
            }

            // Check rain delay status
            loop_fns::check_rain_delay(&mut open_sprinkler, now_seconds);

            // Check binary sensor status (e.g. rain, soil)
            loop_fns::check_binary_sensor_status(&mut open_sprinkler, now_seconds);

            // Check program switch status
            //loop_fns::check_program_switch_status(&mut open_sprinkler, &mut flow_state, &mut program_data);
            loop_fns::check_program_switch_status(&mut open_sprinkler, &mut program_data);

            // Schedule program data

            // since the granularity of start time is minute
            // we only need to check once every minute
            if now_minute > last_minute {
                last_minute = now_minute;

                loop_fns::check_program_schedule(&mut open_sprinkler, &mut program_data, now_seconds);
            }

            // ====== Run program data ======
            // Check if a program is running currently
            // If so, do station run-time keeping
            if open_sprinkler.status.program_busy {
                opensprinkler::scheduler::do_time_keeping(&mut open_sprinkler, &mut program_data, now_seconds);
            }

            opensprinkler::controller::activate_master_station(0, &mut open_sprinkler, &program_data, now_seconds);
            opensprinkler::controller::activate_master_station(1, &mut open_sprinkler, &program_data, now_seconds);

            // Process dynamic events
            //loop_fns::process_dynamic_events(&mut open_sprinkler, &mut program_data, &mut flow_state, now_seconds);
            loop_fns::process_dynamic_events(&mut open_sprinkler, &mut program_data, now_seconds);

            // Actuate valves
            open_sprinkler.apply_all_station_bits();

            // Handle reboot request
            if open_sprinkler.status.safe_reboot && (now_seconds > open_sprinkler.status.reboot_timer) {
                // if no program is running at the moment and if no program is scheduled to run in the next minute
                //if !open_sprinkler.status.program_busy && !program_pending_soon(&open_sprinkler, &program_data, now_seconds + 60) {
                if !open_sprinkler.status.program_busy && !program_pending_soon(&open_sprinkler, now_seconds + 60) {
                    //open_sprinkler.reboot_dev(open_sprinkler.nvdata.reboot_cause);
                    open_sprinkler.reboot_dev(open_sprinkler.controller_config.nv.reboot_cause);
                }
            } else if open_sprinkler.status.reboot_timer != 0 && (now_seconds > open_sprinkler.status.reboot_timer) {
                open_sprinkler.reboot_dev(RebootCause::Timer);
            }

            // Push reboot notification on startup
            // @todo move outside of loop?
            if reboot_notification {
                reboot_notification = false;
                push_message(&open_sprinkler, &RebootEvent::new(true));
            }

            open_sprinkler.update_realtime_flow_count(now_seconds);

            // Check weather
            let _ = check_weather(&mut open_sprinkler, &|open_sprinkler, weather_update_flag| {
                // at the moment, we only send notification if water level or external IP changed
                // the other changes, such as sunrise, sunset changes are ignored for notification
                // @fixme Should this be in the weather module?
                match weather_update_flag {
                    //WeatherUpdateFlag::EIP => push_message(&open_sprinkler, WeatherUpdateEvent::new(Some(open_sprinkler.iopts.wl), None)),
                    WeatherUpdateFlag::EIP => push_message(&open_sprinkler, &WeatherUpdateEvent::new(Some(open_sprinkler.controller_config.iopts.wl), None)),
                    //WeatherUpdateFlag::WL => push_message(&open_sprinkler, WeatherUpdateEvent::new(None, open_sprinkler.nvdata.external_ip)),
                    WeatherUpdateFlag::WL => push_message(&open_sprinkler, &WeatherUpdateEvent::new(None, open_sprinkler.controller_config.nv.external_ip)),
                    _ => (),
                }
            });
        }

        // For OSPI/LINUX, sleep 1 ms to minimize CPU usage
        thread::sleep(time::Duration::from_millis(1));
    }

    tracing::info!("Got Ctrl-C, exiting...");
}

//fn program_pending_soon(open_sprinkler: &OpenSprinkler, program_data: &ProgramData, timestamp: i64) -> bool {
fn program_pending_soon(open_sprinkler: &OpenSprinkler, timestamp: i64) -> bool {
    //let mut program_pending_soon = false;
    //for program_index in 0..program_data.nprograms {
    for program in open_sprinkler.controller_config.programs.iter() {
        //if program_data.read(program_index).unwrap().check_match(&open_sprinkler, timestamp) {
        if program.check_match(&open_sprinkler, timestamp) {
            //program_pending_soon = true;
            //break;
            return true;
        }
    }

    //program_pending_soon
    return false;
}
