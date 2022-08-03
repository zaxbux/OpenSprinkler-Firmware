use std::net::IpAddr;

use serde::{Deserialize, Serialize};

/// Stations/Zones per board
pub const SHIFT_REGISTER_LINES: usize = 8;

/// allow more zones for linux-based firmwares
pub const MAX_EXT_BOARDS: usize = 24;

/// maximum number of 8-zone boards including expanders
pub const MAX_NUM_BOARDS: usize = 1 + MAX_EXT_BOARDS;

/// maximum number of stations
pub const MAX_NUM_STATIONS: usize = MAX_NUM_BOARDS * SHIFT_REGISTER_LINES as usize;

pub type Stations = Vec<Station>;

#[repr(u8)]
#[derive(PartialEq)]
enum StationOption {
    NormallyClosed = 0,
    NormallyOpen = 1,
}

#[repr(u8)]
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub enum StationType {
    /// Stnadard station
    Standard = 0x00,
    /// RF station
    RadioFrequency = 0x01,
    /// Remote OpenSprinkler station
    Remote = 0x02,
    /// GPIO station
    GPIO = 0x03,
    /// HTTP station
    HTTP = 0x04,
    /// Other station
    Other = 0xFF,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Station {
    pub name: String,
    pub attrib: StationAttrib,
    /// Station type
    pub r#type: StationType,
    /// Special station data
    pub sped: Option<SpecialStationData>,
}

impl Default for Station {
    fn default() -> Self {
        Station {
            name: "".into(),
            attrib: StationAttrib {
                mas: true,
                igs: false,
                mas2: false,
                dis: false,
                seq: true,
                igs2: false,
                igrd: false,
            },
            r#type: StationType::Standard,
            sped: None,
        }
    }
}

/// Station Attributes
#[derive(Clone, Serialize, Deserialize)]
pub struct StationAttrib {
    /// Use Master #1
    pub mas: bool,
    /// Ignore Sensor #1
    pub igs: bool,
    /// Use Master #2
    pub mas2: bool,
    /// Disabled
    pub dis: bool,
    /// Sequential
    pub seq: bool,
    /// Ignore Sensor #2
    pub igs2: bool,
    /// Ignore Rain Delay
    pub igrd: bool,
}

/// RF station data structures
#[derive(Clone, Serialize, Deserialize)]
pub struct RFStationData {
    /// 24-bit value
    pub on: u32,
    /// 24-bit value
    pub off: u32,
    /// 16-bit value
    pub timing: u16,
}

/// Remote station data structures - Must fit in STATION_SPECIAL_DATA_SIZE
///
/// @todo: Support for IPv6, string hostname, path, and custom password (or deprecate in favour of HTTP station type?)
#[derive(Clone, Serialize, Deserialize)]
pub struct RemoteStationData {
    pub ip: IpAddr,
    pub port: u16,
    pub sid: usize,
}

/// GPIO station data structures - Must fit in STATION_SPECIAL_DATA_SIZE
#[derive(Clone, Serialize, Deserialize)]
pub struct GPIOStationData {
    /// GPIO Pin (BCM #)
    pub pin: u8,
    /// Active state
    /// - `true` = High
    /// - `false` = Low
    pub active: bool,
}

impl GPIOStationData {
    pub fn active_level(&self) -> rppal::gpio::Level {
        match self.active {
            false => rppal::gpio::Level::Low,
            true => rppal::gpio::Level::High,
        }
    }
}

/// HTTP station data structures - Must fit in STATION_SPECIAL_DATA_SIZE
#[derive(Clone, Serialize, Deserialize)]
pub struct HTTPStationData {
    pub uri: String,
    pub cmd_on: String,
    pub cmd_off: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum SpecialStationData {
    RF(RFStationData),
    REMOTE(RemoteStationData),
    GPIO(GPIOStationData),
    HTTP(HTTPStationData),
}

impl TryFrom<&SpecialStationData> for RFStationData {
    type Error = &'static str;

    fn try_from(value: &SpecialStationData) -> Result<Self, Self::Error> {
        match value {
            SpecialStationData::RF(data) => Ok(data.clone()),
            _ => Err("Cannot convert to RFStationData"),
        }
    }
}

impl TryFrom<&SpecialStationData> for RemoteStationData {
    type Error = &'static str;

    fn try_from(value: &SpecialStationData) -> Result<Self, Self::Error> {
        match value {
            SpecialStationData::REMOTE(data) => Ok(data.clone()),
            _ => Err("Cannot convert to RemoteStationData"),
        }
    }
}

impl TryFrom<&SpecialStationData> for GPIOStationData {
    type Error = &'static str;

    fn try_from(value: &SpecialStationData) -> Result<Self, Self::Error> {
        match value {
            SpecialStationData::GPIO(data) => Ok(data.clone()),
            _ => Err("Cannot convert to GPIOStationData"),
        }
    }
}

impl TryFrom<&SpecialStationData> for HTTPStationData {
    type Error = &'static str;

    fn try_from(value: &SpecialStationData) -> Result<Self, Self::Error> {
        match value {
            SpecialStationData::HTTP(data) => Ok(data.clone()),
            _ => Err("Cannot convert to HTTPStationData"),
        }
    }
}

pub fn default() -> Stations {
    let mut stations = Vec::with_capacity(MAX_NUM_STATIONS);

    for i in 0..MAX_NUM_STATIONS {
        stations.push(Station {
            name: format!("S{:0>3}", i + 1),
            ..Default::default()
        });
    }

    stations
}
