pub use rppal::gpio::Gpio;

pub use rppal::gpio::Level;

// region: GPIO Pins


/// Shift register **OE** (output enable) pin
pub const SHIFT_REGISTER_OE: u8 = 17;
/// Shift register **LATCH** pin
pub const SHIFT_REGISTER_LATCH: u8 = 22;
/// Shift register **CLOCK** pin
pub const SHIFT_REGISTER_CLOCK: u8 = 4;
/// Shift register **DATA** pin
pub const SHIFT_REGISTER_DATA: u8 = 27;
/// Sensor 1 pin
pub const SENSOR_1: u8 = 14;
/// Sensor 2 pin
pub const SENSOR_2: u8 = 23;
/// RF transmitter pin
pub const RF_TX: u8 = 15;

// endregion: GPIO Pins