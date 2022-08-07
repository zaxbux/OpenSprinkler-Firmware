pub mod cli;

use super::{program, sensor, station, weather};
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    fs::OpenOptions,
    io::{self, Write},
    net::IpAddr,
    path::PathBuf,
    str::FromStr,
};

#[cfg(feature = "mqtt")]
use crate::opensprinkler::events::mqtt;

use crate::opensprinkler::events::ifttt;

#[cfg(unix)]
const CONFIG_FILE_PATH: &'static str = "/etc/opt/config.dat";

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

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Location {
    lat: f32,
    lng: f32,
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
    /// Reboot Cause
    pub reboot_cause: RebootCause,
    /// Device key AKA password
    pub device_key: String,
    /// External IP Address
    pub external_ip: Option<IpAddr>,
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
    pub water_scale: u8,
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
    /// Enabled IFTTT events
    pub ifttt: ifttt::EventConfig,
    /// MQTT config
    #[cfg(feature = "mqtt")]
    pub mqtt: mqtt::MQTTConfig,

    /* Fields that are never serialized/deserialized */
    /// Config path
    #[serde(skip)]
    path: PathBuf,

    /// Cause of last reboot
    #[serde(skip)]
    pub last_reboot_cause: RebootCause,
}

impl Config {
    /* pub fn new() -> Self {
        Self::default()
    } */

    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            ..Self::default()
        }
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

    //pub fn write<T: Serialize>(&self, document: &T) -> result::Result<()> {
    pub fn write(&self) -> result::Result<()> {
        tracing::debug!("Write: {:?}", self.path.canonicalize().unwrap_or(self.path.clone()));
        let buf = bson::to_vec(&self)?;
        Ok(io::BufWriter::new(OpenOptions::new().write(true).create(true).open(&self.path)?).write_all(&buf)?)
    }

    pub fn write_default(&self) -> result::Result<()> {
        tracing::debug!("Write default: {:?}", self.path.canonicalize().unwrap_or(self.path.clone()));
        let buf = bson::to_vec(&Self::default())?;
        Ok(io::BufWriter::new(OpenOptions::new().write(true).create(true).open(&self.path)?).write_all(&buf)?)
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
            reboot_cause: RebootCause::Reset,                              // If the config file does not exist, these defaults will be used. Therefore, this is the relevant reason.
            device_key: format!("{:x}", md5::compute(b"opendoor")).into(), // @todo Use modern hash like Argon2
            external_ip: None,
            js_url: core::option_env!("JAVASCRIPT_URL").unwrap_or("https://ui.opensprinkler.com").into(),
            location: Location::default(),
            timezone: 48, // UTC
            weather: weather::WeatherConfig::default(),
            sunrise_time: 360, // 0600 default sunrise
            sunset_time: 1080, // 1800 default sunrise
            rain_delay_stop_time: None,
            water_scale: 100,
            stations: station::default(),
            station_delay_time: 120,
            master_stations: [station::MasterStationConfig::default(); station::MAX_MASTER_STATIONS],
            enable_special_stn_refresh: false,
            programs: Vec::new(),
            sensors: [sensor::SensorConfig::default(); sensor::MAX_SENSORS],
            flow_pulse_rate: 100,
            ifttt: ifttt::EventConfig::default(),
            #[cfg(feature = "mqtt")]
            mqtt: mqtt::MQTTConfig::default(),

            /* Fields that are never serialized/deserialized */
            path: PathBuf::from_str(CONFIG_FILE_PATH).unwrap(),
            last_reboot_cause: RebootCause::None,
        }
    }
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#?}", serde_json::to_string_pretty(&self))
    }
}

pub mod result {
    use core::{num, fmt};
    use std::{sync::Arc, io, error};

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
