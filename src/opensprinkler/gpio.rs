// #[cfg(target_os = "linux")]
use rppal::gpio;

/* pub struct Lines {
    pub shift_register_clock: gpio::OutputPin,
    pub shift_register_oe: gpio::OutputPin,
    pub shift_register_latch: gpio::OutputPin,
    pub shift_register_data: gpio::OutputPin,
    pub sensors: [gpio::InputPin; 2],
    // pub sensor_1: gpio::InputPin,
    // pub sensor_2: gpio::InputPin,
    pub rf_tx: gpio::OutputPin,
} */

pub mod pin {
    /// Shift register **CLOCK** pin
    pub const SHIFT_REGISTER_CLOCK: u8 = 4;
    /// Shift register **OE** (output enable) pin
    pub const SHIFT_REGISTER_OE: u8 = 17;
    /// Shift register **LATCH** pin
    pub const SHIFT_REGISTER_LATCH: u8 = 22;
    /// Shift register **DATA** pin
    pub const SHIFT_REGISTER_DATA: u8 = 27;
    /// Sensor 1 pin
    pub const SENSOR_1: u8 = 14;
    /// Sensor 2 pin
    pub const SENSOR_2: u8 = 23;
    /// RF transmitter pin
    pub const RF_TX: u8 = 15;
}

/* pub struct GPIO {
    gpio: gpio::Gpio,
    pub lines: Lines,
}
impl GPIO {
    pub fn new() -> Result<GPIO, gpio::Error> {
        let gpio = gpio::Gpio::new().expect("Error getting GPIO chip");

        let shift_register_oe = gpio.get(pin::SHIFT_REGISTER_OE)?.into_output_high();
        let shift_register_latch = gpio.get(pin::SHIFT_REGISTER_LATCH)?.into_output_high();
        let shift_register_clock = gpio.get(pin::SHIFT_REGISTER_CLOCK)?.into_output_high();
        let shift_register_data = gpio.get(pin::SHIFT_REGISTER_DATA)?.into_output_high();
        let sensor_1 = gpio.get(pin::SENSOR_1)?.into_input_pullup();
        let sensor_2 = gpio.get(pin::SENSOR_2)?.into_input_pullup();
        let rf_tx = gpio.get(pin::RF_TX)?.into_output_low();

        Ok(GPIO {
            gpio,
            lines: Lines {
                shift_register_oe,
                shift_register_latch,
                shift_register_clock,
                shift_register_data,
                //sensor_1: &mut sensor_1,
                //sensor_2: &mut sensor_2,
                sensors: [
                    sensor_1,
                    sensor_2,
                ],
                rf_tx: rf_tx,
            },
        })
    }

    pub fn get_pin(&self, pin: u8) -> rppal::gpio::Result<rppal::gpio::Pin> {
        self.gpio.get(pin)
    }
} */
