use rppal::gpio::OutputPin;
use serde::{Deserialize, Serialize};
use std::cmp::max;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::{path::Path, time::SystemTime};

use crate::opensprinkler::sensor::SensorOption;

use self::config::ConfigDocument;
use self::program::Programs;
use self::sensor::{SensorType, MAX_SENSORS};
use self::station::{Station, StationType, Stations, MAX_NUM_BOARDS, SHIFT_REGISTER_LINES};

pub mod config;
#[cfg(feature = "demo")]
mod demo;
pub mod events;
pub mod gpio;
pub mod log;
pub mod loop_fns;
#[cfg(feature = "mqtt")]
mod mqtt;
pub mod program;
mod rf;
pub mod sensor;
pub mod station;
#[cfg(target_os = "linux")]
pub mod system;
pub mod weather;

/// Default reboot timer (seconds)
pub const REBOOT_DELAY: i64 = 65;

pub const MINIMUM_ON_DELAY: u8 = 5;
pub const MINIMUM_OFF_DELAY: u8 = 5;

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
    /// HIGH means a program is being executed currently
    pub program_busy: bool,
    /// HIGH means a safe reboot has been marked
    pub safe_reboot: bool,
    /// master station index
    pub mas: Option<usize>,
    /// master2 station index
    pub mas2: Option<usize>,
    // sensor2 status bit (when set, sensor2 on is detected)
    //pub sensor2: bool,
    // sensor1 active bit (when set, sensor1 is activated)
    //pub sensor1_active: bool,
    // sensor2 active bit (when set, sensor2 is activated)
    //pub sensor2_active: bool,
    /// request mqtt restart
    pub req_mqtt_restart: bool,

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
            mas: None,
            mas2: None,
            //sensor2: false,
            //sensor1_active: false,
            //sensor2_active: false,
            req_mqtt_restart: false,
            reboot_timer: 0,

            sensors: [ControllerSensorStatus { detected: false, active: false }, ControllerSensorStatus { detected: false, active: false }],
        }
    }
}

#[derive(Default)]
pub struct WeatherStatus {
    /// time when weather was checked (seconds)
    pub checkwt_lasttime: Option<i64>,

    /// time when weather check was successful (seconds)
    pub checkwt_success_lasttime: Option<i64>,

    /// Result of the most recent request to the weather service
    pub last_response_code: Option<i8>,

    /// Data returned by the weather service (used by web server)
    pub raw_data: Option<String>,
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
const FLOW_COUNT_RT_WINDOW: u8 = 30;

const HARDWARE_VERSION: u8 = HardwareVersionBase::OpenSprinklerPi as u8;

// @todo Get firmware version from cargo
const FIRMWARE_VERSION: u16 = 300;
const FIRMWARE_VERSION_REVISION: u8 = 0;

pub struct OpenSprinkler {
    config: config::Config,
    pub controller_config: config::ConfigDocument,

    #[cfg(not(feature = "demo"))]
    gpio: rppal::gpio::Gpio,

    #[cfg(feature = "mqtt")]
    mqtt: mqtt::OSMqtt,

    pub status: ControllerStatus,
    pub old_status: ControllerStatus,

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
    pub flow_count_log_start: u32,

    // flow count (for computing real-time flow rate)
    pub flowcount_rt: u32,

    /// Weather service status
    pub weather_status: WeatherStatus,

    /// time when controller is powered up most recently (seconds)
    powerup_lasttime: Option<i64>,

    /// Last reboot cause
    last_reboot_cause: RebootCause,
}

impl OpenSprinkler {
    pub fn new(config_path: PathBuf) -> OpenSprinkler {
        //let nboards = 1;

        //let stations = station::default();
        //let programs = Vec::new();

        let gpio = rppal::gpio::Gpio::new();
        if let Err(ref error) = gpio {
            tracing::error!("Failed to obtain GPIO chip: {:?}", error);
        }

        if gpio.is_ok() {
            OpenSprinkler::setup_gpio_pins(gpio.as_ref().unwrap());
        }

        OpenSprinkler {
            config: config::Config::new(config_path),
            controller_config: config::ConfigDocument::default(),

            #[cfg(not(feature = "demo"))]
            gpio: gpio.unwrap(),

            #[cfg(feature = "mqtt")]
            mqtt: mqtt::OSMqtt::new(),

            status: ControllerStatus::default(),
            old_status: ControllerStatus::default(),
            sensor_status: sensor::init_vec(),
            //nboards,
            //nstations: nboards * SHIFT_REGISTER_LINES,
            station_bits: [0u8; MAX_NUM_BOARDS],
            powerup_lasttime: None,
            raindelay_on_last_time: None,
            flow_count_log_start: 0,
            flowcount_rt: 0,

            weather_status: WeatherStatus::default(),

            // Initalize defaults
            //nvdata: config::data_type::ControllerNonVolatile::default(),
            //iopts: config::data_type::IntegerOptions::default(),
            //sopts: config::data_type::StringOptions::default(),
            //stations,
            //programs,
            last_reboot_cause: RebootCause::None,
        }
    }

    fn setup_gpio_pins(gpio: &rppal::gpio::Gpio) {
        if let Err(ref error) = gpio.get(gpio::pin::SHIFT_REGISTER_OE).and_then(|pin| Ok(pin.into_output().set_high())) {
            tracing::error!("Failed to obtain output pin SHIFT_REGISTER_OE: {:?}", error);
        }
        if let Err(ref error) = gpio.get(gpio::pin::SHIFT_REGISTER_LATCH).and_then(|pin| Ok(pin.into_output().set_high())) {
            tracing::error!("Failed to obtain output pin SHIFT_REGISTER_LATCH: {:?}", error);
        }
        if let Err(ref error) = gpio.get(gpio::pin::SHIFT_REGISTER_CLOCK).and_then(|pin| Ok(pin.into_output().set_high())) {
            tracing::error!("Failed to obtain output pin SHIFT_REGISTER_CLOCK: {:?}", error);
        }
        if let Err(ref error) = gpio.get(gpio::pin::SHIFT_REGISTER_DATA).and_then(|pin| Ok(pin.into_output().set_high())) {
            tracing::error!("Failed to obtain output pin SHIFT_REGISTER_DATA: {:?}", error);
        }
        if let Err(ref error) = gpio.get(gpio::pin::SENSOR_1).and_then(|pin| Ok(pin.into_input_pullup().set_reset_on_drop(false))) {
            // @todo Catch abnormal process terminations and reset pullup
            tracing::error!("Failed to obtain input pin SENSOR_1: {:?}", error);
        }
        if let Err(ref error) = gpio.get(gpio::pin::SENSOR_2).and_then(|pin| Ok(pin.into_input_pullup().set_reset_on_drop(false))) {
            // @todo Catch abnormal process terminations and reset pullup
            tracing::error!("Failed to obtain input pin SENSOR_2: {:?}", error);
        }
        if let Err(ref error) = gpio.get(gpio::pin::RF_TX).and_then(|pin| Ok(pin.into_output().set_low())) {
            tracing::error!("Failed to obtain output pin RF_TX: {:?}", error);
        }
    }

    // region: GETTERS
    pub fn is_logging_enabled(&self) -> bool {
        //self.iopts.lg == 1
        self.controller_config.iopts.lg
    }

    pub fn is_mqtt_enabled(&self) -> bool {
        true
        //self.sopts.mqtt
    }

    pub fn is_remote_extension(&self) -> bool {
        //self.iopts.re == 1
        self.controller_config.iopts.re
    }

    pub fn get_water_scale(&self) -> u8 {
        //self.iopts.wl
        self.controller_config.iopts.wl
    }

    pub fn get_sunrise_time(&self) -> u16 {
        //self.nvdata.sunrise_time
        self.controller_config.nv.sunrise_time
    }

    pub fn get_sunset_time(&self) -> u16 {
        //self.nvdata.sunset_time
        self.controller_config.nv.sunset_time
    }

    /// Number of eight-zone station boards (including master controller)
    pub fn get_board_count(&self) -> usize {
        //self.nboards
        //self.iopts.ext + 1
        self.controller_config.iopts.ext + 1
    }

    pub fn get_station_count(&self) -> usize {
        self.get_board_count() * SHIFT_REGISTER_LINES
    }

    pub fn is_station_running(&self, station_index: usize) -> bool {
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
            self.controller_config.iopts.sn1t
        } else if i == 1 {
            //self.iopts.sn2t
            self.controller_config.iopts.sn2t
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

    pub fn get_sensor_option(&self, i: usize) -> SensorOption {
        // sensor_option: 0 if normally closed; 1 if normally open
        match i {
            //0 => self.iopts.sn1o,
            0 => self.controller_config.iopts.sn1o,
            //1 => self.iopts.sn2o,
            1 => self.controller_config.iopts.sn2o,
            _ => unreachable!(),
        }
    }

    pub fn get_sensor_on_delay(&self, i: usize) -> u8 {
        match i {
            0 => self.controller_config.iopts.sn1on,
            1 => self.controller_config.iopts.sn2on,
            _ => unreachable!(),
        }
    }

    pub fn get_sensor_off_delay(&self, i: usize) -> u8 {
        match i {
            0 => self.controller_config.iopts.sn1of,
            1 => self.controller_config.iopts.sn2of,
            _ => unreachable!(),
        }
    }

    pub fn get_flow_pulse_rate(&self) -> u16 {
        //(u16::from(self.iopts.fpr1) << 8) + u16::from(self.iopts.fpr0)
        (u16::from(self.controller_config.iopts.fpr1) << 8) + u16::from(self.controller_config.iopts.fpr0)
    }
    // endregion GETTERS

    // region: SETTERS

    pub fn set_water_scale(&mut self, scale: u8) {
        //self.iopts.wl = scale;
        self.controller_config.iopts.wl = scale;
    }

    /// Update the weather service request success timestamp
    pub fn set_check_weather_success_timestamp(&mut self) {
        self.weather_status.checkwt_success_lasttime = Some(chrono::Utc::now().timestamp());
    }
    // endregion SETTERS

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
        let _port: u16 = if cfg!(demo) { 80 } else { (self.controller_config.iopts.hp1 as u16) << 8 + &self.controller_config.iopts.hp0.into() };

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
        self.controller_config.nv.reboot_cause = cause;
        self.nvdata_save();

        if cfg!(not(demo)) {
            // @todo reboot via commandline, dbus, libc::reboot, etc.
        }
    }

    // @todo Implement crate *self_update* for updates
    //pub fn update_dev() {}

    /// Apply all station bits
    ///
    /// **This will actuate valves**
    pub fn apply_all_station_bits(&mut self) {
        #[cfg(not(feature = "demo"))]
        let mut shift_register_latch = self.gpio.get(gpio::pin::SHIFT_REGISTER_LATCH).and_then(|pin| Ok(pin.into_output()));
        #[cfg(feature = "demo")]
        let mut shift_register_latch = demo::get_gpio_pin(gpio::pin::SHIFT_REGISTER_LATCH);
        if let Err(ref error) = shift_register_latch {
            tracing::error!("Failed to obtain output pin shift_register_latch: {:?}", error);
        }

        #[cfg(not(feature = "demo"))]
        let mut shift_register_clock = self.gpio.get(gpio::pin::SHIFT_REGISTER_CLOCK).and_then(|pin| Ok(pin.into_output()));
        #[cfg(feature = "demo")]
        let mut shift_register_clock = demo::get_gpio_pin(gpio::pin::SHIFT_REGISTER_CLOCK);
        if let Err(ref error) = shift_register_clock {
            tracing::error!("Failed to obtain output pin shift_register_clock: {:?}", error);
        }

        #[cfg(not(feature = "demo"))]
        let mut shift_register_data = self.gpio.get(gpio::pin::SHIFT_REGISTER_DATA).and_then(|pin| Ok(pin.into_output()));
        #[cfg(feature = "demo")]
        let mut shift_register_data = demo::get_gpio_pin(gpio::pin::SHIFT_REGISTER_DATA);
        if let Err(ref error) = shift_register_data {
            tracing::error!("Failed to obtain output pin shift_register_data: {:?}", error);
        }

        if shift_register_latch.is_ok() && shift_register_clock.is_ok() && shift_register_data.is_ok() {
            shift_register_latch.as_mut().and_then(|pin| Ok(pin.set_low()));

            // Shift out all station bit values from the highest bit to the lowest
            for board_index in 0..station::MAX_EXT_BOARDS {
                let sbits = if self.status.enabled { self.station_bits[station::MAX_EXT_BOARDS - board_index] } else { 0 };

                for s in 0..SHIFT_REGISTER_LINES {
                    shift_register_clock.as_mut().and_then(|pin| Ok(pin.set_low()));

                    if sbits & (1 << (7 - s)) != 0 {
                        shift_register_data.as_mut().and_then(|pin| Ok(pin.set_high()));
                        shift_register_data.as_mut().and_then(|pin| Ok(pin.set_low()));
                    }

                    shift_register_clock.as_mut().and_then(|pin| Ok(pin.set_high()));
                }
            }

            shift_register_latch.as_mut().and_then(|pin| Ok(pin.set_high()));
        }

        //if self.iopts.sar == 1 {
        if self.controller_config.iopts.sar {
            // Handle refresh of special stations. We refresh station that is next in line

            let mut next_sid_to_refresh = station::MAX_NUM_STATIONS >> 1;
            let mut last_now = 0;
            let now = chrono::Utc::now().timestamp();

            if now > last_now {
                // Perform this no more than once per second
                // @fixme variable lifetime
                last_now = now;
                next_sid_to_refresh = (next_sid_to_refresh + 1) % station::MAX_NUM_STATIONS;
                let board_index = next_sid_to_refresh >> 3;
                let s = next_sid_to_refresh & 0x07;
                self.switch_special_station(next_sid_to_refresh, (self.station_bits[board_index] >> s) & 0x01 != 0);
            }
        }
    }

    fn detect_sensor_status(&mut self, i: usize, now_seconds: i64) {
        let sensor_type = self.get_sensor_type(i);

        if sensor_type == SensorType::Rain || sensor_type == SensorType::Soil {
            self.status.sensors[i].detected = self.get_sensor_detected(i);

            if self.status.sensors[i].detected {
                if self.sensor_status[i].on_timer.is_none() {
                    // add minimum of 5 seconds on-delay
                    self.sensor_status[i].on_timer = Some(max(self.get_sensor_on_delay(i) * 60, MINIMUM_ON_DELAY).into());
                    self.sensor_status[i].off_timer = Some(0);
                } else {
                    if now_seconds > self.sensor_status[i].on_timer.unwrap_or(0) {
                        self.status.sensors[i].active = true;
                    }
                }
            } else {
                if self.sensor_status[i].off_timer.is_none() {
                    // add minimum of 5 seconds off-delay
                    self.sensor_status[i].off_timer = Some(max(self.get_sensor_off_delay(i) * 60, MINIMUM_OFF_DELAY).into());
                    self.sensor_status[i].on_timer = Some(0);
                } else {
                    if now_seconds > self.sensor_status[i].off_timer.unwrap_or(0) {
                        self.status.sensors[i].active = false;
                    }
                }
            }
        }
    }

    /// Read sensor status
    pub fn detect_binary_sensor_status(&mut self, now_seconds: i64) {
        if cfg!(use_sensor_1) {
            self.detect_sensor_status(0, now_seconds);
        }

        if cfg!(use_sensor_2) {
            self.detect_sensor_status(1, now_seconds);
        }
    }

    /// Get sensor detected
    ///
    /// If sensor is "normally open" -
    fn get_sensor_detected(&self, i: usize) -> bool {
        let sensor_option = self.get_sensor_option(i);

        let pin = match i {
            0 => gpio::pin::SENSOR_1,
            1 => gpio::pin::SENSOR_2,
            _ => unreachable!(),
        };

        tracing::trace!("Reading sensor {}@bcm_pin_{} ({})", i, pin, if sensor_option == SensorOption::NormallyClosed { "NC" } else { "NO" });

        #[cfg(not(feature = "demo"))]
        let sensor_pin = self.gpio.get(pin).and_then(|pin| Ok(pin.into_input()));

        #[cfg(feature = "demo")]
        let sensor_pin = demo::get_gpio_pin(pin);

        if let Err(ref error) = sensor_pin {
            tracing::error!("Failed to obtain sensor input pin (flow): {:?}", error);
            return false;
        } else {
            return match sensor_pin.unwrap().read() {
                rppal::gpio::Level::Low => {
                    /* if sensor_option == SensorOption::NormallyOpen {
                        false
                    } else {
                        true
                    } */
                    match sensor_option {
                        SensorOption::NormallyClosed => true,
                        SensorOption::NormallyOpen => false,
                    }
                }
                rppal::gpio::Level::High => {
                    /* if sensor_option == SensorOption::NormallyClosed {
                        false
                    } else {
                        true
                    } */
                    match sensor_option {
                        SensorOption::NormallyClosed => false,
                        SensorOption::NormallyOpen => true,
                    }
                }
            };
        }
    }

    /// Return program switch status
    pub fn detect_program_switch_status(&mut self) -> [bool; MAX_SENSORS] {
        let mut ret = [false, false];

        for i in 0..MAX_SENSORS {
            if self.get_sensor_type(i) == SensorType::ProgramSwitch {
                self.status.sensors[i].detected = self.get_sensor_detected(i);

                self.sensor_status[i].history = (self.sensor_status[i].history << 1) | if self.status.sensors[i].detected { 1 } else { 0 };

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

        self.old_status.sensors[0].active = false;
        self.status.sensors[0].active = false;
        self.old_status.sensors[1].active = false;
        self.status.sensors[1].active = false;
    }

    /// Switch Radio Frequency (RF) station
    ///
    /// This function takes an RF code, parses it into signals and timing, and sends it out through the RF transmitter.
    fn switch_rf_station(&mut self, data: station::RFStationData, turn_on: bool) {
        //let (on, off, length) = self.parse_rf_station_code(data);
        let code = if turn_on { data.on } else { data.off };
        rf::send_rf_signal(self, code.into(), data.timing.into());
    }

    /// Switch GPIO station
    ///
    /// Special data for GPIO Station is three bytes of ascii decimal (not hex).
    fn switch_gpio_station(&self, data: station::GPIOStationData, state: bool) {
        tracing::trace!("Switching GPIO station {} {}", data.pin, state);
        #[cfg(not(feature = "demo"))]
        {
            let pin = self.gpio.get(data.pin).and_then(|pin| Ok(pin.into_output()));
            if let Err(ref error) = pin {
                tracing::error!("Failed to obtain output pin {} gpio_station: {:?}", data.pin, error);
                return;
            }

            if state {
                if data.active {
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
            }
        }
    }

    /// Switch Remote Station
    /// This function takes a remote station code, parses it into remote IP, port, station index, and makes a HTTP GET request.
    /// The remote controller is assumed to have the same password as the main controller.
    fn switch_remote_station(&self, data: station::RemoteStationData, turn_on: bool) {
        let ip4 = Ipv4Addr::from(data.ip);
        //let timer = match self.iopts.sar {
        let timer = if self.controller_config.iopts.sar {
            station::MAX_NUM_STATIONS * 4
        } else {
            // 18 hours
            64800
        };
        let en = if turn_on { String::from("1") } else { String::from("0") };

        let client = reqwest::blocking::Client::new();
        // @todo log request failures
        let _ = client
            .get(ip4.to_string())
            .query(&[
                // Device key (MD5)
                //("pw", self.sopts.dkey.clone()),
                ("pw", self.controller_config.sopts.dkey.clone()),
                // Station ID/index
                ("sid", data.sid.to_string()),
                // Enable bit
                ("en", en),
                // Timer (seconds)
                ("t", timer.to_string()),
            ])
            .send()
            .expect("Error making remote station request");
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
        let _ = reqwest::blocking::get(origin).expect("Error making HTTP station request");
    }

    /// Switch Special Station
    pub fn switch_special_station(&mut self, station_index: usize, value: bool) {
        //let station = self.stations.get(station_index).unwrap();
        let station = self.controller_config.stations.get(station_index).unwrap();
        //let station_type = self.get_station_type(station);
        // check if station is "special"
        if station.r#type == StationType::Standard {
            return ();
        }

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
        let config = self.config.get::<ConfigDocument>();
        if let Err(error) = config {
            tracing::error!("Error reading config: {:?}", error);
            self.reset_to_defaults();
            return;
        }

        // Check reset conditions
        if config.is_ok() {
            let config = config.unwrap();

            if config.iopts.fwv < FIRMWARE_VERSION {
                tracing::debug!("Invalid firmware version: {:?}", config.iopts.fwv);
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
        self.controller_config.iopts.fwv = FIRMWARE_VERSION;
        //self.iopts.fwm = FIRMWARE_VERSION_REVISION;
        self.controller_config.iopts.fwm = FIRMWARE_VERSION_REVISION;
        //};
        //{
        //let ref mut this = self;
        //self.nvdata = config::get_controller_nv().unwrap();

        self.old_status = self.status;
        //};
        //self.last_reboot_cause = self.nvdata.reboot_cause;
        self.last_reboot_cause = self.controller_config.nv.reboot_cause;
        //self.nvdata.reboot_cause = RebootCause::PowerOn;
        self.controller_config.nv.reboot_cause = RebootCause::PowerOn;
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
        self.status.enabled = self.controller_config.iopts.den;
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
        self.status.enabled = true;
        //self.iopts.den = 1;
        self.controller_config.iopts.den = true;
        self.iopts_save();
    }

    /// Disable controller operation
    pub fn disable(&mut self) {
        self.status.enabled = false;
        //self.iopts.den = 0;
        self.controller_config.iopts.den = false;
        self.iopts_save();
    }

    /// Start rain delay
    pub fn rain_delay_start(&mut self) {
        self.status.rain_delayed = true;
        self.nvdata_save();
    }

    /// Stop rain delay
    pub fn rain_delay_stop(&mut self) {
        self.status.rain_delayed = false;
        //self.nvdata.rd_stop_time = None;
        self.controller_config.nv.rd_stop_time = None;
        self.nvdata_save();
    }

    /// Set station bit
    ///
    /// This function sets the corresponding station bit. [OpenSprinkler::apply_all_station_bits()] must be called after to apply the bits (which results in physically actuating the valves).
    pub fn set_station_bit(&mut self, station: usize, value: bool) -> StationBitChange {
        // Pointer to the station byte
        let data = self.station_bits[(station >> 3) as usize];
        // Mask
        let mask = 1 << (station & 0x07);

        if value == true {
            if (data & mask) == 1 {
                // If bit is already set, return "no change"
                return StationBitChange::NoChange;
            } else {
                self.station_bits[(station >> 3) as usize] = data | mask;
                // Handle special stations
                self.switch_special_station(station, true);
                return StationBitChange::On;
            }
        } else {
            if (data & mask) == 0 {
                // If bit is already set, return "no change"
                return StationBitChange::NoChange;
            } else {
                self.station_bits[(station >> 3) as usize] = data & !mask;
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
}

#[derive(PartialEq)]
pub enum StationBitChange {
    NoChange = 0,
    On = 1,
    Off = 255,
}
