use core::fmt;
use serde::{Deserialize, Serialize};

pub const MAX_SENSORS: usize = 2;

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
pub enum SensorType {
    /// No sensor
    None = 0x00,
    /// Rain sensor
    Rain = 0x01,
    /// Flow sensor
    Flow = 0x02,
    /// Soil moisture sensor
    Soil = 0x03,
    /// Program switch sensor
    ProgramSwitch = 0xF0,
    /// Other sensor
    Other = 0xFF,
}

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
pub enum NormalState {
    Closed = 0,
    Open = 1,
}

impl fmt::Display for NormalState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Closed => write!(f, "NC"),
            Self::Open => write!(f, "NO"),
        }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct SensorConfig {
    pub sensor_type: Option<SensorType>,
    pub normal_state: NormalState,
    pub delay_on: u8,
    pub delay_off: u8,
}

impl Default for SensorConfig {
    fn default() -> Self {
        Self {
            sensor_type: None,
            normal_state: NormalState::Open,
            delay_on: 0,
            delay_off: 0,
        }
    }
}
