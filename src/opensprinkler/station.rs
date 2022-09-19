include!(concat!(env!("OUT_DIR"), "/build_constants.rs"));

use std::{net::{IpAddr, Ipv4Addr}, cmp, num::ParseIntError, fmt::Display};

use serde::{Deserialize, Serialize};

use crate::{utils, server::legacy::IntoLegacyFormat};

use super::sensor;

pub type StationIndex = usize;

pub const MAX_EXT_BOARDS: usize = constants::MAX_EXT_BOARDS;

/// maximum number of 8-zone boards including expanders
pub const MAX_NUM_BOARDS: usize = 1 + MAX_EXT_BOARDS;

/// Stations/Zones per board
pub const SHIFT_REGISTER_LINES: usize = 8;

/// maximum number of stations
pub const MAX_NUM_STATIONS: usize = MAX_NUM_BOARDS * SHIFT_REGISTER_LINES as usize;

pub const MAX_MASTER_STATIONS: usize = 2;

/// Maximum water time (seconds) = 18 hours.
pub const MAX_WATER_TIME: u16 = 64800;

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

    /// Values outside the range of [0, 600] will be limited
    pub fn set_adjusted_on_time_secs(&mut self, value: i16) {
        self.adjusted_on = utils::water_time_encode_signed(cmp::max(0, value));
    }

    /// Values outside the range of [-600, 0] will be limited
    pub fn set_adjusted_off_time_secs(&mut self, value: i16) {
        self.adjusted_off = utils::water_time_encode_signed(cmp::min(0, value));
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StationType {
    /// Standard station
    Standard = 0x00,
    /// RF station
    //#[cfg(feature = "station-rf")]
    RadioFrequency = 0x01,
    /// Remote OpenSprinkler station
    Remote = 0x02,
    /// GPIO station
    //#[cfg(feature = "station-gpio")]
    GPIO = 0x03,
    /// HTTP station
    HTTP = 0x04,
    /// Other station
    Other = 0xFF,
}

impl Into<u8> for StationType {
    fn into(self) -> u8 {
        self as u8
    }
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

pub trait TryFromLegacyString: Sized {
    type Error;

    fn try_from_legacy_string(data: &str) -> Result<Self, Self::Error>;
}

/// RF station data structures
#[derive(Debug, Clone, Serialize, Deserialize)]
//#[cfg(feature = "station-rf")]
pub struct RFStationData {
    /// 24-bit value
    pub on: u32,
    /// 24-bit value
    pub off: u32,
    /// 16-bit value
    pub timing: u16,
}

//#[cfg(feature = "station-rf")]
impl TryFromLegacyString for RFStationData {
    type Error = ParseIntError;

    /// Special data for RF Station is 16 characters of hex
    /// 
    /// * First 6 chars (3 bytes; u24) is on
    /// * Next  6 chars (3 bytes; u24) is off
    /// * Last  4 chars (2 bytes; u16) is the timing
    /// 
    /// ```text
    ///  0               1
    ///  0 1 2 3 4 5 6 7 0 1 2 3 4 5 6 7
    /// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    /// |    ON     |    OFF    | TIME  |
    /// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    /// ```
    fn try_from_legacy_string(data: &str) -> Result<Self, Self::Error> {
        // @todo check data length
        let data = format!("{:0>16}", data);

        let on = &data[0..6];
        let off = &data[6..12];
        let timing = &data[12..16];

        Ok(Self {
            on: u32::from_str_radix(on, 16)?,
            off: u32::from_str_radix(off, 16)?,
            timing: u16::from_str_radix(timing, 16)?,
        })
    }
}

/// Remote station data structures
///
/// @todo: Support for IPv6, string hostname, path, and custom password (or deprecate in favour of HTTP station type?)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteStationData {
    pub host: IpAddr,
    pub port: u16,
    pub station_index: StationIndex,
}

impl TryFromLegacyString for RemoteStationData {
    type Error = ParseIntError; //ParseError;

    /// Special data for Remote Station is 14 characters of hex.
    /// 
    /// * First 8 chars (4 bytes) is the IP address
    /// * Next  4 chars (2 bytes) is the port number
    /// * Last  2 chars (1 bytes) is the station index
    /// 
    /// ```text
    ///  0               1
    ///  0 1 2 3 4 5 6 7 0 1 2 3 4 5
    /// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    /// |  IP ADDRESS   | PORT  | I |
    /// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    /// ```
    fn try_from_legacy_string(data: &str) -> Result<Self, Self::Error> {
        let ip_addr = &data[0..8];
        let port = &data[8..12];
        let station_index = &data[12..14];
        Ok(Self {
            host: IpAddr::V4(Ipv4Addr::from(u32::from_str_radix(ip_addr, 16)?)),
            port: u16::from_str_radix(port, 16)?,
            station_index: usize::from_str_radix(station_index, 16)?,
        })
    }
}

/// GPIO station data structures
#[derive(Debug, Clone, Serialize, Deserialize)]
//#[cfg(feature = "station-gpio")]
pub struct GPIOStationData {
    /// GPIO Pin (BCM #)
    pub pin: u8,
    /// Active state
    /// - `true` = High
    /// - `false` = Low
    pub active: bool,
}

//#[cfg(feature = "station-gpio")]
impl GPIOStationData {
    pub fn active_level(&self) -> rppal::gpio::Level {
        match self.active {
            false => rppal::gpio::Level::Low,
            true => rppal::gpio::Level::High,
        }
    }
}

//#[cfg(feature = "station-gpio")]
impl TryFromLegacyString for GPIOStationData {
    type Error = (); //ParseError;

    /// Special data for GPIO Station is three bytes of ascii decimal (not hex).
    /// 
    /// * First two bytes is the GPIO pin number (zero padded)
    /// * Third byte is either `0` for active low (GND), or `1` for high (+5V) relays.
    /// 
    /// ```text
    ///    0     1     2
    /// +-----+-----+-----+
    /// | GPIO PIN  | STA |
    /// +-----+-----+-----+
    /// ```
    fn try_from_legacy_string(data: &str) -> Result<Self, Self::Error> {
        let pin_str = &data[0..2];
        let active_str = &data[2..3];

        let pin = pin_str.parse::<u8>().map_err(|_| ())?;
        let active: bool = (match active_str {
            "0" => Ok(false),
            "1" => Ok(true),
            _ => Err(()),
        })?;
        
        Ok(Self { pin, active })
    }
}

/// HTTP station data structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HTTPStationData {
    pub uri: String,
    pub cmd_on: String,
    pub cmd_off: String,
}

impl TryFromLegacyString for HTTPStationData {
    type Error = &'static str; //ParseError;

    /// Special data for HTTP Station is up to 240 characters
    /// (limitation of legacy firmware) of the comma-separated values:
    /// 
    /// * host
    /// * port
    /// * on command
    /// * off command
    fn try_from_legacy_string(data: &str) -> Result<Self, Self::Error> {
        let data: Vec<&str> = data.splitn(4, ',').collect();
        Ok(Self {
            uri: format!("http://{}:{}", data[0], data[1]),
            cmd_on: data[2].into(),
            cmd_off: data[3].into(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpecialStationData {
    NONE(()),
    RF(RFStationData),
    REMOTE(RemoteStationData),
    GPIO(GPIOStationData),
    HTTP(HTTPStationData),
}

impl Display for SpecialStationData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpecialStationData::NONE(data) => write!(f, "{:?}", data),
            SpecialStationData::RF(data) => write!(f, "{:?}", data),
            SpecialStationData::REMOTE(data) => write!(f, "{:?}", data),
            SpecialStationData::GPIO(data) => write!(f, "{:?}", data),
            SpecialStationData::HTTP(data) => write!(f, "{:?}", data),
        }
    }
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

impl IntoLegacyFormat for SpecialStationData {
    type Format = Option<String>;

    fn into_legacy_format(&self) -> Self::Format {
        match self {
            SpecialStationData::NONE(_) => None,
            SpecialStationData::RF(data) => {
                // sd (16 characters) stores the 16-digit hex RF code
                // on [6]; off [6]; timing [4];
                Some(format!("{:06x}{:06x}{:04x}", data.on, data.off, data.timing))
            }
            SpecialStationData::REMOTE(data) => {
                // ip address (8 bytes), port number (4 bytes), station index (2 bytes)
                if let IpAddr::V4(addr) = data.host {
                    Some(format!("{:08x}{:04x}{:02x}", u32::from(addr), data.port, data.station_index))
                } else {
                    None
                }
            }
            SpecialStationData::GPIO(data) => {
                // 3 bytes: the first two define the GPIO index (as zero-padded decimal), the third byte indicates active state (1 or 0)
                Some(format!("{:02}{:}", data.pin, if data.active { 1 } else { 0 }))
            }
            SpecialStationData::HTTP(data) => {
                // up to 240 bytes, comma separated HTTP GET command with the following data: server name (can be either a domain name or IP address), port number, on command, off command
                if let Ok(uri) = url::Url::parse(data.uri.as_str()) {
                    Some(format!("{},{},{},{}", uri.host_str().unwrap(), uri.port().unwrap_or(80), data.cmd_on, data.cmd_off))
                } else {
                    None
                }
            }
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