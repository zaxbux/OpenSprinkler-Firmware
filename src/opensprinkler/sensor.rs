pub mod flow;

use core::fmt;
use serde::{Deserialize, Serialize};

pub type SensorStatusVec = Vec<SensorStatus>;

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

#[derive(Clone, Copy, Default)]
pub struct SensorStatus {
    /// time when sensor is detected on last time
    pub timestamp_on: Option<i64>,
    /// time when sensor is detected off last time
    pub timestamp_off: Option<i64>,
    /// most recent time when sensor is activated
    pub timestamp_activated: Option<i64>,

    /// State history used for "noise filtering"
    pub history: u8,

    pub detected: bool,

    pub active_now: bool,
    pub active_previous: bool,
}


impl SensorStatus {
    pub fn reset(&mut self) {
        self.timestamp_on = None;
        self.timestamp_off = None;
        self.timestamp_activated = None;
        self.history = 0;
        self.active_now = false;
        self.active_previous = false;
    }
}