use rppal::gpio;

use crate::{
    timer,
};

/// Transmit one RF signal bit
fn transmit_rf_bit(pin: &mut gpio::OutputPin, len_h: u64, len_l: u64) {
    pin.set_high();
    timer::sleep(std::time::Duration::from_micros(len_h));

    pin.set_low();
    timer::sleep(std::time::Duration::from_micros(len_l));
}

/// Transmit RF signal
pub fn send_rf_signal(gpio: Option<&gpio::Gpio>, code: u64, length: u64) -> gpio::Result<()> {
    let len3 = length * 3;
    let len31 = length * 31;

    if let Some(gpio) = gpio {
        let mut rf_tx = gpio.get(super::gpio::RF_TX).and_then(|pin| Ok(pin.into_output()))?;

        for _ in 0..15 {
            let mut i = 23;
            // send code
            while i >= 0 {
                if (code >> i) & 1 != 0 {
                    transmit_rf_bit(&mut rf_tx, len3, length);
                } else {
                    transmit_rf_bit(&mut rf_tx, length, len3);
                }
                i -= 1;
            }
    
            // send sync
            transmit_rf_bit(&mut rf_tx, length, len31);
        }
    }
    Ok(())
}
