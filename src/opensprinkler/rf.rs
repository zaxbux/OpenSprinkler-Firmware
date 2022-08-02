use rppal::gpio::OutputPin;
use super::OpenSprinkler;

// Parse RF code into on/off/timing sections
/* fn parse_rf_station_code(data: &RFStationData) -> (u32, u32, u16) {
	let on = hex::decode(data.on).unwrap();
	let off = hex::decode(data.off).unwrap();

	(
		BigEndian::read_u24(&on),
		BigEndian::read_u24(&off),
		u16::from_ne_bytes(data.timing),
	)
} */

/// Transmit one RF signal bit
///
/// This implementation uses the Pi's hardware timer.
 #[cfg(not(feature = "demo"))]
fn transmit_rf_bit(pin: &mut OutputPin, len_h: u64, len_l: u64) {
		pin.set_high();
		ruspiro_timer::sleep(std::time::Duration::from_micros(len_h));

		pin.set_low();
		ruspiro_timer::sleep(std::time::Duration::from_micros(len_l));
}

/// Transmit RF signal
pub fn send_rf_signal(open_sprinkler: &mut OpenSprinkler, code: u64, length: u64) {
	let len3 = length * 3;
	let len31 = length * 31;

	#[cfg(not(feature = "demo"))]
	{
		let rf_tx = open_sprinkler.gpio.get(super::gpio::pin::RF_TX).and_then(|pin| Ok(pin.into_output()));
		if let Err(ref error) = rf_tx {
			tracing::error!("Failed to obtain output pin rf_tx: {:?}", error);
			return;
		}
		let mut pin = rf_tx.unwrap();
	}

	for _ in 0..15 {
		let mut i = 23;
		// send code
		while i >= 0 {
			if (code >> i) & 1 != 0 {
				#[cfg(not(feature = "demo"))]
				transmit_rf_bit(&mut pin, len3, length);
			} else {
				#[cfg(not(feature = "demo"))]
				transmit_rf_bit(&mut pin, length, len3);
			}
			i -= 1;
		}

		// send sync
		#[cfg(not(feature = "demo"))]
		transmit_rf_bit(&mut pin, length, len31);
	}
}