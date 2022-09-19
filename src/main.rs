#![allow(dead_code)]

mod opensprinkler;
mod utils;
pub mod timer;

include!(concat!(env!("OUT_DIR"), "/build_constants.rs"));

use clap::Parser;
use core::time;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
        Mutex,
    },
    thread,
};
use tracing_subscriber::FmtSubscriber;
use tracing_log::LogTracer;

use opensprinkler::{
    events,
    //program,
    weather,
    OpenSprinkler,
};

use crate::opensprinkler::{scheduler, config};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Binary config file path
    #[clap(short = 'c', long = "config", parse(from_os_str))]
    config: Option<std::path::PathBuf>,

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

pub fn setup_tracing() {
    // a builder for `FmtSubscriber`.
    let subscriber = FmtSubscriber::builder()
        // all spans/events with a level higher than TRACE (e.g, debug, info, warn, etc.) will be written to stdout.
        .with_max_level(tracing::Level::TRACE)
        // completes the builder.
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    // Convert [log::Record] to [tracing::Event]
    LogTracer::init().expect("Init log tracer failed");
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
    tracing::info!("MAX_EXT_BOARDS={}", constants::MAX_EXT_BOARDS);
    // endregion TRACING

    let mut open_sprinkler = if let Some(config) = args.config {
        OpenSprinkler::with_config_path(config)
    } else {
        OpenSprinkler::new()
    };

    // Setup options
    if let Err(ref error) = open_sprinkler.setup() {
        tracing::error!("Controller setup error: {:?}", error);
        return;
    }

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
            open_sprinkler.config.write().unwrap();
        } else if let Err(err) = result {
            println!("Error: {:?}", err);
        }
        return;
    }

    // Push reboot notification on startup
    open_sprinkler.push_event(events::RebootEvent::new(true));

    let open_sprinkler = Mutex::new(open_sprinkler);
    let open_sprinkler = Arc::new(open_sprinkler);

    // Time-keeping
    let mut now_seconds: i64;
    let mut last_seconds = 0;
    let mut now_minute: i64;
    let mut last_minute = 0;
    #[cfg(not(feature = "demo"))]
    let mut now_millis: i64;
    #[cfg(not(feature = "demo"))]
    let mut last_millis = 0;

    let open_sprinkler = Arc::clone(&open_sprinkler);

    // Main loop
    while running.load(Ordering::SeqCst) {
        
        let mut open_sprinkler = open_sprinkler.lock().unwrap();
        // handle flow sensor using polling every 1ms (maximum freq 1/(2*1ms)=500Hz)
        #[cfg(not(feature = "demo"))]
        if open_sprinkler.is_flow_sensor_enabled() {
            now_millis = chrono::Utc::now().timestamp_millis();

            if now_millis > last_millis {
                last_millis = now_millis;
                open_sprinkler.flow_poll();
            }
        }

        now_seconds = chrono::Utc::now().timestamp();

        // The main control loop runs once every second
        if now_seconds > last_seconds {
            last_seconds = now_seconds;
            now_minute = now_seconds / 60;

            #[cfg(feature = "mqtt")]
            open_sprinkler.try_mqtt_connect();
            
            open_sprinkler.check_rain_delay_status(now_seconds);

            #[cfg(not(feature = "demo"))]
            {
                open_sprinkler.check_binary_sensor_status(now_seconds);
                open_sprinkler.check_program_switch_status();
            }

            
            if now_minute > last_minute {
                last_minute = now_minute;

                // Schedule program data
                // since the granularity of start time is minute, we only need to check once every minute
                scheduler::check_program_schedule(&mut open_sprinkler, now_seconds);

                // STUN: Get external IP
                if let Ok(Some(ip)) = open_sprinkler.get_external_ip() {
                    if open_sprinkler.config.external_ip != Some(ip) {
                        open_sprinkler.config.external_ip = Some(ip);
                        open_sprinkler.config.write().unwrap();
                        tracing::trace!("External IP: {}", ip);
                    }
                }
            }

            // ====== Run program data ======
            // Check if a program is running currently
            // If so, do station run-time keeping
            if open_sprinkler.state.program.busy {
                opensprinkler::scheduler::do_time_keeping(&mut open_sprinkler, now_seconds);
            }
            
            open_sprinkler.activate_master_stations(now_seconds);

            // Process dynamic events
            open_sprinkler.process_dynamic_events(now_seconds);

            // Actuate valves
            open_sprinkler.apply_all_station_bits();

            // Handle reboot request
            open_sprinkler.check_reboot_request(now_seconds);

            // Flow count
            open_sprinkler.update_realtime_flow_count(now_seconds);

            // Check weather
            if let Err(ref err) = weather::check_weather(&mut open_sprinkler) {
                tracing::error!("Weather error: {:?}", err);
            }
        }

        // For OSPI/LINUX, sleep 1 ms to minimize CPU usage
        thread::sleep(time::Duration::from_millis(1));
    }

    tracing::info!("Got Ctrl-C, exiting...");
}