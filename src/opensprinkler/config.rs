pub mod cli;

use super::{
    program,
    station,
    sensor,
    weather,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    error, fmt,
    fs::OpenOptions,
    io::{self, Write},
    num,
    path::PathBuf,
    sync::Arc,
    str::FromStr, net::IpAddr,
};

#[cfg(feature = "mqtt")]
use crate::opensprinkler::events::mqtt;

use crate::opensprinkler::events::ifttt;

#[derive(Clone, Serialize, Deserialize)]
#[repr(u8)]
pub enum HardwareVersionBase {
    #[deprecated(note = "Rust port of firmware is not compatible with Arduino/ESP platforms")]
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

impl Default for RebootCause {
    fn default() -> Self {
        Self::None
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

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Location {
    lat: f32,
    lng: f32,
}

impl TryFrom<String> for Location {
    type Error = ParseLocationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if let Some((lat, lng)) = value.split_once(',') {
            return Ok(Location {
                lat: f32::from_str(lat)?,
                lng: f32::from_str(lng)?,
            });
        }

        Err(ParseLocationError::Invalid)
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 4 decimal places gives ≈10 meters of accuracy and should be enough for this use case
        write!(f, "{:.4},{:.4}", self.lat, self.lng)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ControllerConfiguration {
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

    /* Fields that are never saved */

    /// Cause of last reboot
    #[serde(skip)]
    pub last_reboot_cause: RebootCause,
}

impl Default for ControllerConfiguration {
    fn default() -> Self {
        ControllerConfiguration {
            firmware_version: semver::Version::parse(core::env!("CARGO_PKG_VERSION")).unwrap(),
            hardware_version: HardwareVersionBase::OpenSprinklerPi,
            extension_board_count: 0,
            enable_controller: true,
            enable_remote_ext_mode: false,
            enable_log: true,
            reboot_cause: RebootCause::Reset, // If the config file does not exist, these defaults will be used. Therefore, this is the relevant reason.
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

            last_reboot_cause: RebootCause::None,
        }
    }
}

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

pub struct Config {
    path: PathBuf,
}

impl Config {
    pub fn new(path: PathBuf) -> Config {
        Config { path }
    }

    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    pub fn get<T: DeserializeOwned>(&self) -> Result<T, Error> {
        let reader = io::BufReader::new(OpenOptions::new().read(true).open(&self.path)?);
        Ok(bson::from_reader(reader)?)
    }

    pub fn commit<T: Serialize>(&self, document: &T) -> Result<(), Error> {
        let buf = bson::to_vec(document)?;
        Ok(io::BufWriter::new(OpenOptions::new().write(true).create(true).open(&self.path)?).write_all(&buf)?)
    }

    pub fn commit_defaults(&self) -> Result<(), Error> {
        let document = ControllerConfiguration::default();
        Ok(self.commit(&document)?)
    }
}
/*
pub fn get_config<P: AsRef<Path>>(path: P) -> Result<ConfigDocument, io::Error> {
    let reader = io::BufReader::new(File::open(path)?);
    Ok(bson::from_reader(reader).unwrap())
}

pub fn commit_config<P: AsRef<Path>>(path: P, document: &ConfigDocument) -> Result<(), io::Error> {
    // Write config
    let buf = bson::to_vec(document).unwrap();
    let mut writer = io::BufWriter::new(OpenOptions::new().write(true).create(true).open(path)?);
    writer.write_all(&buf)?;

    Ok(())
}

/// Reads the integer options file and returns a deserialized struct
pub fn get_controller_nv() -> Result<data_type::ControllerNonVolatile, io::Error> {
    tracing::trace!("Reading controller non-volatile data");
    Ok(get_config()?.nv)
}

pub fn commit_controller_nv(nv: &data_type::ControllerNonVolatile) -> Result<(), io::Error> {
    // Read then modify config
    let mut config: ConfigDocument = get_config()?;
    config.nv = nv.clone();

    Ok(commit_config(&config)?)
}

/// Reads the integer options file and returns a deserialized struct
pub fn get_integer_options() -> Result<data_type::IntegerOptions, io::Error> {
    tracing::trace!("Reading integer options");
    Ok(get_config()?.iopts)
}

pub fn commit_integer_options(iopts: &data_type::IntegerOptions) -> Result<(), io::Error> {
    // Read then modify config
    let mut config: ConfigDocument = get_config()?;
    config.iopts = iopts.clone();

    Ok(commit_config(&config)?)
}

/// Reads the string options file and returns a deserialized struct
pub fn get_string_options() -> Result<data_type::StringOptions, io::Error> {
    tracing::trace!("Reading string options");
    Ok(get_config()?.sopts)
}

pub fn commit_string_options(sopts: &data_type::StringOptions) -> Result<(), io::Error> {
    // Read then modify config
    let mut config: ConfigDocument = get_config()?;
    config.sopts = sopts.clone();

    Ok(commit_config(&config)?)
}

pub fn get_stations() -> Result<Stations, io::Error> {
    tracing::trace!("Reading stations");
    let stations = get_config()?.stations;
    tracing::trace!("Got {} stations", stations.len());
    Ok(stations)
}

pub fn commit_stations(stations: &Stations) -> Result<(), io::Error> {
    // Read then modify config
    let mut config: ConfigDocument = get_config()?;
    config.stations = stations.to_vec();

    Ok(commit_config(&config)?)
}

pub fn get_programs() -> Result<Programs, io::Error> {
    tracing::trace!("Reading programs");
    let programs = get_config()?.programs;
    tracing::trace!("Got {} programs", programs.len());
    Ok(programs)
}

pub fn commit_programs(programs: &Programs) -> Result<(), io::Error> {
    // Read then modify config
    let mut config: ConfigDocument = get_config()?;
    config.programs = programs.to_vec();

    Ok(commit_config(&config)?)
}

pub fn pre_factory_reset<P: AsRef<Path>>(path: P) -> io::Result<()> {
    fs::remove_file(path)
}

pub fn factory_reset() -> io::Result<()> {
    let config = ConfigDocument {
        nv: data_type::ControllerNonVolatile {
            reboot_cause: RebootCause::Reset,
            ..Default::default()
        },
        iopts: data_type::IntegerOptions::default(),
        sopts: data_type::StringOptions::default(),
        stations: station::default(),
        programs: Vec::new(),
    };

    commit_config(&config)
}
 */
