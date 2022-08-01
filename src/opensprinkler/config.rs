use std::{fs::{File, OpenOptions, self}, io::{self, Write}};
use serde::{Serialize, Deserialize};
use super::{station::{Stations, self}, program::Programs, RebootCause};

/* pub mod data_file {
    pub const INTEGER_OPTIONS: &'static str = "iopts.dat";
    pub const STRING_OPTIONS: &'static str = "sopts.dat";
    pub const STATIONS: &'static str = "stns.dat";
    pub const NV_CONTROLLER: &'static str = "nvcon.dat";
    pub const PROGRAMS: &'static str = "prog.dat";
    pub const DONE: &'static str = "done.dat";
} */
pub mod data_type {
    use std::net::IpAddr;

    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Serialize};

    use crate::opensprinkler::{
        RebootCause, FIRMWARE_VERSION, HARDWARE_VERSION, FIRMWARE_VERSION_REVISION,
    };

    /// Non-volatile controller data
	#[derive(Clone, Serialize, Deserialize)]
    pub struct ControllerNonVolatile {
        /// Sunrise time (minutes)
        /// Was: [u16]
        pub sunrise_time: u16,
        /// Sunset time (minutes)
        /// Was: [u16]
        pub sunset_time: u16,
        /// Rain-delay stop time (seconds since unix epoch)
        /// Was: [u32]
        pub rd_stop_time: Option<DateTime<Utc>>,
        /// External IP @todo Add support for IPv6
        /// Was: [u32]
        pub external_ip: Option<IpAddr>,
        /// Reboot Cause
        pub reboot_cause: RebootCause,
    }

	impl Default for ControllerNonVolatile {
        fn default() -> Self {
            ControllerNonVolatile {
				sunrise_time: 360, // 0600 default sunrise
				sunset_time: 1080, // 1800 default sunrise
				rd_stop_time: None,
				external_ip: None,
				reboot_cause: RebootCause::None,
			}
		}
	}

    #[derive(Clone, Serialize, Deserialize)]
    pub struct IntegerOptions {
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
        pub den: u8,
        /// lcd contrast
        pub con: u8,
        /// lcd backlight
        pub lit: u8,
        /// lcd dimming
        pub dim: u8,
        /// weather algorithm (0 means not using weather algorithm)
        pub uwt: u8,
        /// enable logging: 0: disable; 1: enable.
        pub lg: u8,
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
        pub re: u8,
        /// special station auto refresh
        pub sar: u8,
        /// ifttt enable bits
        pub ife: u8,
        /// sensor 1 type (see SENSOR_TYPE macro defines)
        pub sn1t: u8,
        /// sensor 1 option. 0: normally closed; 1: normally open.	default 1.
        pub sn1o: u8,
        /// sensor 2 type
        pub sn2t: u8,
        /// sensor 2 option. 0: normally closed; 1: normally open. default 1.
        pub sn2o: u8,
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
    }

    impl Default for IntegerOptions {
        fn default() -> Self {
            IntegerOptions {
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
                den: 1,
                con: 150,
                lit: 100,
                dim: 50,
                uwt: 0,
                lg: 1,
                mas2: None,
                mton2: 120,
                mtof2: 120,
                fwm: FIRMWARE_VERSION_REVISION,
                fpr0: 100,
                fpr1: 0,
                re: 0,
                sar: 0,
                ife: 0,
                sn1t: 0,
                sn1o: 1,
                sn2t: 0,
                sn2o: 1,
                sn1on: 0,
                sn1of: 0,
                sn2on: 0,
                sn2of: 0,
                reset: 0,
            }
        }
    }

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
                wsp: "weather.opensprinkler.com".into(),
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
pub struct ConfigDocument {
	pub nv: data_type::ControllerNonVolatile,
	pub iopts: data_type::IntegerOptions,
	pub sopts: data_type::StringOptions,
	pub stations: Stations,
	pub programs: Programs,
}

pub fn get_config() -> Result<ConfigDocument, io::Error> {
	let reader = io::BufReader::new(File::open("./config.dat")?);
    Ok(bson::from_reader(reader).unwrap())
}

pub fn commit_config(config: &ConfigDocument) -> Result<(), io::Error> {
	// Write config
	let buf = bson::to_vec(config).unwrap();
	let mut writer = io::BufWriter::new(OpenOptions::new().write(true).create(true).open("./config.dat")?);
	writer.write_all(&buf);

	Ok(())
}

/// Reads the integer options file and returns a deserialized struct
pub fn get_controller_nv() -> Result<data_type::ControllerNonVolatile, io::Error> {
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
    Ok(get_config()?.iopts)
}

pub fn commit_integer_options(iopts: &data_type::IntegerOptions) -> Result<(), io::Error>  {
	// Read then modify config
	let mut config: ConfigDocument = get_config()?;
	config.iopts = iopts.clone();
	
	Ok(commit_config(&config)?)
}

/// Reads the string options file and returns a deserialized struct
pub fn get_string_options() -> Result<data_type::StringOptions, io::Error> {
	Ok(get_config()?.sopts)
}

pub fn commit_string_options(sopts: &data_type::StringOptions) -> Result<(), io::Error>  {
	// Read then modify config
	let mut config: ConfigDocument = get_config()?;
	config.sopts = sopts.clone();
	
	Ok(commit_config(&config)?)
}

pub fn get_stations() -> Result<Stations, io::Error> {
	Ok(get_config()?.stations)
}

pub fn commit_stations(stations: &Stations) -> Result<(), io::Error>  {
	// Read then modify config
	let mut config: ConfigDocument = get_config()?;
	config.stations = stations.to_vec();
	
	Ok(commit_config(&config)?)
}

pub fn get_programs() -> Result<Programs, io::Error> {
	Ok(get_config()?.programs)
}

pub fn commit_programs(programs: &Programs) -> Result<(), io::Error>  {
	// Read then modify config
	let mut config: ConfigDocument = get_config()?;
	config.programs = programs.to_vec();
	
	Ok(commit_config(&config)?)
}

pub fn pre_factory_reset() {
	fs::remove_file("./config.dat");
}

pub fn factory_reset() {
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

	commit_config(&config);
}