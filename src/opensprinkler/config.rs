use super::{
    program::Programs,
    station::{self, Stations},
    RebootCause,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    fs::OpenOptions,
    io::{self, Write},
    path::PathBuf,
    sync::Arc,
};

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct EventsEnabled {
    pub program_sched: bool,
    pub sensor1: bool,
    pub flow_sensor: bool,
    pub weather_update: bool,
    pub reboot: bool,
    pub station_off: bool,
    pub sensor2: bool,
    pub rain_delay: bool,
    pub station_on: bool,
}

impl Default for EventsEnabled {
    fn default() -> Self {
        EventsEnabled {
            program_sched: false,
            sensor1: false,
            flow_sensor: false,
            weather_update: false,
            reboot: false,
            station_off: false,
            sensor2: false,
            rain_delay: false,
            station_on: false,
        }
    }
}

/* pub mod data_file {
    pub const INTEGER_OPTIONS: &'static str = "iopts.dat";
    pub const STRING_OPTIONS: &'static str = "sopts.dat";
    pub const STATIONS: &'static str = "stns.dat";
    pub const NV_CONTROLLER: &'static str = "nvcon.dat";
    pub const PROGRAMS: &'static str = "prog.dat";
    pub const DONE: &'static str = "done.dat";
} */

use crate::opensprinkler::{sensor::SensorOption, FIRMWARE_VERSION, FIRMWARE_VERSION_REVISION, HARDWARE_VERSION};
use std::net::IpAddr;

pub mod data_type {
    use std::net::IpAddr;

    use serde::{Deserialize, Serialize};

    use crate::opensprinkler::{sensor::SensorOption, RebootCause, FIRMWARE_VERSION, FIRMWARE_VERSION_REVISION, HARDWARE_VERSION};

    /* /// Non-volatile controller data
    #[derive(Clone, Serialize, Deserialize)]
    pub struct ControllerNonVolatile {
        
    }

    impl Default for ControllerNonVolatile {
        fn default() -> Self {
            ControllerNonVolatile {
                
            }
        }
    } */

    /* #[derive(Clone, Serialize, Deserialize)]
    pub struct IntegerOptions {

    }

    impl Default for IntegerOptions {
        fn default() -> Self {
            IntegerOptions {

            }
        }
    } */

    #[derive(Clone, Serialize, Deserialize)]
    pub struct StringOptions {
        /// Device key AKA password
        pub dkey: String,
        /// Device location (decimal coordinates)
        /// @todo Represent as a vector using [f64] instead of a string. This means dropping support for using city name / postal code, but geocoder can find coordinates anyways.
        pub loc: String,
        /// Javascript URL for the web app
        pub jsp: String,
        /// Weather Service URL
        pub wsp: String,
        /// Weather adjustment options
        /// This data is specific to the weather adjustment method.
        pub wto: String,
        /// IFTTT Webhooks API key
        pub ifkey: String,
        // Wi-Fi ESSID
        //#[deprecated(since = "3.0.0")]
        //pub ssid: String,
        // Wi-Fi PSK
        //#[deprecated(since = "3.0.0")]
        //pub pass: String,
        /// MQTT config @todo Use a struct?
        pub mqtt: String,
    }

    impl Default for StringOptions {
        fn default() -> Self {
            StringOptions {
                dkey: format!("{:x}", md5::compute(b"opendoor")).into(), // @todo Use modern hash like Argon2
                loc: "0,0".into(),
                jsp: "https://ui.opensprinkler.com".into(),
                wsp: "https://weather.opensprinkler.com".into(),
                wto: "".into(),
                ifkey: "".into(),
                //ssid: "".into(),
                //pass: "".into(),
                mqtt: "".into(),
            }
        }
    }

    /// maximum number of characters in each station name
    const STATION_NAME_SIZE: usize = 32;
}

#[derive(Serialize, Deserialize)]
pub struct ControllerConfiguration {
    //pub nv: data_type::ControllerNonVolatile,
    /// Sunrise time (minutes)
    pub sunrise_time: u16,
    /// Sunset time (minutes)
    pub sunset_time: u16,
    /// Rain-delay stop time (seconds since unix epoch)
    pub rd_stop_time: Option<i64>,
    /// External IP @todo Add support for IPv6
    pub external_ip: Option<IpAddr>,
    /// Reboot Cause
    pub reboot_cause: RebootCause,
    //pub iopts: data_type::IntegerOptions,
    /// firmware version
    pub fwv: u16,
    /// Time Zone
    ///
    /// Default: UTC
    pub tz: u8,
    /// this and the next unsigned char define HTTP port
    pub hp0: u8,
    /// -
    pub hp1: u8,
    /// -
    pub hwv: u8,
    /// number of 8-station extension board. 0: no extension boards
    pub ext: usize,
    /// station delay time (-10 minutes to 10 minutes).
    pub sdt: u8,
    /// index of master station. 0: no master station
    pub mas: Option<usize>,
    /// master on time adjusted time (-10 minutes to 10 minutes)
    pub mton: u8,
    /// master off adjusted time (-10 minutes to 10 minutes)
    pub mtof: u8,
    /// water level (default 100%),
    pub wl: u8,
    /// device enable
    pub den: bool,
    /// lcd contrast
    pub con: u8,
    /// lcd backlight
    pub lit: u8,
    /// lcd dimming
    pub dim: u8,
    /// weather algorithm (0 means not using weather algorithm)
    pub uwt: u8,
    /// enable logging: 0: disable; 1: enable.
    pub lg: bool,
    /// index of master2. 0: no master2 station
    pub mas2: Option<usize>,
    /// master2 on adjusted time
    pub mton2: u8,
    /// master2 off adjusted time
    pub mtof2: u8,
    /// firmware minor version
    pub fwm: u8,
    /// this and next unsigned char define flow pulse rate (100x)
    pub fpr0: u8,
    /// default is 1.00 (100)
    pub fpr1: u8,
    /// set as remote extension
    pub re: bool,
    /// special station auto refresh
    pub sar: bool,
    //pub ife: u8,
    /// ifttt enabled events
    pub ifttt_events: EventsEnabled,
    /// sensor 1 type (see SENSOR_TYPE macro defines)
    pub sn1t: u8,
    /// sensor 1 option. 0: normally closed; 1: normally open.	default 1.
    pub sn1o: SensorOption,
    /// sensor 2 type
    pub sn2t: u8,
    /// sensor 2 option. 0: normally closed; 1: normally open. default 1.
    pub sn2o: SensorOption,
    /// sensor 1 on delay
    pub sn1on: u8,
    /// sensor 1 off delay
    pub sn1of: u8,
    /// sensor 2 on delay
    pub sn2on: u8,
    /// sensor 2 off delay
    pub sn2of: u8,
    /// reset
    pub reset: u8,

    pub sopts: data_type::StringOptions,
    pub stations: Stations,
    pub programs: Programs,
}

impl Default for ControllerConfiguration {
    fn default() -> Self {
        ControllerConfiguration {
            /* nv: data_type::ControllerNonVolatile {
                reboot_cause: RebootCause::Reset,
                ..Default::default()
            }, */
            sunrise_time: 360, // 0600 default sunrise
            sunset_time: 1080, // 1800 default sunrise
            rd_stop_time: None,
            external_ip: None,
            reboot_cause: RebootCause::Reset,
            //iopts: data_type::IntegerOptions::default(),
            fwv: FIRMWARE_VERSION,
            tz: 48, // UTC
            hp0: 80,
            hp1: 0,
            hwv: HARDWARE_VERSION,
            ext: 0,
            sdt: 120,
            mas: None,
            mton: 120,
            mtof: 120,
            wl: 100,
            den: true,
            con: 150,
            lit: 100,
            dim: 50,
            uwt: 0,
            lg: true,
            mas2: None,
            mton2: 120,
            mtof2: 120,
            fwm: FIRMWARE_VERSION_REVISION,
            fpr0: 100,
            fpr1: 0,
            re: false,
            sar: false,
            //ife: 0,
            ifttt_events: EventsEnabled::default(),
            sn1t: 0,
            sn1o: SensorOption::NormallyOpen,
            sn2t: 0,
            sn2o: SensorOption::NormallyOpen,
            sn1on: 0,
            sn1of: 0,
            sn2on: 0,
            sn2of: 0,
            reset: 0,

            sopts: data_type::StringOptions::default(),
            stations: station::default(),
            programs: Vec::new(),
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
        let reader = io::BufReader::new(OpenOptions::new().open(&self.path)?);
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
