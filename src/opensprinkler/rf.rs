use super::{OpenSprinkler, demo};

/// Transmit one RF signal bit
///
/// This implementation uses the Pi's hardware timer.
#[cfg(not(feature = "demo"))]
fn transmit_rf_bit(pin: &mut rppal::gpio::IoPin, len_h: u64, len_l: u64) {
		pin.set_high();
		ruspiro_timer::sleep(std::time::Duration::from_micros(len_h));

		pin.set_low();
		ruspiro_timer::sleep(std::time::Duration::from_micros(len_l));
}

/// Transmit RF signal
pub fn send_rf_signal(open_sprinkler: &mut OpenSprinkler, code: u64, length: u64) -> rppal::gpio::Result<()> {
	let len3 = length * 3;
	let len31 = length * 31;

	#[cfg(not(feature = "demo"))]
	let mut rf_tx = open_sprinkler.gpio.get(super::gpio::pin::RF_TX).and_then(|pin| Ok(pin.into_output()))?;
	#[cfg(feature = "demo")]
	let mut rf_tx = demo::get_gpio_pin(super::gpio::pin::RF_TX)?;
	

	for _ in 0..15 {
		let mut i = 23;
		// send code
		while i >= 0 {
			if (code >> i) & 1 != 0 {
				#[cfg(not(feature = "demo"))]
				transmit_rf_bit(&mut rf_tx, len3, length);
			} else {
				#[cfg(not(feature = "demo"))]
				transmit_rf_bit(&mut rf_tx, length, len3);
			}
			i -= 1;
		}

		// send sync
		#[cfg(not(feature = "demo"))]
		transmit_rf_bit(&mut rf_tx, length, len31);
	}

	Ok(())
}