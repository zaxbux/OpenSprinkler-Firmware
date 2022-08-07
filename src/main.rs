#![allow(dead_code)]

mod opensprinkler;
mod utils;
pub mod timer;

use clap::Parser;
use core::time;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
};
use tracing_subscriber::FmtSubscriber;

use opensprinkler::{
    events,
    program,
    weather,
    OpenSprinkler,
};

use crate::opensprinkler::{scheduler, sensor, config};

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

    /// Set a config value
    #[clap(long = "set", takes_value = true, required = false, min_values = 2, max_values = 2)]
    set: Option<Vec<String>>,

    // List config values
    #[clap(long = "list", takes_value = false)]
    list: bool,

    // Reset all config values
    #[clap(long = "reset", takes_value = false)]
    reset: bool,
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
    open_sprinkler.options_setup().unwrap();

    // ProgramData initialization
    let mut program_data = program::ProgramQueue::new();

    if args.reset {
        config::cli::reset(&open_sprinkler);
        return;
    }

    if args.list {
        config::cli::list(&open_sprinkler);
        return;
    }

    if let Some(set_config) = args.set {
        let result = config::cli::set(set_config, &mut open_sprinkler);

        if let Ok(ok) = result {
            println!("Success: {:?}", ok);
            open_sprinkler.commit_config().unwrap();
        } else if let Err(err) = result {
            println!("Error: {:?}", err);
        }
        return;
    }

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
        if open_sprinkler.get_sensor_type(0).unwrap_or(sensor::SensorType::None) == sensor::SensorType::Flow {
            now_millis = chrono::Utc::now().timestamp_millis();

            if now_millis > last_millis {
                last_millis = now_millis;
                //loop_fns::flow_poll(&open_sprinkler, &mut flow_state);
                open_sprinkler.flow_poll();
            }
        }

        now_seconds = chrono::Utc::now().timestamp();

        // The main control loop runs once every second
        if now_seconds > last_seconds {
            last_seconds = now_seconds;
            now_minute = now_seconds / 60;

            // Start MQTT when there is a network connection
            // @todo use [paho_mqtt::async_client::AsyncClient#is_connected()] instead of start_mqtt
            if start_mqtt && open_sprinkler.is_mqtt_enabled() && open_sprinkler.network_connected() {
                tracing::debug!("Network is OK, starting MQTT");
                //open_sprinkler.mqtt.begin(); @todo
                start_mqtt = false
            }

            
            open_sprinkler.check_rain_delay_status(now_seconds);
            open_sprinkler.check_binary_sensor_status(now_seconds);
            open_sprinkler.check_program_switch_status(&mut program_data);

            // since the granularity of start time is minute, we only need to check once every minute
            if now_minute > last_minute {
                last_minute = now_minute;

                // Schedule program data
                scheduler::check_program_schedule(&mut open_sprinkler, &mut program_data, now_seconds);
            }

            // ====== Run program data ======
            // Check if a program is running currently
            // If so, do station run-time keeping
            if open_sprinkler.status_current.program_busy {
                opensprinkler::scheduler::do_time_keeping(&mut open_sprinkler, &mut program_data, now_seconds);
            }

            opensprinkler::controller::activate_master_station(0, &mut open_sprinkler, &program_data, now_seconds);
            opensprinkler::controller::activate_master_station(1, &mut open_sprinkler, &program_data, now_seconds);

            // Process dynamic events
            //loop_fns::process_dynamic_events(&mut open_sprinkler, &mut program_data, &mut flow_state, now_seconds);
            open_sprinkler.process_dynamic_events(&mut program_data, now_seconds);

            // Actuate valves
            open_sprinkler.apply_all_station_bits();

            // Handle reboot request
            open_sprinkler.check_reboot_request(now_seconds);

            // Push reboot notification on startup
            // @todo move outside of loop?
            if reboot_notification {
                reboot_notification = false;
                events::push_message(&open_sprinkler, &events::RebootEvent::new(true));
            }

            open_sprinkler.update_realtime_flow_count(now_seconds);

            // Check weather
            let _ = weather::check_weather(&mut open_sprinkler, &|open_sprinkler, weather_update_flag| {
                // at the moment, we only send notification if water level or external IP changed
                // the other changes, such as sunrise, sunset changes are ignored for notification
                // @fixme Should this be in the weather module?
                match weather_update_flag {
                    //WeatherUpdateFlag::EIP => push_message(&open_sprinkler, WeatherUpdateEvent::new(Some(open_sprinkler.iopts.wl), None)),
                    weather::WeatherUpdateFlag::EIP => events::push_message(&open_sprinkler, &events::WeatherUpdateEvent::new(Some(open_sprinkler.controller_config.water_scale), None)),
                    //WeatherUpdateFlag::WL => push_message(&open_sprinkler, WeatherUpdateEvent::new(None, open_sprinkler.nvdata.external_ip)),
                    weather::WeatherUpdateFlag::WL => events::push_message(&open_sprinkler, &events::WeatherUpdateEvent::new(None, open_sprinkler.controller_config.external_ip)),
                    _ => (),
                }
            });
        }

        // For OSPI/LINUX, sleep 1 ms to minimize CPU usage
        thread::sleep(time::Duration::from_millis(1));
    }

    tracing::info!("Got Ctrl-C, exiting...");
}
