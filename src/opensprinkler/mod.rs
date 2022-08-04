pub mod config;
pub mod events;
pub mod gpio;
mod http;
pub mod log;
pub mod program;
mod rf;
pub mod sensor;
pub mod station;
pub mod weather;

pub mod controller;
#[cfg(feature = "demo")]
mod demo;
#[cfg(feature = "mqtt")]
mod mqtt;
pub mod scheduler;
#[cfg(target_os = "linux")]
pub mod system;

use serde::{Deserialize, Serialize};
use std::cmp::max;
use std::path::PathBuf;

use self::config::ControllerConfiguration;
use self::sensor::{SensorType, MAX_SENSORS};
use self::station::{StationType, MAX_NUM_BOARDS, SHIFT_REGISTER_LINES};

/// Default reboot timer (seconds)
pub const REBOOT_DELAY: i64 = 65;

pub const MINIMUM_ON_DELAY: u8 = 5;
pub const MINIMUM_OFF_DELAY: u8 = 5;

const SPECIAL_CMD_REBOOT: &'static str = ":>reboot";
const SPECIAL_CMD_REBOOT_NOW: &'static str = ":>reboot_now";

pub type StationIndex = usize;

#[repr(u8)]
enum HardwareVersionBase {
    #[deprecated(since = "3.0.0", note = "Rust port of firmware is not compatible with Arduino/ESP platforms")]
    OpenSprinkler = 0x00,
    OpenSprinklerPi = 0x40,
    Simulated = 0xC0,
}

#[derive(Copy, Clone)]
struct ControllerSensorStatus {
    detected: bool,
    active: bool,
}

/// Volatile controller status bits
#[derive(Copy, Clone)]
pub struct ControllerStatus {
    /// operation enable (when set, controller operation is enabled)
    pub enabled: bool,
    /// rain delay bit (when set, rain delay is applied)
    pub rain_delayed: bool,
    // sensor1 status bit (when set, sensor1 on is detected)
    //pub sensor1: bool,
    /// [true] means a program is being executed currently
    pub program_busy: bool,
    /// [true] means a safe reboot has been marked
    pub safe_reboot: bool,
    // master station index
    //pub mas: Option<usize>,
    // master2 station index
    //pub mas2: Option<usize>,
    // sensor2 status bit (when set, sensor2 on is detected)
    //pub sensor2: bool,
    // sensor1 active bit (when set, sensor1 is activated)
    //pub sensor1_active: bool,
    // sensor2 active bit (when set, sensor2 is activated)
    //pub sensor2_active: bool,
    // request mqtt restart
    //pub req_mqtt_restart: bool,
    sensors: [ControllerSensorStatus; 2],

    /// Reboot timer
    pub reboot_timer: i64,
}

impl Default for ControllerStatus {
    fn default() -> Self {
        ControllerStatus {
            enabled: true,
            rain_delayed: false,
            //sensor1: false,
            program_busy: false,
            safe_reboot: false,
            //mas: None,
            //mas2: None,
            //sensor2: false,
            //sensor1_active: false,
            //sensor2_active: false,
            //req_mqtt_restart: false,
            reboot_timer: 0,

            sensors: [ControllerSensorStatus { detected: false, active: false }, ControllerSensorStatus { detected: false, active: false }],
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum RebootCause {
    /// None
    None = 0,
    /// Factory Reset
    Reset = 1,
    /// Hardware Button
    Button = 2,
    /* #[deprecated(since = "3.0.0", note = "Wi-Fi is handled by OS")]
    ResetAP = 3, */
    Timer = 4,
    Web = 5,
    /* #[deprecated(since = "3.0.0", note = "Wi-Fi is handled by OS")]
    WifiDone = 6, */
    FirmwareUpdate = 7,
    WeatherFail = 8,
    NetworkFail = 9,
    /* #[deprecated(since = "3.0.0", note = "NTP is handled by OS")]
    NTP = 10, */
    Program = 11,
    PowerOn = 99,
}

/// Flow Count Window (seconds)
///
/// For computing real-time flow rate.
const FLOW_COUNT_REALTIME_WINDOW: i64 = 30;

const HARDWARE_VERSION: u8 = HardwareVersionBase::OpenSprinklerPi as u8;

// @todo Get firmware version from cargo
const FIRMWARE_VERSION: u16 = 300;
const FIRMWARE_VERSION_REVISION: u8 = 0;

pub struct OpenSprinkler {
    config: config::Config,
    pub controller_config: config::ControllerConfiguration,

    pub flow_state: sensor::flow::State,

    #[cfg(not(feature = "demo"))]
    gpio: gpio::Gpio,

    #[cfg(feature = "mqtt")]
    pub mqtt: mqtt::OSMqtt,

    pub status_current: ControllerStatus,
    pub status_last: ControllerStatus,

    // Number of controller boards (including "master")
    //pub nboards: usize,

    // Total number of stations or zones
    //pub nstations: usize,
    /// station activation bits. each unsigned char corresponds to a board (8 stations)
    ///
    /// first byte-> master controller, second byte-> ext. board 1, and so on
    pub station_bits: [u8; MAX_NUM_BOARDS],

    //pub nvdata: config::data_type::ControllerNonVolatile,
    //pub iopts: config::data_type::IntegerOptions,
    //pub sopts: config::data_type::StringOptions,
    //pub stations: Stations,
    //pub programs: Programs,
    /// Sensor Status
    pub sensor_status: sensor::SensorStatusVec,

    /// time when the most recent rain delay started (seconds)
    pub raindelay_on_last_time: Option<i64>,

    /// Starting flow count (for logging)
    pub flow_count_log_start: u64,

    // flow count (for computing real-time flow rate)
    flow_count_rt: u64,
    flow_count_rt_start: u64,

    /// Weather service status
    pub weather_status: weather::WeatherStatus,

    /// time when controller is powered up most recently (seconds)
    ///
    /// When the program was started
    boot_time: chrono::DateTime<chrono::Utc>,

    /// Last reboot cause
    last_reboot_cause: RebootCause,

    sar__next_sid_to_refresh: usize,
    sar__last_now: i64,
}

impl OpenSprinkler {
    pub fn new(config_path: PathBuf) -> OpenSprinkler {
        //let nboards = 1;

        //let stations = station::default();
        //let programs = Vec::new();

        let gpio = gpio::Gpio::new();
        if let Err(ref error) = gpio {
            tracing::error!("Failed to obtain GPIO chip: {:?}", error);
        } else if gpio.is_ok() {
            OpenSprinkler::setup_gpio_pins(gpio.as_ref().unwrap());
        }

        OpenSprinkler {
            config: config::Config::new(config_path),
            controller_config: config::ControllerConfiguration::default(),

            flow_state: sensor::flow::State::default(),

            #[cfg(not(feature = "demo"))]
            gpio: gpio.unwrap(),

            #[cfg(feature = "mqtt")]
            mqtt: mqtt::OSMqtt::new(),

            status_current: ControllerStatus::default(),
            status_last: ControllerStatus::default(),
            sensor_status: sensor::init_vec(),
            //nboards,
            //nstations: nboards * SHIFT_REGISTER_LINES,
            station_bits: [0u8; MAX_NUM_BOARDS],
            boot_time: chrono::Utc::now(),
            raindelay_on_last_time: None,
            flow_count_log_start: 0,
            flow_count_rt: 0,
            flow_count_rt_start: 0,

            weather_status: weather::WeatherStatus::default(),

            // Initalize defaults
            //nvdata: config::data_type::ControllerNonVolatile::default(),
            //iopts: config::data_type::IntegerOptions::default(),
            //sopts: config::data_type::StringOptions::default(),
            //stations,
            //programs,
            last_reboot_cause: RebootCause::None,


            // special station auto-refresh
            sar__next_sid_to_refresh: station::MAX_NUM_STATIONS >> 1,
            sar__last_now: 0,
        }
    }

    fn setup_gpio_pins(gpio: &gpio::Gpio) {
        if let Err(ref error) = gpio.get(gpio::SHIFT_REGISTER_OE).and_then(|pin| Ok(pin.into_output().set_high())) {
            tracing::error!("Failed to obtain output pin SHIFT_REGISTER_OE: {:?}", error);
        }
        if let Err(ref error) = gpio.get(gpio::SHIFT_REGISTER_LATCH).and_then(|pin| Ok(pin.into_output().set_high())) {
            tracing::error!("Failed to obtain output pin SHIFT_REGISTER_LATCH: {:?}", error);
        }
        if let Err(ref error) = gpio.get(gpio::SHIFT_REGISTER_CLOCK).and_then(|pin| Ok(pin.into_output().set_high())) {
            tracing::error!("Failed to obtain output pin SHIFT_REGISTER_CLOCK: {:?}", error);
        }
        if let Err(ref error) = gpio.get(gpio::SHIFT_REGISTER_DATA).and_then(|pin| Ok(pin.into_output().set_high())) {
            tracing::error!("Failed to obtain output pin SHIFT_REGISTER_DATA: {:?}", error);
        }
        if let Err(ref error) = gpio.get(gpio::SENSOR_1).and_then(|pin| Ok(pin.into_input_pullup().set_reset_on_drop(false))) {
            // @todo Catch abnormal process terminations and reset pullup
            tracing::error!("Failed to obtain input pin SENSOR_1: {:?}", error);
        }
        if let Err(ref error) = gpio.get(gpio::SENSOR_2).and_then(|pin| Ok(pin.into_input_pullup().set_reset_on_drop(false))) {
            // @todo Catch abnormal process terminations and reset pullup
            tracing::error!("Failed to obtain input pin SENSOR_2: {:?}", error);
        }
        if let Err(ref error) = gpio.get(gpio::RF_TX).and_then(|pin| Ok(pin.into_output().set_low())) {
            tracing::error!("Failed to obtain output pin RF_TX: {:?}", error);
        }
    }

    // region: GETTERS

    /// Get the uptime of the system
    ///
    /// Will return [None] if the uptime could not be obtained.
    pub fn get_system_uptime() -> Option<std::time::Duration> {
        #[cfg(unix)]
        return std::time::Duration::from(nix::time::clock_gettime(nix::time::ClockId::CLOCK_MONOTONIC)?);

        None
    }
    pub fn is_logging_enabled(&self) -> bool {
        //self.iopts.lg == 1
        self.controller_config.lg
    }

    pub fn is_mqtt_enabled(&self) -> bool {
        self.controller_config.mqtt.enabled
        //self.sopts.mqtt
    }

    pub fn is_remote_extension(&self) -> bool {
        //self.iopts.re == 1
        self.controller_config.re
    }

    /// Gets the weather service URL (with adjustment method)
    pub fn get_weather_service_url(&self) -> Result<reqwest::Url, url::ParseError> {
        let mut url = url::Url::parse(&self.controller_config.wsp)?;
        let _ = url.path_segments_mut().and_then(|mut p| {
            p.push(&self.controller_config.uwt.to_string());
            Ok(())
        });
        Ok(url)
    }

    pub fn get_water_scale(&self) -> u8 {
        //self.iopts.wl
        self.controller_config.wl
    }

    pub fn get_sunrise_time(&self) -> u16 {
        //self.nvdata.sunrise_time
        self.controller_config.sunrise_time
    }

    pub fn get_sunset_time(&self) -> u16 {
        //self.nvdata.sunset_time
        self.controller_config.sunset_time
    }

    /// Number of eight-zone station boards (including master controller)
    pub fn get_board_count(&self) -> usize {
        //self.nboards
        //self.iopts.ext + 1
        self.controller_config.ext + 1
    }

    pub fn get_station_count(&self) -> usize {
        self.get_board_count() * SHIFT_REGISTER_LINES
    }

    pub fn is_station_running(&self, station_index: StationIndex) -> bool {
        let bid = station_index >> 3;
        let s = station_index & 0x07;
        self.station_bits[bid] & (1 << s) != 0
    }

    /// Get sensor type
    ///
    /// - `0` = Sensor 1
    /// - `1` = Sensor 2
    /// - ...
    pub fn get_sensor_type(&self, i: usize) -> SensorType {
        let st = if i == 0 {
            //self.iopts.sn1t
            self.controller_config.sn1t
        } else if i == 1 {
            //self.iopts.sn2t
            self.controller_config.sn2t
        } else {
            return SensorType::None;
        };

        match st {
            0x00 => SensorType::None,
            0x01 => SensorType::Rain,
            0x02 => SensorType::Flow,
            0x03 => SensorType::Soil,
            0xF0 => SensorType::ProgramSwitch,
            0xFF => SensorType::Other,
            _ => unreachable!(),
        }
    }

    pub fn get_sensor_normal_state(&self, i: usize) -> sensor::NormalState {
        // sensor_option: 0 if normally closed; 1 if normally open
        match i {
            //0 => self.iopts.sn1o,
            0 => self.controller_config.sn1o,
            //1 => self.iopts.sn2o,
            1 => self.controller_config.sn2o,
            _ => unreachable!(),
        }
    }

    pub fn get_sensor_on_delay(&self, i: usize) -> u8 {
        match i {
            0 => self.controller_config.sn1on,
            1 => self.controller_config.sn2on,
            _ => unreachable!(),
        }
    }

    pub fn get_sensor_off_delay(&self, i: usize) -> u8 {
        match i {
            0 => self.controller_config.sn1of,
            1 => self.controller_config.sn2of,
            _ => unreachable!(),
        }
    }

    pub fn get_flow_pulse_rate(&self) -> u16 {
        //(u16::from(self.iopts.fpr1) << 8) + u16::from(self.iopts.fpr0)
        (u16::from(self.controller_config.fpr1) << 8) + u16::from(self.controller_config.fpr0)
    }

    /// Returns the index (0-indexed) of a master station
    pub fn get_master_station_index(&self, i: usize) -> Option<StationIndex> {
        match i {
            0 => self.controller_config.mas,
            1 => self.controller_config.mas2,
            _ => None,
        }
    }

    pub fn is_master_station(&self, station_index: StationIndex) -> bool {
        self.get_master_station_index(0) == Some(station_index) || self.get_master_station_index(1) == Some(station_index)
    }
    // endregion GETTERS

    // region: SETTERS

    pub fn set_water_scale(&mut self, scale: u8) {
        //self.iopts.wl = scale;
        self.controller_config.wl = scale;
    }

    /// Update the weather service request success timestamp
    pub fn set_check_weather_success_timestamp(&mut self) {
        self.weather_status.checkwt_success_lasttime = Some(chrono::Utc::now().timestamp());
    }
    // endregion SETTERS

    pub fn start_flow_log_count(&mut self) {
        self.flow_count_log_start = self.flow_state.get_flow_count();
    }

    pub fn get_flow_log_count(&self) -> u64 {
        // @fixme potential subtraction overflow
        self.flow_state.get_flow_count() - self.flow_count_log_start
    }

    /// Realtime flow count
    pub fn update_realtime_flow_count(&mut self, now_seconds: i64) {
        //if open_sprinkler.iopts.sn1t == SensorType::Flow as u8 && now_seconds % FLOW_COUNT_REALTIME_WINDOW == 0 {
        if self.get_sensor_type(0) == SensorType::Flow && now_seconds % FLOW_COUNT_REALTIME_WINDOW == 0 {
            //open_sprinkler.flowcount_rt = if flow_state.flow_count > flow_count_rt_start { flow_state.flow_count - flow_count_rt_start } else { 0 };
            self.flow_count_rt = max(0, self.flow_state.get_flow_count() - self.flow_count_rt_start); // @fixme subtraction overflow
            self.flow_count_rt_start = self.flow_state.get_flow_count();
        }
    }

    pub fn check_reboot_request(&mut self, now_seconds: i64) {
        if self.status_current.safe_reboot && (now_seconds > self.status_current.reboot_timer) {
            // if no program is running at the moment and if no program is scheduled to run in the next minute
            //if !open_sprinkler.status.program_busy && !program_pending_soon(&open_sprinkler, &program_data, now_seconds + 60) {
            if !self.status_current.program_busy && !self.program_pending_soon(now_seconds + 60) {
                //open_sprinkler.reboot_dev(open_sprinkler.nvdata.reboot_cause);
                self.reboot_dev(self.controller_config.reboot_cause);
            }
        } else if self.status_current.reboot_timer != 0 && (now_seconds > self.status_current.reboot_timer) {
            self.reboot_dev(RebootCause::Timer);
        }
    }

    //fn program_pending_soon(open_sprinkler: &OpenSprinkler, program_data: &ProgramData, timestamp: i64) -> bool {
    fn program_pending_soon(&self, timestamp: i64) -> bool {
        //let mut program_pending_soon = false;
        //for program_index in 0..program_data.nprograms {
        for program in self.controller_config.programs.iter() {
            //if program_data.read(program_index).unwrap().check_match(&open_sprinkler, timestamp) {
            if program.check_match(self, timestamp) {
                //program_pending_soon = true;
                //break;
                return true;
            }
        }

        //program_pending_soon
        return false;
    }

    // Calculate local time (UTC time plus time zone offset)
    /* pub fn now_tz(&self) -> u64 {
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
        return now + 3600 / 4 * (self.iopts.tz - 48) as u64;
    } */

    /// Initalize network with given HTTP port
    ///
    /// @todo Separate server into separate process and use IPC
    pub fn start_network(&self) -> bool {
        //let _port: u16 = if cfg!(demo) { 80 } else { (self.iopts.hp1 as u16) << 8 + &self.iopts.hp0.into() };
        let _port: u16 = if cfg!(demo) { 80 } else { (self.controller_config.hp1 as u16) << 8 + &self.controller_config.hp0.into() };

        return true;
    }

    /// @todo Define primary interface e.g. `eth0` and check status (IFF_UP).
    pub fn network_connected(&self) -> bool {
        #[cfg(feature = "demo")]
        return true;

        #[cfg(target_os = "linux")]
        return system::is_interface_online("eth0");
    }

    /// @todo Use primary interface and get mac from it.
    pub fn load_hardware_mac() {}

    pub fn reboot_dev(&mut self, cause: RebootCause) {
        //self.nvdata.reboot_cause = cause;
        self.controller_config.reboot_cause = cause;
        self.nvdata_save();

        if cfg!(not(demo)) {
            // @todo reboot via commandline, dbus, libc::reboot, etc.
        }
    }

    // @todo Implement crate *self_update* for updates
    //pub fn update_dev() {}

    pub fn flow_poll(&mut self) {
        #[cfg(not(feature = "demo"))]
        let sensor1_pin = self.gpio.get(gpio::SENSOR_1).and_then(|pin| Ok(pin.into_input()));
        #[cfg(feature = "demo")]
        let sensor1_pin = demo::get_gpio_pin(gpio::SENSOR_1);

        if let Err(ref error) = sensor1_pin {
            tracing::error!("Failed to obtain sensor input pin (flow): {:?}", error);
        } else if let Ok(pin) = sensor1_pin {
            // Perform calculations using the current state of the sensor
            self.flow_state.poll(pin.read());
        }
    }

    /// Apply all station bits
    ///
    /// **This will actuate valves**
    pub fn apply_all_station_bits(&mut self) {
        #[cfg(not(feature = "demo"))]
        let shift_register_latch = self.gpio.get(gpio::SHIFT_REGISTER_LATCH).and_then(|pin| Ok(pin.into_output()));
        #[cfg(feature = "demo")]
        let shift_register_latch = demo::get_gpio_pin(gpio::SHIFT_REGISTER_LATCH);
        if let Err(ref error) = shift_register_latch {
            tracing::error!("Failed to obtain output pin shift_register_latch: {:?}", error);
        }

        #[cfg(not(feature = "demo"))]
        let shift_register_clock = self.gpio.get(gpio::SHIFT_REGISTER_CLOCK).and_then(|pin| Ok(pin.into_output()));
        #[cfg(feature = "demo")]
        let shift_register_clock = demo::get_gpio_pin(gpio::SHIFT_REGISTER_CLOCK);
        if let Err(ref error) = shift_register_clock {
            tracing::error!("Failed to obtain output pin shift_register_clock: {:?}", error);
        }

        #[cfg(not(feature = "demo"))]
        let shift_register_data = self.gpio.get(gpio::SHIFT_REGISTER_DATA).and_then(|pin| Ok(pin.into_output()));
        #[cfg(feature = "demo")]
        let shift_register_data = demo::get_gpio_pin(gpio::SHIFT_REGISTER_DATA);
        if let Err(ref error) = shift_register_data {
            tracing::error!("Failed to obtain output pin shift_register_data: {:?}", error);
        }

        if shift_register_latch.is_ok() && shift_register_clock.is_ok() && shift_register_data.is_ok() {
            let mut shift_register_latch = shift_register_latch.unwrap();
            let mut shift_register_clock = shift_register_clock.unwrap();
            let mut shift_register_data = shift_register_data.unwrap();

            shift_register_latch.set_low();

            // Shift out all station bit values from the highest bit to the lowest
            for board_index in 0..station::MAX_EXT_BOARDS {
                let sbits = if self.status_current.enabled { self.station_bits[station::MAX_EXT_BOARDS - board_index] } else { 0 };

                for s in 0..SHIFT_REGISTER_LINES {
                    shift_register_clock.set_low();

                    if sbits & (1 << (7 - s)) != 0 {
                        shift_register_data.set_high();
                        shift_register_data.set_low();
                    }

                    shift_register_clock.set_high();
                }
            }

            shift_register_latch.set_high();
        }

        
        if self.controller_config.sar {
            self.do_sar();
        }
    }

    /// Handle refresh of special stations
    /// 
    /// Original implementation details: [OpenSprinkler/OpenSprinkler-Firmware@d8c1bc0](https://github.com/OpenSprinkler/OpenSprinkler-Firmware/commit/d8c1bc0)
    /// 
    /// Refresh station that is next in line. This deliberately starts with station `101` to avoid startup delays.
    /// 
    /// @todo Async
    fn do_sar(&mut self) {
        let now = chrono::Utc::now().timestamp();

        if now > self.sar__last_now {
            // Perform this no more than once per second
            // @fixme variable lifetime
            self.sar__last_now = now;
            self.sar__next_sid_to_refresh = (self.sar__next_sid_to_refresh + 1) % station::MAX_NUM_STATIONS;
            let board_index = self.sar__next_sid_to_refresh >> 3;
            let s = self.sar__next_sid_to_refresh & 0x07;
            self.switch_special_station(self.sar__next_sid_to_refresh, (self.station_bits[board_index] >> s) & 0x01 != 0);
        }
    }

    /// Check rain delay status
    pub fn check_rain_delay_status(&mut self, now_seconds: i64) {
        if self.status_current.rain_delayed {
            //if now_seconds >= open_sprinkler.nvdata.rd_stop_time.unwrap_or(0) {
            if now_seconds >= self.controller_config.rd_stop_time.unwrap_or(0) {
                // rain delay is over
                self.rain_delay_stop();
            }
        } else {
            //if open_sprinkler.nvdata.rd_stop_time.unwrap_or(0) > now_seconds {
            if self.controller_config.rd_stop_time.unwrap_or(0) > now_seconds {
                // rain delay starts now
                self.rain_delay_start();
            }
        }

        // Check controller status changes and write log
        if self.status_last.rain_delayed != self.status_current.rain_delayed {
            if self.status_current.rain_delayed {
                // rain delay started, record time
                self.raindelay_on_last_time = now_seconds.try_into().unwrap();
                /* push_message(&open_sprinkler, NotifyEvent::RainDelay, RainDelay::new(true)); */
            } else {
                // rain delay stopped, write log
                let _ = log::write_log_message(&self, &log::message::SensorMessage::new(log::LogDataType::RainDelay, now_seconds), now_seconds);
                /* push_message(&open_sprinkler, NotifyEvent::RainDelay, RainDelay::new(false)); */
            }
            events::push_message(&self, &events::RainDelayEvent::new(true));
            self.status_last.rain_delayed = self.status_current.rain_delayed;
        }
    }

    fn detect_sensor_status(&mut self, i: usize, now_seconds: i64) {
        let sensor_type = self.get_sensor_type(i);

        if sensor_type == SensorType::Rain || sensor_type == SensorType::Soil {
            self.status_current.sensors[i].detected = self.get_sensor_detected(i);

            if self.status_current.sensors[i].detected {
                if self.sensor_status[i].on_timer.is_none() {
                    // add minimum of 5 seconds on-delay
                    self.sensor_status[i].on_timer = Some(max(self.get_sensor_on_delay(i) * 60, MINIMUM_ON_DELAY).into());
                    self.sensor_status[i].off_timer = Some(0);
                } else {
                    if now_seconds > self.sensor_status[i].on_timer.unwrap_or(0) {
                        self.status_current.sensors[i].active = true;
                    }
                }
            } else {
                if self.sensor_status[i].off_timer.is_none() {
                    // add minimum of 5 seconds off-delay
                    self.sensor_status[i].off_timer = Some(max(self.get_sensor_off_delay(i) * 60, MINIMUM_OFF_DELAY).into());
                    self.sensor_status[i].on_timer = Some(0);
                } else {
                    if now_seconds > self.sensor_status[i].off_timer.unwrap_or(0) {
                        self.status_current.sensors[i].active = false;
                    }
                }
            }
        }
    }

    /// Read sensor status
    fn detect_binary_sensor_status(&mut self, now_seconds: i64) {
        if cfg!(use_sensor_1) {
            self.detect_sensor_status(0, now_seconds);
        }

        if cfg!(use_sensor_2) {
            self.detect_sensor_status(1, now_seconds);
        }
    }

    /// Check binary sensor status (e.g. rain, soil)
    pub fn check_binary_sensor_status(&mut self, now_seconds: i64) {
        self.detect_binary_sensor_status(now_seconds);

        if self.status_last.sensors[0].active != self.status_current.sensors[0].active {
            // send notification when sensor becomes active
            if self.status_current.sensors[0].active {
                self.sensor_status[0].active_last_time = Some(now_seconds);
            } else {
                let _ = log::write_log_message(&self, &log::message::SensorMessage::new(log::LogDataType::Sensor1, now_seconds), now_seconds);
            }
            events::push_message(&self, &events::BinarySensorEvent::new(0, self.status_current.sensors[0].active));
        }
        self.status_last.sensors[0].active = self.status_current.sensors[0].active;
    }

    /// Check program switch status
    pub fn check_program_switch_status(&mut self, program_data: &mut program::ProgramQueue) {
        let program_switch = self.detect_program_switch_status();
        if program_switch[0] == true || program_switch[1] == true {
            self.reset_all_stations_immediate(program_data); // immediately stop all stations
        }

        for i in 0..MAX_SENSORS {
            //if program_data.nprograms > i {
            if self.controller_config.programs.len() > i {
                //manual_start_program(open_sprinkler, flow_state, program_data, i + 1, false);
                scheduler::manual_start_program(self, program_data, i + 1, false);
            }
        }
    }

    /// Immediately reset all stations
    ///
    /// No log records will be written
    pub fn reset_all_stations_immediate(&mut self, program_data: &mut program::ProgramQueue) {
        self.clear_all_station_bits();
        self.apply_all_station_bits();
        program_data.reset_runtime();
    }

    /// Check and process special program command
    pub fn process_special_program_command(&mut self, now_seconds: i64, program_name: &String) -> bool {
        if !program_name.starts_with(':') {
            return false;
        }

        if program_name == SPECIAL_CMD_REBOOT_NOW || program_name == SPECIAL_CMD_REBOOT {
            // reboot regardless of program status
            self.status_current.safe_reboot = match program_name.as_str() {
                SPECIAL_CMD_REBOOT_NOW => false,
                SPECIAL_CMD_REBOOT => true,
                _ => true,
            };
            // set a timer to reboot in 65 seconds
            self.status_current.reboot_timer = now_seconds + REBOOT_DELAY;
            // this is to avoid the same command being executed again right after reboot
            return true;
        }

        false
    }

    /// Gets the current state of the sensor (evaluates the normal state)
    ///
    /// |                               | [gpio::Level::Low] | [gpio::Level::High] |
    /// | ----------------------------- | ------------------ | ------------------- |
    /// | [sensor::NormalState::Closed] | [false]            | [true]              |
    /// | [sensor::NormalState::Open]   | [true]             | [false]             |
    fn get_sensor_detected(&self, i: usize) -> bool {
        let normal_state = self.get_sensor_normal_state(i);

        let pin = match i {
            0 => gpio::SENSOR_1,
            1 => gpio::SENSOR_2,
            _ => unreachable!(),
        };

        tracing::trace!("Reading sensor {}@bcm_pin_{} ({})", i, pin, normal_state);

        #[cfg(not(feature = "demo"))]
        let sensor_pin = self.gpio.get(pin).and_then(|pin| Ok(pin.into_input()));

        #[cfg(feature = "demo")]
        let sensor_pin = demo::get_gpio_pin(pin);

        if let Err(ref error) = sensor_pin {
            tracing::error!("Failed to obtain sensor input pin (flow): {:?}", error);
            return false;
        } else {
            return match sensor_pin.unwrap().read() {
                gpio::Level::Low => match normal_state {
                    sensor::NormalState::Closed => false,
                    sensor::NormalState::Open => true,
                },
                gpio::Level::High => match normal_state {
                    sensor::NormalState::Closed => true,
                    sensor::NormalState::Open => false,
                },
            };
        }
    }

    /// Return program switch status
    pub fn detect_program_switch_status(&mut self) -> [bool; MAX_SENSORS] {
        let mut ret = [false, false];

        for i in 0..MAX_SENSORS {
            if self.get_sensor_type(i) == SensorType::ProgramSwitch {
                self.status_current.sensors[i].detected = self.get_sensor_detected(i);

                self.sensor_status[i].history = (self.sensor_status[i].history << 1) | if self.status_current.sensors[i].detected { 1 } else { 0 };

                // basic noise filtering: only trigger if sensor matches pattern:
                // i.e. two consecutive lows followed by two consecutive highs
                if (self.sensor_status[i].history & 0b1111) == 0b0011 {
                    ret[i] = true;
                }
            }
        }

        ret
    }

    pub fn sensor_reset_all(&mut self) {
        /*         self.sensor1_status.on_timer = 0;
        self.sensor1_status.off_timer = 0;
        self.sensor1_status.active_last_time = 0;
        self.sensor2_status.on_timer = 0;
        self.sensor2_status.off_timer = 0;
        self.sensor2_status.active_last_time = 0; */

        self.sensor_status = sensor::init_vec();

        self.status_last.sensors[0].active = false;
        self.status_current.sensors[0].active = false;
        self.status_last.sensors[1].active = false;
        self.status_current.sensors[1].active = false;
    }

    /// Switch Radio Frequency (RF) station
    ///
    /// This function takes an RF code, parses it into signals and timing, and sends it out through the RF transmitter.
    fn switch_rf_station(&mut self, data: station::RFStationData, turn_on: bool) {
        //let (on, off, length) = self.parse_rf_station_code(data);
        let code = if turn_on { data.on } else { data.off };

        if let Err(ref error) = rf::send_rf_signal(self, code.into(), data.timing.into()) {
            tracing::error!("Could not switch RF station: {:?}", error);
        }
    }

    /// Switch GPIO station
    ///
    /// Special data for GPIO Station is three bytes of ascii decimal (not hex).
    fn switch_gpio_station(&self, data: station::GPIOStationData, state: bool) {
        tracing::trace!("[GPIO Station] pin: {} state: {}", data.pin, state);

        #[cfg(not(feature = "demo"))]
        let pin = self.gpio.get(data.pin).and_then(|pin| Ok(pin.into_output()));
        #[cfg(feature = "demo")]
        let pin = demo::get_gpio_pin(data.pin);
        if let Err(ref error) = pin {
            tracing::error!("[GPIO Station] pin {} Failed to obtain output pin: {:?}", data.pin, error);
            return;
        } else if pin.is_ok() {
            let mut pin = pin.unwrap();
            /* if state {
                if data.active_level() {
                    pin.unwrap().set_high();
                } else {
                    pin.unwrap().set_low();
                }
            } else {
                if data.active {
                    pin.unwrap().set_low();
                } else {
                    pin.unwrap().set_high();
                }
            } */
            pin.write(match state {
                false => !data.active_level(),
                true => data.active_level(),
            });
        }
    }

    /// Switch Remote Station
    /// This function takes a remote station code, parses it into remote IP, port, station index, and makes a HTTP GET request.
    /// The remote controller is assumed to have the same password as the main controller.
    fn switch_remote_station(&self, data: station::RemoteStationData, value: bool) {
        let mut host = String::from("http://"); // @todo HTTPS?
        host.push_str(&data.ip.to_string());
        //let timer = match self.iopts.sar {
        let timer = match self.controller_config.sar {
            true => (station::MAX_NUM_STATIONS * 4) as i64,
            false => 64800, // 18 hours
        };
        /* let en = match turn_on {
            true => String::from("1"),
            false => String::from("0"),
        }; */

        // @todo log request failures
        let response = reqwest::blocking::Client::new()
            .get(host)
            .query(&http::request::RemoteStationRequestParametersV219::new(&self.controller_config.dkey, data.sid, value, timer))
            .send();

        if let Err(error) = response {
            tracing::error!("[Remote Station] HTTP request error: {:?}", error);
        }
    }

    /// Switch HTTP station
    ///
    /// This function takes an http station code, parses it into a server name and two HTTP GET requests.
    fn switch_http_station(&self, data: station::HTTPStationData, turn_on: bool) {
        let mut origin: String = String::new();
        origin.push_str(&data.uri);
        if turn_on {
            origin.push_str(&data.cmd_on);
        } else {
            origin.push_str(&data.cmd_off);
        }

        // @todo log request failures
        let response = reqwest::blocking::get(origin);
        if let Err(error) = response {
            tracing::error!("[HTTP Station] HTTP request error: {:?}", error);
        }
    }

    /// Switch Special Station
    pub fn switch_special_station(&mut self, station_index: StationIndex, value: bool) {
        //let station = self.stations.get(station_index).unwrap();
        let station = self.controller_config.stations.get(station_index).unwrap();
        //let station_type = self.get_station_type(station);
        // check if station is "special"
        /* if station.r#type == StationType::Standard {
            return ();
        } */ // Not necessary, match block ignores standard

        //let data: &StationData;
        //let data = self.get_station_data(station);
        match station.r#type {
            StationType::RadioFrequency => self.switch_rf_station(station::RFStationData::try_from(station.sped.as_ref().unwrap()).unwrap(), value),
            StationType::Remote => self.switch_remote_station(station::RemoteStationData::try_from(station.sped.as_ref().unwrap()).unwrap(), value),
            StationType::GPIO => self.switch_gpio_station(station::GPIOStationData::try_from(station.sped.as_ref().unwrap()).unwrap(), value),
            StationType::HTTP => self.switch_http_station(station::HTTPStationData::try_from(station.sped.as_ref().unwrap()).unwrap(), value),
            // Nothing to do for [StationType::Standard] and [StationType::Other]
            _ => (),
        }
    }

    /// "Factory Reset
    ///
    /// This function should be called if the config does not exist.
    pub fn reset_to_defaults(&self) -> Result<(), config::Error> {
        tracing::info!("Resetting controller to defaults.");
        Ok(self.config.commit_defaults()?)
    }

    // Setup function for options
    pub fn options_setup(&mut self) {
        // Check reset conditions
        let config = self.config.get::<ControllerConfiguration>();
        if let Err(error) = config {
            tracing::error!("Error reading config: {:?}", error);
            self.reset_to_defaults();
            return;
        }

        // Check reset conditions
        if config.is_ok() {
            let config = config.unwrap();

            if config.fwv < FIRMWARE_VERSION {
                tracing::debug!("Invalid firmware version: {:?}", config.fwv);
                self.reset_to_defaults();
                return;
            }

            // This will be handled by the OS:
            /* let config_path = Path::new(&self.config_path);

            if !config_path.exists() {
                tracing::debug!("Config file does not exist: {:?}", config_path);
                self.reset_to_defaults();
                return;
            } */

            self.controller_config = config;

            //self.nvdata = config.nv;
            //self.iopts = config.iopts;
            //self.sopts = config.sopts;
            //self.stations = config.stations;
        }

        //{
        //let ref mut this = self;
        //this.iopts = config::get_integer_options().unwrap();

        //self.nboards = self.iopts.ext + 1;
        //self.nstations = self.nboards * SHIFT_REGISTER_LINES;
        //self.iopts.fwv = FIRMWARE_VERSION;
        self.controller_config.fwv = FIRMWARE_VERSION;
        //self.iopts.fwm = FIRMWARE_VERSION_REVISION;
        self.controller_config.fwm = FIRMWARE_VERSION_REVISION;
        //};
        //{
        //let ref mut this = self;
        //self.nvdata = config::get_controller_nv().unwrap();

        //self.old_status = self.status; // Not necessary, both fields are initialized with the same values.
        //};
        //self.last_reboot_cause = self.nvdata.reboot_cause;
        self.last_reboot_cause = self.controller_config.reboot_cause;
        //self.nvdata.reboot_cause = RebootCause::PowerOn;
        self.controller_config.reboot_cause = RebootCause::PowerOn;
        self.nvdata_save();
        //self.stations = config::get_stations().unwrap();
    }

    /// Save non-volatile controller status data
    pub fn nvdata_save(&self) {
        self.config.commit(&self.controller_config);
        //let _ = config::commit_controller_nv(&self.nvdata);
    }

    /// Save integer options to file
    pub fn iopts_save(&mut self) {
        self.config.commit(&self.controller_config);
        //let _ = config::commit_integer_options(&self.iopts);

        //self.nboards = self.iopts.ext + 1;
        //self.nstations = self.nboards * SHIFT_REGISTER_LINES;
        //self.status.enabled = match self.iopts.den {
        self.status_current.enabled = self.controller_config.den;
    }

    /*     /// Load a string option from file into a buffer.
    pub fn sopt_load_buf(&self, option_id: usize, buf: &mut [u8; MAX_SOPTS_SIZE]) {
        let mut reader = io::BufReader::new(File::open(DataFile::STRING_OPTIONS).unwrap());
        reader.seek(io::SeekFrom::Start((option_id * MAX_SOPTS_SIZE) as u64));
        reader.read_exact(buf);
    }

    /// Load a string option from file and return a String.
    pub fn sopt_load(&self, option_id: usize) -> String {
        let mut buf = [0u8; MAX_SOPTS_SIZE];
        self.sopt_load_buf(option_id, &mut buf);
        String::from_utf8(buf.try_into().unwrap()).unwrap()
    }

    /// Save a string option to file
    pub fn sopt_save(&self, option_id: u64, buf: Vec<u8>) {
        if file_cmp_block(
            DataFile::STRING_OPTIONS,
            buf,
            option_id * MAX_SOPTS_SIZE as u64,
        ) {
            // The value has not changed, skip writing.
            return;
        }

        let mut writer = io::BufWriter::new(File::create(DataFile::STRING_OPTIONS).unwrap());
        writer.write(&buf);
    } */

    /// Enable controller operation
    pub fn enable(&mut self) {
        self.status_current.enabled = true;
        //self.iopts.den = 1;
        self.controller_config.den = true;
        self.iopts_save();
    }

    /// Disable controller operation
    pub fn disable(&mut self) {
        self.status_current.enabled = false;
        //self.iopts.den = 0;
        self.controller_config.den = false;
        self.iopts_save();
    }

    /// Start rain delay
    pub fn rain_delay_start(&mut self) {
        self.status_current.rain_delayed = true;
        self.nvdata_save();
    }

    /// Stop rain delay
    pub fn rain_delay_stop(&mut self) {
        self.status_current.rain_delayed = false;
        //self.nvdata.rd_stop_time = None;
        self.controller_config.rd_stop_time = None;
        self.nvdata_save();
    }

    /// Set station bit
    ///
    /// This function sets the corresponding station bit. [OpenSprinkler::apply_all_station_bits()] must be called after to apply the bits (which results in physically actuating the valves).
    pub fn set_station_bit(&mut self, station: StationIndex, value: bool) -> StationBitChange {
        // Pointer to the station byte
        let data = self.station_bits[(station >> 3)];
        // Mask
        let mask = 1 << (station & 0x07);

        if value == true {
            if (data & mask) == 1 {
                // If bit is already set, return "no change"
                return StationBitChange::NoChange;
            } else {
                self.station_bits[(station >> 3)] = data | mask;
                // Handle special stations
                self.switch_special_station(station, true);
                return StationBitChange::On;
            }
        } else {
            if (data & mask) == 0 {
                // If bit is already set, return "no change"
                return StationBitChange::NoChange;
            } else {
                self.station_bits[(station >> 3)] = data & !mask;
                // Handle special stations
                self.switch_special_station(station, false);
                return StationBitChange::Off;
            }
        }
    }

    /// Clear all station bits
    pub fn clear_all_station_bits(&mut self) {
        for i in 0..station::MAX_NUM_STATIONS {
            self.set_station_bit(i, false);
        }
    }

    /// Process dynamic events
    ///
    /// Processes events such as: Rain delay, rain sensing, station state changes, etc.
    pub fn process_dynamic_events(&mut self, program_data: &mut program::ProgramQueue, now_seconds: i64) {
        let sn1 = (self.get_sensor_type(0) == SensorType::Rain || self.get_sensor_type(0) == SensorType::Soil) && self.status_current.sensors[0].active;
        let sn2 = (self.get_sensor_type(1) == SensorType::Rain || self.get_sensor_type(1) == SensorType::Soil) && self.status_current.sensors[1].active;

        for board_index in 0..self.get_board_count() {
            for line in 0..SHIFT_REGISTER_LINES {
                let station_index = board_index * SHIFT_REGISTER_LINES + line;

                // Ignore master stations because they are handled separately
                if self.is_master_station(station_index) {
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

                if q.pid >= program::TEST_PROGRAM_ID {
                    // This is a manually started program, skip
                    continue;
                }

                // If system is disabled, turn off zone
                if !self.status_current.enabled {
                    controller::turn_off_station(self, program_data, now_seconds, station_index);
                }

                // if rain delay is on and zone does not ignore rain delay, turn it off
                if self.status_current.rain_delayed && !self.controller_config.stations[station_index].attrib.igrd {
                    controller::turn_off_station(self, program_data, now_seconds, station_index);
                }

                // if sensor1 is on and zone does not ignore sensor1, turn it off
                if sn1 && !self.controller_config.stations[station_index].attrib.igs {
                    controller::turn_off_station(self, program_data, now_seconds, station_index);
                }

                // if sensor2 is on and zone does not ignore sensor2, turn it off
                if sn2 && !self.controller_config.stations[station_index].attrib.igs2 {
                    controller::turn_off_station(self, program_data, now_seconds, station_index);
                }
            }
        }
    }
}

#[derive(PartialEq)]
pub enum StationBitChange {
    NoChange = 0,
    On = 1,
    Off = 255,
}
