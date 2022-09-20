pub mod cli;

use super::{events, program, sensor, station, weather};
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    fs::OpenOptions,
    io::{self, Write},
    path::PathBuf,
    str::FromStr,
};

use crate::{
    opensprinkler::events::{ifttt, mqtt},
    server::legacy::{FromLegacyFormat, IntoLegacyFormat},
    utils,
};

#[cfg(unix)]
const CONFIG_FILE_PATH: &'static str = "/etc/opt/opensprinkler/config.dat";

#[cfg(not(unix))]
const CONFIG_FILE_PATH: &'static str = "./config.dat";

#[derive(Clone, Serialize, Deserialize)]
#[repr(u8)]
pub enum HardwareVersionBase {
    OpenSprinkler = 0x00,
    OpenSprinklerPi = 0x40,
    Simulated = 0xC0,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[repr(u8)]
pub enum RebootCause {
    /// None
    None = 0,
    /// Factory Reset
    Reset = 1,
    /// Hardware Button
    Button = 2,
    Timer = 4,
    Web = 5,
    FirmwareUpdate = 7,
    WeatherFail = 8,
    NetworkFail = 9,
    Program = 11,
    PowerOn = 99,
}

impl Default for RebootCause {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Location {
    lat: f32,
    lng: f32,
}

impl Location {
    pub fn new(lat: f32, lng: f32) -> Self {
        Self { lat, lng }
    }
}

impl TryFrom<&str> for Location {
    type Error = result::ParseLocationError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if let Some((lat, lng)) = value.split_once(',') {
            return Ok(Location {
                lat: f32::from_str(lat)?,
                lng: f32::from_str(lng)?,
            });
        }

        Err(result::ParseLocationError::Invalid)
    }
}

impl TryFrom<String> for Location {
    type Error = result::ParseLocationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if let Some((lat, lng)) = value.split_once(',') {
            return Ok(Location {
                lat: f32::from_str(lat)?,
                lng: f32::from_str(lng)?,
            });
        }

        Err(result::ParseLocationError::Invalid)
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 4 decimal places gives ≈10 meters of accuracy and should be enough for this use case
        write!(f, "{:.4},{:.4}", self.lat, self.lng)
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EventsEnabled {
    pub program_start: bool,
    pub sensor1: bool,
    pub flow_sensor: bool,
    pub weather_update: bool,
    pub reboot: bool,
    pub station_off: bool,
    pub sensor2: bool,
    pub rain_delay: bool,
    pub station_on: bool,
}

impl FromLegacyFormat for EventsEnabled {
    type Format = u8;

    fn from_legacy_format(flags: Self::Format) -> Self {
        Self {
            program_start: utils::get_bit_flag_bool(flags, 0),
            sensor1: utils::get_bit_flag_bool(flags, 1),
            flow_sensor: utils::get_bit_flag_bool(flags, 2),
            weather_update: utils::get_bit_flag_bool(flags, 3),
            reboot: utils::get_bit_flag_bool(flags, 4),
            station_off: utils::get_bit_flag_bool(flags, 5),
            sensor2: utils::get_bit_flag_bool(flags, 6),
            rain_delay: utils::get_bit_flag_bool(flags, 7),
            station_on: false,
        }
    }
}

impl IntoLegacyFormat for EventsEnabled {
    type Format = u8;

    fn into_legacy_format(&self) -> Self::Format {
        let mut flags = 0;

        if self.program_start {
            flags = utils::apply_bit_flag(flags, 0, 1);
        }

        if self.sensor1 {
            flags = utils::apply_bit_flag(flags, 1, 1);
        }

        if self.flow_sensor {
            flags = utils::apply_bit_flag(flags, 2, 1);
        }

        if self.weather_update {
            flags = utils::apply_bit_flag(flags, 3, 1);
        }

        if self.reboot {
            flags = utils::apply_bit_flag(flags, 4, 1);
        }

        if self.station_off {
            flags = utils::apply_bit_flag(flags, 5, 1);
        }

        if self.sensor2 {
            flags = utils::apply_bit_flag(flags, 6, 1);
        }

        if self.rain_delay {
            flags = utils::apply_bit_flag(flags, 7, 1);
        }

        flags
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    /// firmware version
    pub firmware_version: semver::Version,
    /// Hardware version
    pub hardware_version: HardwareVersionBase,
    /// number of 8-station extension board. 0: no extension boards
    pub extension_board_count: usize,
    /// Enable controller
    pub enable_controller: bool,
    /// Enable remote extension mode
    pub enable_remote_ext_mode: bool,
    /// Enable logging
    pub enable_log: bool,
    /// Event logging config
    pub event_log: events::log::Config,
    /// Reboot Cause
    pub reboot_cause: RebootCause,
    /// Device key AKA password
    pub device_key: String,
    /// Javascript URL for the web app
    pub js_url: String,
    /// Device location (decimal coordinates)
    pub location: Location,
    /// Default: UTC
    pub timezone: u8,
    /// Weather config
    pub weather: weather::WeatherConfig,
    /// Sunrise time (minutes)
    pub sunrise_time: u16,
    /// Sunset time (minutes)
    pub sunset_time: u16,
    /// Rain-delay stop time (seconds since unix epoch)
    pub rain_delay_stop_time: Option<i64>,
    /// water level (default 100%)
    pub water_scale: f32,
    /// Stations
    pub stations: station::Stations,
    /// station delay time (-10 minutes to 10 minutes).
    pub station_delay_time: u8,
    /// Master stations
    pub master_stations: [station::MasterStationConfig; station::MAX_MASTER_STATIONS],
    /// Special station auto refresh
    pub enable_special_stn_refresh: bool,
    /// Programs
    pub programs: program::Programs,
    /// Sensors
    pub sensors: [sensor::SensorConfig; sensor::MAX_SENSORS],
    /// Flow pulse rate (100x)
    pub flow_pulse_rate: u16,
    /// IFTTT config
    pub ifttt: ifttt::Config,
    /// MQTT config
    pub mqtt: mqtt::Config,

    /* Fields that are never serialized/deserialized */
    /// Config path
    #[serde(skip)]
    path: PathBuf,
}

impl Config {
    pub fn new(path: PathBuf) -> Self {
        Self { path, ..Self::default() }
    }

    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    pub fn read(&self) -> result::Result<Config> {
        tracing::debug!("Read: {:?}", self.path.canonicalize().unwrap_or(self.path.clone()));
        let reader = io::BufReader::new(OpenOptions::new().read(true).open(&self.path)?);
        Ok(Config {
            // Returning the config directly does not include it's path
            path: self.path.clone(),
            ..bson::from_reader(reader)?
        })
    }

    pub fn write(&self) -> result::Result<()> {
        let path = self.path.canonicalize().unwrap_or(self.path.clone());

        tracing::debug!("write({:?}): {}", path, serde_json::to_string(self).unwrap());
        let buf = bson::to_vec(&self)?;
        Ok(io::BufWriter::new(OpenOptions::new().write(true).create(true).open(&path)?).write_all(&buf)?)
    }

    pub fn write_default(&self) -> result::Result<()> {
        let path = self.path.canonicalize().unwrap_or(self.path.clone());

        tracing::debug!("Write default: {:?}", path);
        let buf = bson::to_vec(&Self::default())?;
        Ok(io::BufWriter::new(OpenOptions::new().write(true).create(true).truncate(true).open(&path)?).write_all(&buf)?)
    }

    pub fn check(&self) -> result::Result<bool> {
        // @todo What about higher version numbers?
        if self.firmware_version < Config::default().firmware_version {
            // @todo Migrate config based on existing version
            tracing::debug!("Invalid firmware version: {:?}", self.firmware_version);
            return Ok(false);
        }

        tracing::debug!("Config is OK");
        Ok(true)
    }

    /// Returns a master station
    pub fn get_master_station(&self, i: usize) -> station::MasterStationConfig {
        self.master_stations[i]
    }

    /// Returns the index (0-indexed) of a master station
    pub fn get_master_station_index(&self, i: usize) -> Option<station::StationIndex> {
        self.master_stations[i].station
    }

    pub fn is_master_station(&self, station_index: station::StationIndex) -> bool {
        self.get_master_station_index(0) == Some(station_index) || self.get_master_station_index(1) == Some(station_index)
    }

    pub fn is_logging_enabled(&self) -> bool {
        self.enable_log
    }

    pub fn is_mqtt_enabled(&self) -> bool {
        self.mqtt.enabled
    }

    pub fn is_remote_extension(&self) -> bool {
        self.enable_remote_ext_mode
    }

    pub fn set_water_scale(&mut self, scale: f32) {
        self.water_scale = scale;
    }

    pub fn get_water_scale(&self) -> f32 {
        self.water_scale
    }

    /// Gets the weather service URL (with adjustment method)
    pub fn get_weather_service_url(&self) -> Result<Option<reqwest::Url>, url::ParseError> {
        if let Some(algorithm) = &self.weather.algorithm {
            let mut url = url::Url::parse(&self.weather.service_url)?;
            if let Ok(mut path_seg) = url.path_segments_mut() {
                path_seg.push(&algorithm.get_id().to_string());
            }
            return Ok(Some(url));
        }
        return Ok(None);
    }

    pub fn get_sunrise_time(&self) -> u16 {
        self.sunrise_time
    }

    pub fn get_sunset_time(&self) -> u16 {
        self.sunset_time
    }

    /// Number of eight-zone station boards (including master controller)
    pub fn get_board_count(&self) -> usize {
        self.extension_board_count + 1
    }

    pub fn get_station_count(&self) -> usize {
        self.get_board_count() * station::SHIFT_REGISTER_LINES
    }

    pub fn get_sensor_type(&self, i: usize) -> Option<sensor::SensorType> {
        self.sensors[i].sensor_type
    }

    pub fn get_sensor_normal_state(&self, i: usize) -> sensor::NormalState {
        self.sensors[i].normal_state
    }

    pub fn get_sensor_on_delay(&self, i: usize) -> u8 {
        self.sensors[i].delay_on
    }

    pub fn get_sensor_off_delay(&self, i: usize) -> u8 {
        self.sensors[i].delay_off
    }

    pub fn get_flow_pulse_rate(&self) -> u16 {
        self.flow_pulse_rate
    }

    pub fn is_flow_sensor_enabled(&self) -> bool {
        self.get_sensor_type(0) == Some(sensor::SensorType::Flow)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            firmware_version: semver::Version::parse(core::env!("CARGO_PKG_VERSION")).unwrap(),
            hardware_version: HardwareVersionBase::OpenSprinklerPi,
            extension_board_count: 0,
            enable_controller: true,
            enable_remote_ext_mode: false,
            enable_log: true,
            event_log: events::log::Config::default(),
            reboot_cause: RebootCause::Reset,                              // If the config file does not exist, these defaults will be used. Therefore, this is the relevant reason.
            device_key: format!("{:x}", md5::compute(b"opendoor")).into(), // @todo Use modern hash like Argon2
            js_url: core::option_env!("JAVASCRIPT_URL").unwrap_or("https://ui.opensprinkler.com/js").into(),
            location: Location::default(),
            timezone: 48, // UTC
            weather: weather::WeatherConfig::default(),
            sunrise_time: 360, // 0600 default sunrise
            sunset_time: 1080, // 1800 default sunrise
            rain_delay_stop_time: None,
            water_scale: 1.0,
            stations: station::default(),
            station_delay_time: 120,
            master_stations: [station::MasterStationConfig::default(); station::MAX_MASTER_STATIONS],
            enable_special_stn_refresh: false,
            programs: Vec::new(),
            sensors: [sensor::SensorConfig::default(); sensor::MAX_SENSORS],
            flow_pulse_rate: 100,
            ifttt: ifttt::Config::default(),
            mqtt: mqtt::Config::default(),

            /* Fields that are never serialized/deserialized */
            path: PathBuf::from_str(CONFIG_FILE_PATH).unwrap(),
        }
    }
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", serde_json::to_string_pretty(&self).unwrap_or(String::from("{}")))
    }
}

pub mod result {
    use core::{fmt, num};
    use std::{error, io, sync::Arc};

    pub type Result<T> = core::result::Result<T, Error>;

    #[derive(Clone, Debug)]
    #[non_exhaustive]
    pub enum Error {
        Io(Arc<io::Error>),

        #[non_exhaustive]
        SerializationError(Arc<bson::ser::Error>),

        #[non_exhaustive]
        DeserializationError(Arc<bson::de::Error>),
    }

    impl fmt::Display for Error {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Error::Io(ref err) => write!(f, "IO Error: {:?}", err),
                Error::SerializationError(ref err) => write!(f, "BSON Serialization Error: {:?}", err),
                Error::DeserializationError(ref err) => write!(f, "BSON Deserialization Error: {:?}", err),
            }
        }
    }

    impl From<bson::ser::Error> for Error {
        fn from(err: bson::ser::Error) -> Error {
            Error::SerializationError(Arc::new(err))
        }
    }

    impl From<bson::de::Error> for Error {
        fn from(err: bson::de::Error) -> Error {
            Error::DeserializationError(Arc::new(err))
        }
    }

    impl From<io::Error> for Error {
        fn from(err: io::Error) -> Error {
            Error::Io(Arc::new(err))
        }
    }

    #[derive(Debug)]
    pub enum ParseLocationError {
        Invalid,
        ParseFloatError(num::ParseFloatError),
    }

    impl fmt::Display for ParseLocationError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match *self {
                Self::Invalid => write!(f, "Invalid location string"),
                ParseLocationError::ParseFloatError(ref err) => write!(f, "Float Parse Error: {}", err),
            }
        }
    }

    impl error::Error for ParseLocationError {}

    impl From<std::num::ParseFloatError> for ParseLocationError {
        fn from(error: std::num::ParseFloatError) -> Self {
            ParseLocationError::ParseFloatError(error)
        }
    }
}
