pub use rppal::gpio::{Gpio, Level};
use super::sensor;

/// Shift register **OE** (output enable) pin
pub const SHIFT_REGISTER_OE: u8 = 17;
/// Shift register **LATCH** pin
pub const SHIFT_REGISTER_LATCH: u8 = 22;
/// Shift register **CLOCK** pin
pub const SHIFT_REGISTER_CLOCK: u8 = 4;
/// Shift register **DATA** pin
pub const SHIFT_REGISTER_DATA: u8 = 27;
/// Sensor pins
pub const SENSOR: [u8; sensor::MAX_SENSORS] = [14, 23];
/// RF transmitter pin
#[cfg(feature = "station-rf")]
pub const RF_TX: u8 = 15;