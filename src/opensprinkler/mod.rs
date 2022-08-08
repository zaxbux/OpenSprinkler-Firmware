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
pub mod errors;
#[cfg(feature = "mqtt")]
mod mqtt;
pub mod scheduler;
#[cfg(target_os = "linux")]
pub mod system;

use std::cmp::max;
use std::path::PathBuf;

use self::http::request;

/// Default reboot timer (seconds)
pub const REBOOT_DELAY: i64 = 65;

pub const MINIMUM_ON_DELAY: u8 = 5;
pub const MINIMUM_OFF_DELAY: u8 = 5;

const SPECIAL_CMD_REBOOT: &'static str = ":>reboot";
const SPECIAL_CMD_REBOOT_NOW: &'static str = ":>reboot_now";

#[derive(Copy, Clone)]
struct ControllerSensorStatus {
    detected: bool,
    active: bool,
}

/// Volatile controller status bits
#[derive(Copy, Clone)]
pub struct ControllerStatus {
    /// rain delay bit (when [true], rain delay is applied)
    pub rain_delayed: bool,
    /// [true] means a program is being executed currently
    pub program_busy: bool,
    /// [true] means a safe reboot has been marked
    pub safe_reboot: bool,
    /// Sensor status
    sensors: [ControllerSensorStatus; 2],
    /// Reboot timer
    pub reboot_timer: i64,
}

impl Default for ControllerStatus {
    fn default() -> Self {
        ControllerStatus {
            rain_delayed: false,
            program_busy: false,
            safe_reboot: false,
            reboot_timer: 0,

            sensors: [ControllerSensorStatus { detected: false, active: false }, ControllerSensorStatus { detected: false, active: false }],
        }
    }
}

/// Flow Count Window (seconds)
///
/// For computing real-time flow rate.
const FLOW_COUNT_REALTIME_WINDOW: i64 = 30;

pub struct OpenSprinkler {
    pub config: config::Config,
    pub status_current: ControllerStatus,
    pub status_last: ControllerStatus,

    /// Weather service status
    pub weather_status: weather::WeatherStatus,

    /// time when the most recent rain delay started (seconds)
    pub raindelay_on_last_time: Option<i64>,

    /// Sensor Status
    pub sensor_status: [sensor::SensorStatus; sensor::MAX_SENSORS], // @todo refactor

    /// station activation bits. each unsigned char corresponds to a board (8 stations)
    ///
    /// first byte-> master controller, second byte-> ext. board 1, and so on
    pub station_bits: [u8; station::MAX_NUM_BOARDS],

    /// Starting flow count (for logging)
    pub flow_count_log_start: u64,

    pub flow_state: sensor::flow::State,

    #[cfg(feature = "mqtt")]
    pub mqtt: mqtt::Mqtt,

    gpio: Option<gpio::Gpio>,

    /// time when controller is powered up most recently (seconds)
    ///
    /// When the program was started
    boot_time: chrono::DateTime<chrono::Utc>,

    sar__next_sid_to_refresh: usize,
    sar__last_now: i64,

    // flow count (for computing real-time flow rate)
    flow_count_rt: u64,
    flow_count_rt_start: u64,
}

impl OpenSprinkler {
    pub fn new() -> OpenSprinkler {
        Self::default()
    }

    pub fn with_config_path(config_path: PathBuf) -> Self {
        Self {
            config: config::Config::new(config_path),
            ..Self::default()
        }
    }

    /// Setup controller
    pub fn setup(&mut self) -> errors::Result<()> {
        // Read configuration from file
        if !self.config.exists() {
            tracing::debug!("Config file does not exist");
        }

        // Check reset conditions
        if let Ok(config) = self.config.read() {
            if self.check_config(&config)? {
                // Replace defaults with config from file.
                self.config = config;
            }
        }

        #[cfg(not(feature = "demo"))]
        self.setup_gpio();

        #[cfg(feature = "mqtt")]
        self.mqtt.setup(&self.config.mqtt)?;

        // Store the last reboot cause in memory and set the new cause
        self.config.last_reboot_cause = self.config.reboot_cause;
        self.config.reboot_cause = config::RebootCause::PowerOn;
        self.config.write()?;

        Ok(())
    }

    fn check_config(&self, config: &config::Config) -> config::result::Result<bool> {
        // @todo What about higher version numbers?
        if config.firmware_version < self.config.firmware_version {
            // @todo Migrate config based on existing version
            tracing::debug!("Invalid firmware version: {:?}", config.firmware_version);
            return Ok(false);
        }

        tracing::debug!("Config is OK");
        Ok(true)
    }

    /// Setup GPIO peripheral and pins
    ///
    /// @todo: Check hardware version and determine which GPIO peripheral to use.
    #[cfg(not(feature = "demo"))]
    fn setup_gpio(&mut self) {
        // Setup GPIO peripheral
        let gpio = gpio::Gpio::new();
        if let Err(ref error) = gpio {
            tracing::error!("Cannot access GPIO peripheral: {:?}", error);
        } else if let Ok(gpio) = gpio {
            self.gpio = Some(gpio);
        }

        // Setup GPIO pins
        if let Some(ref gpio) = self.gpio {
            if let Err(ref error) = gpio.get(gpio::SHIFT_REGISTER_OE).and_then(|pin| Ok(pin.into_output().set_high())) {
                tracing::error!("GPIO Error (SHIFT_REGISTER_OE): {:?}", error);
            }
            if let Err(ref error) = gpio.get(gpio::SHIFT_REGISTER_LATCH).and_then(|pin| Ok(pin.into_output().set_high())) {
                tracing::error!("GPIO Error (SHIFT_REGISTER_LATCH): {:?}", error);
            }
            if let Err(ref error) = gpio.get(gpio::SHIFT_REGISTER_CLOCK).and_then(|pin| Ok(pin.into_output().set_high())) {
                tracing::error!("GPIO Error (SHIFT_REGISTER_CLOCK): {:?}", error);
            }
            if let Err(ref error) = gpio.get(gpio::SHIFT_REGISTER_DATA).and_then(|pin| Ok(pin.into_output().set_high())) {
                tracing::error!("GPIO Error (SHIFT_REGISTER_DATA): {:?}", error);
            }
            for i in 0..sensor::MAX_SENSORS {
                if let Err(ref error) = gpio.get(gpio::SENSOR[i]).and_then(|pin| Ok(pin.into_input_pullup().set_reset_on_drop(false))) {
                    // @todo Catch abnormal process terminations and reset pullup
                    tracing::error!("GPIO Error (SENSOR[{}]): {:?}", i, error);
                }
            }
            if let Err(ref error) = gpio.get(gpio::RF_TX).and_then(|pin| Ok(pin.into_output().set_low())) {
                tracing::error!("GPIO Error (RF_TX): {:?}", error);
            }
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
        self.config.enable_log
    }

    pub fn is_mqtt_enabled(&self) -> bool {
        #[cfg(feature = "mqtt")]
        return self.config.mqtt.enabled;

        #[cfg(not(feature = "mqtt"))]
        return false;
    }

    pub fn is_remote_extension(&self) -> bool {
        self.config.enable_remote_ext_mode
    }

    /// Gets the weather service URL (with adjustment method)
    pub fn get_weather_service_url(&self) -> Result<Option<reqwest::Url>, url::ParseError> {
        if let Some(algorithm) = &self.config.weather.algorithm {
            let mut url = url::Url::parse(&self.config.weather.service_url)?;
            if let Ok(mut path_seg) = url.path_segments_mut() {
                path_seg.push(&algorithm.get_id().to_string());
            }
            return Ok(Some(url));
        }
        return Ok(None);
    }

    pub fn get_water_scale(&self) -> u8 {
        self.config.water_scale
    }

    pub fn get_sunrise_time(&self) -> u16 {
        self.config.sunrise_time
    }

    pub fn get_sunset_time(&self) -> u16 {
        self.config.sunset_time
    }

    /// Number of eight-zone station boards (including master controller)
    pub fn get_board_count(&self) -> usize {
        self.config.extension_board_count + 1
    }

    pub fn get_station_count(&self) -> usize {
        self.get_board_count() * controller::SHIFT_REGISTER_LINES
    }

    pub fn is_station_running(&self, station_index: station::StationIndex) -> bool {
        let bid = station_index >> 3;
        let s = station_index & 0x07;
        self.station_bits[bid] & (1 << s) != 0
    }

    pub fn get_sensor_type(&self, i: usize) -> Option<sensor::SensorType> {
        self.config.sensors[i].sensor_type
    }

    pub fn get_sensor_normal_state(&self, i: usize) -> sensor::NormalState {
        self.config.sensors[i].normal_state
    }

    pub fn get_sensor_on_delay(&self, i: usize) -> u8 {
        self.config.sensors[i].delay_on
    }

    pub fn get_sensor_off_delay(&self, i: usize) -> u8 {
        self.config.sensors[i].delay_off
    }

    pub fn get_flow_pulse_rate(&self) -> u16 {
        self.config.flow_pulse_rate
    }

    /// Returns the index (0-indexed) of a master station
    pub fn get_master_station(&self, i: usize) -> station::MasterStationConfig {
        self.config.master_stations[i]
    }

    /// Returns the index (0-indexed) of a master station
    pub fn get_master_station_index(&self, i: usize) -> Option<station::StationIndex> {
        self.config.master_stations[i].station
    }

    pub fn is_master_station(&self, station_index: station::StationIndex) -> bool {
        self.get_master_station_index(0) == Some(station_index) || self.get_master_station_index(1) == Some(station_index)
    }

    pub fn set_water_scale(&mut self, scale: u8) {
        //self.iopts.wl = scale;
        self.config.water_scale = scale;
    }

    /// Update the weather service request success timestamp
    pub fn update_check_weather_success_timestamp(&mut self) {
        self.weather_status.checkwt_success_lasttime = Some(chrono::Utc::now().timestamp());
    }

    pub fn start_flow_log_count(&mut self) {
        self.flow_count_log_start = self.flow_state.get_flow_count();
    }

    pub fn get_flow_log_count(&self) -> u64 {
        // @fixme potential subtraction overflow
        self.flow_state.get_flow_count() - self.flow_count_log_start
    }

    /// Realtime flow count
    pub fn update_realtime_flow_count(&mut self, now_seconds: i64) {
        if self.get_sensor_type(0).unwrap_or(sensor::SensorType::None) == sensor::SensorType::Flow && now_seconds % FLOW_COUNT_REALTIME_WINDOW == 0 {
            self.flow_count_rt = max(0, self.flow_state.get_flow_count() - self.flow_count_rt_start); // @fixme subtraction overflow
            self.flow_count_rt_start = self.flow_state.get_flow_count();
        }
    }

    pub fn check_reboot_request(&mut self, now_seconds: i64) {
        if self.status_current.safe_reboot && (now_seconds > self.status_current.reboot_timer) {
            // if no program is running at the moment and if no program is scheduled to run in the next minute
            if !self.status_current.program_busy && !self.program_pending_soon(now_seconds + 60) {
                self.reboot_dev(self.config.reboot_cause).unwrap();
            }
        } else if self.status_current.reboot_timer != 0 && (now_seconds > self.status_current.reboot_timer) {
            self.reboot_dev(config::RebootCause::Timer).unwrap();
        }
    }

    fn program_pending_soon(&self, timestamp: i64) -> bool {
        for program in self.config.programs.iter() {
            if program.check_match(self, timestamp) {
                return true;
            }
        }
        return false;
    }

    // Calculate local time (UTC time plus time zone offset)
    /* pub fn now_tz(&self) -> u64 {
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
        return now + 3600 / 4 * (self.iopts.tz - 48) as u64;
    } */

    /// @todo Define primary interface e.g. `eth0` and check status (IFF_UP).
    pub fn network_connected(&self) -> bool {
        if cfg!(feature = "demo") {
            return true;
        }

        #[cfg(target_os = "linux")]
        return system::is_interface_online("eth0");

        // @hack default case
        return true;
    }

    pub fn load_hardware_mac() {
        // Use primary interface and get mac from it.
        todo!();
    }

    pub fn reboot_dev(&mut self, cause: config::RebootCause) -> config::result::Result<()> {
        self.config.reboot_cause = cause;
        self.config.write()?;

        if cfg!(not(feature = "demo")) {
            // reboot via commandline, dbus, libc::reboot, etc.
            todo!();
        }

        Ok(())
    }

    /// Update software
    pub fn update_dev() {
        todo!();
    }

    #[cfg(not(feature = "demo"))]
    pub fn flow_poll(&mut self) {
        if let Some(ref gpio) = self.gpio {
            let sensor1_pin = gpio.get(gpio::SENSOR[0]).and_then(|pin| Ok(pin.into_input()));

            if let Err(ref error) = sensor1_pin {
                tracing::error!("GPIO Error (SENSOR[0]): {:?}", error);
            } else if let Ok(pin) = sensor1_pin {
                // Perform calculations using the current state of the sensor
                self.flow_state.poll(pin.read());
            }
        }
    }

    /// Apply all station bits
    ///
    /// **This will actuate valves**
    pub fn apply_all_station_bits(&mut self) {
        #[cfg(not(feature = "demo"))]
        if let Some(ref gpio) = self.gpio {
            let shift_register_latch = gpio.get(gpio::SHIFT_REGISTER_LATCH).and_then(|pin| Ok(pin.into_output()));
            if let Err(ref error) = shift_register_latch {
                tracing::error!("GPIO Error (SHIFT_REGISTER_LATCH): {:?}", error);
            }

            let shift_register_clock = gpio.get(gpio::SHIFT_REGISTER_CLOCK).and_then(|pin| Ok(pin.into_output()));
            if let Err(ref error) = shift_register_clock {
                tracing::error!("GPIO Error (SHIFT_REGISTER_CLOCK): {:?}", error);
            }

            let shift_register_data = gpio.get(gpio::SHIFT_REGISTER_DATA).and_then(|pin| Ok(pin.into_output()));
            if let Err(ref error) = shift_register_data {
                tracing::error!("GPIO Error (SHIFT_REGISTER_DATA): {:?}", error);
            }

            if shift_register_latch.is_ok() && shift_register_clock.is_ok() && shift_register_data.is_ok() {
                let mut shift_register_latch = shift_register_latch.unwrap();
                let mut shift_register_clock = shift_register_clock.unwrap();
                let mut shift_register_data = shift_register_data.unwrap();

                shift_register_latch.set_low();

                // Shift out all station bit values from the highest bit to the lowest
                for board_index in 0..station::MAX_EXT_BOARDS {
                    let sbits = if self.config.enable_controller { self.station_bits[station::MAX_EXT_BOARDS - board_index] } else { 0 };

                    for s in 0..controller::SHIFT_REGISTER_LINES {
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
        }

        if self.config.enable_special_stn_refresh {
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
            if now_seconds >= self.config.rain_delay_stop_time.unwrap_or(0) {
                // rain delay is over
                self.rain_delay_stop();
            }
        } else {
            if self.config.rain_delay_stop_time.unwrap_or(0) > now_seconds {
                // rain delay starts now
                self.rain_delay_start();
            }
        }

        // Check controller status changes and write log
        if self.status_last.rain_delayed != self.status_current.rain_delayed {
            if self.status_current.rain_delayed {
                // rain delay started, record time
                self.raindelay_on_last_time = now_seconds.try_into().unwrap();
                events::push_message(self, &events::RainDelayEvent::new(true));
            } else {
                // rain delay stopped, write log
                let _ = log::write_log_message(&self, &log::message::SensorMessage::new(log::LogDataType::RainDelay, now_seconds), now_seconds);
                events::push_message(self, &events::RainDelayEvent::new(false));
            }
            events::push_message(&self, &events::RainDelayEvent::new(true));
            self.status_last.rain_delayed = self.status_current.rain_delayed;
        }
    }

    #[cfg(not(feature = "demo"))]
    fn detect_sensor_status(&mut self, i: usize, now_seconds: i64) {
        let sensor_type = self.get_sensor_type(i);

        if sensor_type.unwrap_or(sensor::SensorType::None) == sensor::SensorType::Rain || sensor_type.unwrap_or(sensor::SensorType::None) == sensor::SensorType::Soil {
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

    /// Check binary sensor status (e.g. rain, soil)
    #[cfg(not(feature = "demo"))]
    pub fn check_binary_sensor_status(&mut self, now_seconds: i64) {
        for i in 0..sensor::MAX_SENSORS {
            self.detect_sensor_status(i, now_seconds);

            if self.status_last.sensors[i].active != self.status_current.sensors[i].active {
                // send notification when sensor becomes active
                if self.status_current.sensors[i].active {
                    self.sensor_status[i].active_last_time = Some(now_seconds);
                } else {
                    let _ = log::write_log_message(&self, &log::message::SensorMessage::new(log::LogDataType::Sensor1, now_seconds), now_seconds);
                }
                events::push_message(&self, &events::BinarySensorEvent::new(i, self.status_current.sensors[i].active));
            }
            self.status_last.sensors[i].active = self.status_current.sensors[i].active;
        }
    }

    /// Check program switch status
    #[cfg(not(feature = "demo"))]
    pub fn check_program_switch_status(&mut self, program_data: &mut program::ProgramQueue) {
        let program_switch = self.detect_program_switch_status();

        if program_switch.into_iter().any(|d| d == true) {
            // immediately stop all stations
            self.reset_all_stations_immediate(program_data);
        }

        for i in 0..sensor::MAX_SENSORS {
            if self.config.programs.len() > i {
                // Program switch sensors start the same program index
                scheduler::manual_start_program(self, program_data, program::ProgramStart::User(i), false);
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
    #[cfg(not(feature = "demo"))]
    fn get_sensor_detected(&self, i: usize) -> bool {
        let normal_state = self.get_sensor_normal_state(i);

        if let Some(ref gpio) = self.gpio {
            let sensor = gpio.get(gpio::SENSOR[i]).and_then(|pin| Ok(pin.into_input()));

            if let Err(ref error) = sensor {
                tracing::error!("GPIO Error (SENSOR[{}]): {:?}", i, error);
            } else if let Ok(sensor) = sensor {
                return match sensor.read() {
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

        false
    }

    /// Return program switch status
    #[cfg(not(feature = "demo"))]
    pub fn detect_program_switch_status(&mut self) -> [bool; sensor::MAX_SENSORS] {
        let mut detected = [false; sensor::MAX_SENSORS];

        for i in 0..sensor::MAX_SENSORS {
            if self.get_sensor_type(i).unwrap_or(sensor::SensorType::None) == sensor::SensorType::ProgramSwitch {
                self.status_current.sensors[i].detected = self.get_sensor_detected(i);

                self.sensor_status[i].history = (self.sensor_status[i].history << 1) | if self.status_current.sensors[i].detected { 1 } else { 0 };

                // basic noise filtering: only trigger if sensor matches pattern:
                // i.e. two consecutive lows followed by two consecutive highs
                if (self.sensor_status[i].history & 0b1111) == 0b0011 {
                    detected[i] = true;
                }
            }
        }

        detected
    }

    pub fn sensor_reset_all(&mut self) {
        self.sensor_status = [sensor::SensorStatus::default(); sensor::MAX_SENSORS];

        for i in 0..sensor::MAX_SENSORS {
            self.status_last.sensors[i].active = false;
            self.status_current.sensors[i].active = false;
        }
    }

    /// Switch Radio Frequency (RF) station
    ///
    /// This function takes an RF code, parses it into signals and timing, and sends it out through the RF transmitter.
    fn switch_rf_station(&mut self, data: station::RFStationData, value: bool) {
        let code = if value { data.on } else { data.off };

        if let Err(ref error) = rf::send_rf_signal(self, code.into(), data.timing.into()) {
            tracing::error!("[RF Station] Error: {:?}", error);
        }
    }

    /// Switch GPIO station
    ///
    /// Special data for GPIO Station is three bytes of ascii decimal (not hex).
    fn switch_gpio_station(&self, data: station::GPIOStationData, value: bool) {
        tracing::trace!("[GPIO Station] pin: {} state: {}", data.pin, value);

        #[cfg(not(feature = "demo"))]
        if let Some(ref gpio) = self.gpio {
            let pin = gpio.get(data.pin).and_then(|pin| Ok(pin.into_output()));

            if let Err(ref error) = pin {
                tracing::error!("GPIO Error (GPIO Station Pin {}): {:?}", data.pin, error);
            } else if let Ok(mut pin) = pin {
                pin.write(match value {
                    false => !data.active_level(),
                    true => data.active_level(),
                });
            }
        }
    }

    /// Switch Remote Station
    /// This function takes a remote station code, parses it into remote IP, port, station index, and makes a HTTP GET request.
    /// The remote controller is assumed to have the same password as the main controller.
    fn switch_remote_station(&self, data: station::RemoteStationData, value: bool) {
        let mut host = String::from("http://");
        host.push_str(&data.ip.to_string());
        let timer = match self.config.enable_special_stn_refresh {
            true => (station::MAX_NUM_STATIONS * 4) as i64,
            false => 64800, // 18 hours
        };

        // @todo log request failures
        let client = request::build_client().unwrap();
        let response = client.get(host).query(&http::request::RemoteStationRequestParametersV2_1_9::new(&self.config.device_key, data.sid, value, timer)).send();

        if let Err(error) = response {
            tracing::error!("[Remote Station] HTTP request error: {:?}", error);
        }
    }

    /// Switch HTTP station
    ///
    /// This function takes an http station code, parses it into a server name and two HTTP GET requests.
    fn switch_http_station(&self, data: station::HTTPStationData, value: bool) {
        let mut origin: String = String::new();
        origin.push_str(&data.uri);
        if value {
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
    pub fn switch_special_station(&mut self, station_index: station::StationIndex, value: bool) {
        if let Some(station) = self.config.stations.get(station_index) {
            match station.station_type {
                station::StationType::RadioFrequency => self.switch_rf_station(station::RFStationData::try_from(station.sped.as_ref().unwrap()).unwrap(), value),
                station::StationType::Remote => self.switch_remote_station(station::RemoteStationData::try_from(station.sped.as_ref().unwrap()).unwrap(), value),
                station::StationType::GPIO => self.switch_gpio_station(station::GPIOStationData::try_from(station.sped.as_ref().unwrap()).unwrap(), value),
                station::StationType::HTTP => self.switch_http_station(station::HTTPStationData::try_from(station.sped.as_ref().unwrap()).unwrap(), value),
                _ => (), // Nothing to do for [StationType::Standard] and [StationType::Other]
            }
        }
    }

    pub fn get_available_gpio_pins(&self) {
        todo!();
    }

    /// "Factory Reset
    ///
    /// This function should be called if the config does not exist.
    pub fn reset_to_defaults(&self) -> config::result::Result<()> {
        tracing::info!("Resetting controller to defaults.");
        Ok(self.config.write_default()?)
    }

    /// Enable controller operation
    pub fn enable(&mut self) {
        self.config.enable_controller = true;
        self.config.write().unwrap();
    }

    /// Disable controller operation
    pub fn disable(&mut self) {
        self.config.enable_controller = false;
        self.config.write().unwrap();
    }

    /// Start rain delay
    pub fn rain_delay_start(&mut self) {
        self.status_current.rain_delayed = true;
        self.config.write().unwrap();
    }

    /// Stop rain delay
    pub fn rain_delay_stop(&mut self) {
        self.status_current.rain_delayed = false;
        self.config.rain_delay_stop_time = None;
        self.config.write().unwrap();
    }

    /// Set station bit
    ///
    /// This function sets the corresponding station bit. [OpenSprinkler::apply_all_station_bits()] must be called after to apply the bits (which results in physically actuating the valves).
    pub fn set_station_bit(&mut self, station: station::StationIndex, value: bool) -> StationBitChange {
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
        // Determine which rain/soil sensors are currently active
        let mut sn = [false; sensor::MAX_SENSORS];
        for i in 0..sensor::MAX_SENSORS {
            let sensor_type = self.get_sensor_type(i).unwrap_or(sensor::SensorType::None);
            sn[i] = (sensor_type == sensor::SensorType::Rain || sensor_type == sensor::SensorType::Rain) && self.status_current.sensors[i].active;
        }

        for board_index in 0..self.get_board_count() {
            for line in 0..controller::SHIFT_REGISTER_LINES {
                let station_index = board_index * controller::SHIFT_REGISTER_LINES + line;

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

                //if q.program_index >= program::TEST_PROGRAM_ID {
                if q.program_index == program::ProgramStart::Test || q.program_index == program::ProgramStart::TestShort || q.program_index == program::ProgramStart::RunOnce {
                    // This is a manually started program, skip
                    continue;
                }

                // If system is disabled, turn off zone
                if !self.config.enable_controller {
                    controller::turn_off_station(self, program_data, now_seconds, station_index);
                }

                // if rain delay is on and zone does not ignore rain delay, turn it off
                if self.status_current.rain_delayed && !self.config.stations[station_index].attrib.ignore_rain_delay {
                    controller::turn_off_station(self, program_data, now_seconds, station_index);
                }

                for i in 0..sensor::MAX_SENSORS {
                    if sn[i] && !self.config.stations[station_index].attrib.ignore_sensor[i] {
                        controller::turn_off_station(self, program_data, now_seconds, station_index);
                    }
                }
            }
        }
    }
}

impl Default for OpenSprinkler {
    fn default() -> Self {
        Self {
            config: config::Config::default(),
            status_current: ControllerStatus::default(),
            status_last: ControllerStatus::default(),
            weather_status: weather::WeatherStatus::default(),
            raindelay_on_last_time: None,
            sensor_status: [sensor::SensorStatus::default(); sensor::MAX_SENSORS],
            station_bits: [0u8; station::MAX_NUM_BOARDS],
            flow_count_log_start: 0,
            flow_state: sensor::flow::State::default(),
            #[cfg(feature = "mqtt")]
            mqtt: mqtt::Mqtt::new(),
            gpio: None,
            boot_time: chrono::Utc::now(),
            sar__next_sid_to_refresh: station::MAX_NUM_STATIONS >> 1,
            sar__last_now: 0,
            flow_count_rt: 0,
            flow_count_rt_start: 0,
        }
    }
}

#[derive(PartialEq)]
pub enum StationBitChange {
    NoChange = 0,
    On = 1,
    Off = 255,
}
