use rppal::gpio::Level;

/// Robert Hillman (RAH)'s implementation of flow sensor
#[derive(Debug, Default)]
pub struct State {
    /// time when valve turns on
    time_begin: i64,
    /// time when flow starts being measured (i.e. 2 mins after flow_begin approx
    time_measure_start: i64,
    /// time when valve turns off (last rising edge pulse detected before off)
    time_measure_stop: i64,
    /// total # of gallons+1 from flow_start to flow_stop
    gallons: u64,

    /// current flow count
    flow_count: u64,

    previous_logic_level: Option<Level>,
}

impl State {
	pub fn poll(&mut self, logic_level: Level) {
		if self.previous_logic_level.unwrap_or(Level::Low) == Level::Low && logic_level != Level::Low {
			// only record on falling edge
			self.previous_logic_level = Some(logic_level);
			return;
		}
		self.previous_logic_level = Some(logic_level);
		let now_millis = chrono::Utc::now().timestamp_millis();
		self.flow_count += 1;
	
		/* RAH implementation of flow sensor */
		if self.time_measure_start == 0 {
			// if first pulse, record time
			self.gallons = 0;
			self.time_measure_start = now_millis;
		} 
		if now_millis - self.time_measure_start < 90000 {
			// wait 90 seconds before recording time_begin
			self.gallons = 0;
		} else {
			if self.gallons == 1 {
				self.time_begin = now_millis;
			}
		}
		// get time in ms for stop
		self.time_measure_stop = now_millis;
		// increment gallon count for each poll
		self.gallons += 1;
	}

	/// Reset the current flow measurement state
	pub fn reset(&mut self) {
		self.time_measure_start = 0;
	}

	/// last flow rate measured (averaged over flow_gallons) from last valve stopped (used to write to log file).
	pub fn measure(&mut self) -> f64 {
		if self.gallons > 1 {
			// RAH calculate GPM, 1 pulse per gallon
	
			//if self.time_measure_stop <= self.time_measure_start {
			if self.get_duration() <= 0 {
				//self.flow_last_gpm = 0.0;
				//return 0.0;
			} else {
				//self.flow_last_gpm = 60000.0 / (self.get_duration() as f64 / (self.gallons - 1) as f64);
				return 60000.0 / (self.get_duration() as f64 / (self.gallons - 1) as f64);
			}
		} else {
			// RAH if not one gallon (two pulses) measured then record 0 gpm
			//self.flow_last_gpm = 0.0;
			//return 0.0;
		}

		return 0.0;
	}

	pub fn get_flow_count(&self) -> u64 {
		self.flow_count
	}

	fn get_duration(&self) -> i64 {
		self.time_measure_stop - self.time_measure_start
	}
}