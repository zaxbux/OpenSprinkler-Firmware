use serde::{Serialize, Deserialize};

// flow count window (for computing real-time flow rate), 30 seconds
pub const FLOW_COUNT_REALTIME_WINDOW: i64 = 30;

#[derive(PartialEq)]
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
pub enum SensorOption {
    NormallyClosed = 0,
    NormallyOpen = 1,
}

#[derive(Default)]
pub struct SensorStatus {
    /// time when sensor is detected on last time
    pub on_timer: Option<i64>,
    /// time when sensor is detected off last time
    pub off_timer: Option<i64>,
    /// most recent time when sensor is activated
    pub active_last_time: Option<i64>,

    /// State history used for "noise filtering"
	pub history: u8
}

/* impl Default for SensorStatus {
    fn default() -> Self {
        SensorStatus { on_timer: None, off_timer: None, active_last_time: None }
    }
} */

pub const MAX_SENSORS: usize = 2;

pub type SensorStatusVec = Vec<SensorStatus>;

pub fn init_vec() -> SensorStatusVec {
    let mut sensor_status = Vec::with_capacity(MAX_SENSORS);
    for _ in 0..sensor_status.capacity() {
        sensor_status.push(SensorStatus::default());
    }
    sensor_status
}
