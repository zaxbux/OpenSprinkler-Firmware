use std::time::Duration;

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
fn transmit_rf_bit(open_sprinkler: &OpenSprinkler, len_h: u64, len_l: u64) {
		self.gpio.lines.rf_tx.set_high();
		rusprio_timer::sleep(Duration::from_micros(len_h));
		self.gpio.lines.rf_tx.set_low();
		rusprio_timer::sleep(Duration::from_micros(len_l));
}

/// Transmit RF signal
pub fn send_rf_signal(open_sprinkler: &OpenSprinkler, code: u64, length: u64) {
	let len3 = length * 3;
	let len31 = length * 31;

	for n in 0..15 {
		let mut i = 23;
		// send code
		while i >= 0 {
			if (code >> i) & 1 != 0 {
				transmit_rf_bit(open_sprinkler, len3, length);
			} else {
				transmit_rf_bit(open_sprinkler, length, len3);
			}
			i -= 1;
		}

		// send sync
		transmit_rf_bit(open_sprinkler, length, len31);
	}
}