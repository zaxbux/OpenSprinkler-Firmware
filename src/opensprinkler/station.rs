use std::net::IpAddr;

use serde::{Deserialize, Serialize};

use crate::utils;

use super::{controller, sensor};

pub type StationIndex = usize;

pub const MAX_EXT_BOARDS: usize = 24;

/// maximum number of 8-zone boards including expanders
pub const MAX_NUM_BOARDS: usize = 1 + MAX_EXT_BOARDS;

/// maximum number of stations
pub const MAX_NUM_STATIONS: usize = MAX_NUM_BOARDS * controller::SHIFT_REGISTER_LINES as usize;

pub const MAX_MASTER_STATIONS: usize = 2;

pub type Stations = Vec<Station>;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct MasterStationConfig {
    pub station: Option<StationIndex>,
    /// Adjusted on duration (range: 0 – 600 seconds; step: 5 seconds)
    adjusted_on: u8,
    /// Adjusted off duration (range: -600 – 0 seconds; step: 5 seconds)
    adjusted_off: u8,
}

impl MasterStationConfig {
    pub fn get_adjusted_on_time(&self) -> i64 {
        utils::water_time_decode_signed(self.adjusted_on).into()
    }

    pub fn get_adjusted_off_time(&self) -> i64 {
        utils::water_time_decode_signed(self.adjusted_off).into()
    }

    pub fn set_adjusted_on_time_secs(&mut self, value: i16) {
        self.adjusted_on = utils::water_time_encode_signed(value);
    }

    pub fn set_adjusted_off_time_secs(&mut self, value: i16) {
        self.adjusted_off = utils::water_time_encode_signed(value);
    }
}

impl Default for MasterStationConfig {
    fn default() -> Self {
        Self {
            station: None,
            adjusted_on: 120,
            adjusted_off: 120,
        }
    }
}

#[repr(u8)]
#[derive(PartialEq)]
enum StationOption {
    NormallyClosed = 0,
    NormallyOpen = 1,
}

#[repr(u8)]
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub enum StationType {
    /// Standard station
    Standard = 0x00,
    /// RF station
    #[cfg(feature = "station-rf")]
    RadioFrequency = 0x01,
    /// Remote OpenSprinkler station
    Remote = 0x02,
    /// GPIO station
    #[cfg(feature = "station-gpio")]
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
    pub station_type: StationType,
    /// Special station data
    pub sped: Option<SpecialStationData>,
}

impl Station {
    pub fn is_sequential(&self) -> bool {
        self.attrib.is_sequential
    }
}

impl Default for Station {
    fn default() -> Self {
        Station {
            name: "".into(),
            attrib: StationAttrib {
                use_master: [false; MAX_MASTER_STATIONS],
                ignore_sensor: [false; sensor::MAX_SENSORS],
                is_disabled: false,
                is_sequential: true,
                ignore_rain_delay: false,
            },
            station_type: StationType::Standard,
            sped: None,
        }
    }
}

/// Station Attributes
#[derive(Clone, Serialize, Deserialize)]
pub struct StationAttrib {
    /// Use master stations
    pub use_master: [bool; MAX_MASTER_STATIONS],
    /// Ignore sensors
    pub ignore_sensor: [bool; sensor::MAX_SENSORS],
    /// Disabled
    pub is_disabled: bool,
    /// Sequential
    pub is_sequential: bool,
    /// Ignore Rain Delay
    pub ignore_rain_delay: bool,
}

/// RF station data structures
#[derive(Clone, Serialize, Deserialize)]
#[cfg(feature = "station-rf")]
pub struct RFStationData {
    /// 24-bit value
    pub on: u32,
    /// 24-bit value
    pub off: u32,
    /// 16-bit value
    pub timing: u16,
}

/// Remote station data structures
///
/// @todo: Support for IPv6, string hostname, path, and custom password (or deprecate in favour of HTTP station type?)
#[derive(Clone, Serialize, Deserialize)]
pub struct RemoteStationData {
    pub host: IpAddr,
    pub port: u16,
    pub station_index: StationIndex,
}

/// GPIO station data structures
#[derive(Clone, Serialize, Deserialize)]
#[cfg(feature = "station-gpio")]
pub struct GPIOStationData {
    /// GPIO Pin (BCM #)
    pub pin: u8,
    /// Active state
    /// - `true` = High
    /// - `false` = Low
    pub active: bool,
}

#[cfg(feature = "station-gpio")]
impl GPIOStationData {
    pub fn active_level(&self) -> rppal::gpio::Level {
        match self.active {
            false => rppal::gpio::Level::Low,
            true => rppal::gpio::Level::High,
        }
    }
}

/// HTTP station data structures
#[derive(Clone, Serialize, Deserialize)]
pub struct HTTPStationData {
    pub uri: String,
    pub cmd_on: String,
    pub cmd_off: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum SpecialStationData {
    #[cfg(feature = "station-rf")]
    RF(RFStationData),
    REMOTE(RemoteStationData),
    #[cfg(feature = "station-gpio")]
    GPIO(GPIOStationData),
    HTTP(HTTPStationData),
}

#[cfg(feature = "station-rf")]
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

#[cfg(feature = "station-gpio")]
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

#[cfg(test)]
mod tests {

    #[test]
    fn test_try_from_station_data() {
        todo!();
    }
}