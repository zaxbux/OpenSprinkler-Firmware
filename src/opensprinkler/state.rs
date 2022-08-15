use super::{gpio, sensor, station, weather, program};

pub type StationBits = [bool; station::SHIFT_REGISTER_LINES];


#[derive(PartialEq)]
pub enum StationChange {
    /// The active state of the station was not changed
    NoChange,
    /// The active state of the station was changed
    Change(bool),
}

pub struct StationState {
    /// Slice containing active state flags for each board
    pub active: [StationBits; station::MAX_NUM_BOARDS],
    /// Special station auto-refresh next station index
    pub auto_refresh_next_index: station::StationIndex,
    /// Most recent timestamp of special station auto-refresh
    pub auto_refresh_timestamp: i64,
}

impl StationState {
    /// Set all station active flags to [false]
    pub fn clear(&mut self) {
        for i in 0..station::MAX_NUM_STATIONS {
            self.set_active(i, false);
        }
    }

    pub fn is_active(&self, station_index: station::StationIndex) -> bool {
        let board = station_index >> 3;
        let board_index = station_index & 0x07;
        self.active[board][board_index]
    }

    pub fn set_active(&mut self, station_index: station::StationIndex, active: bool) -> StationChange {
        if self.is_active(station_index) == active {
            return StationChange::NoChange;
        }

        let board = station_index >> 3;
        let board_index = station_index & 0x07;
        self.active[board][board_index] = active;

        StationChange::Change(active)
    }

    pub fn board(&self, station_index: station::StationIndex) -> StationBits {
        self.active[station_index >> 3]
    }

    pub fn auto_refresh_next_index(&self) -> station::StationIndex {
        self.auto_refresh_next_index
    }

    pub fn set_auto_refresh_next_index(&mut self, auto_refresh_next_index: station::StationIndex) {
        self.auto_refresh_next_index = auto_refresh_next_index;
    }

    pub fn auto_refresh_timestamp(&self) -> i64 {
        self.auto_refresh_timestamp
    }

    pub fn set_auto_refresh_timestamp(&mut self, auto_refresh_timestamp: i64) {
        self.auto_refresh_timestamp = auto_refresh_timestamp;
    }
}

impl Default for StationState {
    fn default() -> Self {
        Self {
            active: [[false; station::SHIFT_REGISTER_LINES]; station::MAX_NUM_BOARDS],
            auto_refresh_next_index: station::MAX_NUM_STATIONS >> 1,
            auto_refresh_timestamp: 0,
        }
    }
}

pub struct ProgramState {
    /// A program is currently being executed
    pub busy: bool,
    pub queue: program::ProgramQueue,
}

impl Default for ProgramState {
    fn default() -> Self {
        Self {
            busy: false,
            queue: program::ProgramQueue::new(),
        }
    }
}

/// Rain Delay state
/// 
/// @todo: Use timestamp / config.stop_time instead of active flags
pub struct RainDelayState {
    /// time when the most recent rain delay started (seconds)
    pub timestamp_active_last: Option<i64>,
    pub active_now: bool,
    pub active_previous: bool,
}

impl Default for RainDelayState {
    fn default() -> Self {
        Self {
            timestamp_active_last: None,
            active_now: false,
            active_previous: false,
        }
    }
}

const HISTORY_SIZE: usize = 4;

#[derive(Clone, Copy)]
pub struct SensorState {
    /// time when sensor is detected on last time
    pub timestamp_on: Option<i64>,
    /// time when sensor is detected off last time
    pub timestamp_off: Option<i64>,
    /// most recent time when sensor is activated
    pub timestamp_activated: Option<i64>,

    /// State history used for "noise filtering"
    pub history: [bool; HISTORY_SIZE],

    pub detected: bool,

    pub active_now: bool,
    pub active_previous: bool,
}

impl SensorState {
    pub fn reset(&mut self) {
        self.timestamp_on = None;
        self.timestamp_off = None;
        self.timestamp_activated = None;
        self.history = [false; HISTORY_SIZE];
        self.active_now = false;
        self.active_previous = false;
    }
}

impl Default for SensorState {
    fn default() -> Self {
        Self {
            timestamp_on: None,
            timestamp_off: None,
            timestamp_activated: None,
            history: [false; HISTORY_SIZE],
            detected: false,
            active_now: false,
            active_previous: false,
        }
    }
}

pub struct SensorStateVec {
	vec: Vec<SensorState>,
}

impl SensorStateVec {
    pub fn new() -> Self {
        /* let mut vec = Vec::with_capacity(sensor::MAX_SENSORS);
        vec.fill(SensorState::default());

        Self {
            vec,
        } */
        Self { vec: vec![SensorState::default(); sensor::MAX_SENSORS] }
    }

    pub fn detected(&self, i: usize) -> bool {
        self.vec[i].detected
    }

    pub fn set_detected(&mut self, i: usize, detected: bool) {
        self.vec[i].detected = detected
    }

    pub fn state(&self, i: usize) -> bool {
        self.vec[i].active_now
    }

    pub fn set_state(&mut self, i: usize, state_now: bool) {
        self.vec[i].active_now = state_now
    }

    /// Set the previous state of the sensor equal to the current state
    pub fn set_state_equal(&mut self, i: usize) {
        self.vec[i].active_previous = self.vec[i].active_now;
    }

    /// Returns [true] if the current state is equal to the previous state
    pub fn state_equal(&self, i: usize) -> bool {
        self.vec[i].active_previous == self.vec[i].active_now
    }

    pub fn timestamp_on(&self, i: usize) -> Option<i64> {
        self.vec[i].timestamp_on
    }

    pub fn set_timestamp_on(&mut self, i: usize, timestamp_on: Option<i64>) {
        self.vec[i].timestamp_on = timestamp_on
    }

    pub fn timestamp_off(&self, i: usize) -> Option<i64> {
        self.vec[i].timestamp_off
    }

    pub fn set_timestamp_off(&mut self, i: usize, timestamp_off: Option<i64>) {
        self.vec[i].timestamp_off = timestamp_off
    }

    pub fn timestamp_activated(&self, i: usize) -> Option<i64> {
        self.vec[i].timestamp_activated
    }

    pub fn set_timestamp_activated(&mut self, i: usize, timestamp_activated: Option<i64>) {
        self.vec[i].timestamp_activated = timestamp_activated
    }

    /// Perform basic noise filtering on sensor history (for program switch type)
    ///
    /// i.e. two consecutive lows followed by two consecutive highs
    pub fn history_filter(&self, i: usize) -> bool {
        let s: SensorState = self.vec[i];
        return s.history == [false, false, true, true];
        //return (self.vec[i].history & 0b1111) == 0b0011;
    }

    pub fn push_history(&mut self, i: usize, detected: bool) {
        if let Some(s) = self.vec.get_mut(i) {
            s.history.rotate_left(1);
            s.history[HISTORY_SIZE - 1] = detected;
        }
    }

    /// Reset sensors
    ///
    /// Arguments:
    /// - `i`: Sensor index (pass [None] to reset all sensors)
    pub fn reset(&mut self, i: Option<usize>) {
        if let Some(i) = i {
            self.vec[i].reset();
        } else {
            for sensor in self.vec.iter_mut() {
                sensor.reset();
            }
        }
    }
}


/// State for recording/logging realtime flow count
#[derive(Debug)]
pub struct FlowState {
    /// Flow count (initial)
    pub count_log_start: i64,
    /// Flow count
    pub count_realtime_now: i64,
    /// Flow count
    pub count_realtime_start: i64,

    /* Flow Rate */
    /// time when valve turns on
    time_begin: i64,
    /// time when flow starts being measured (i.e. 2 mins after flow_begin approx
    time_measure_start: i64,
    /// time when valve turns off (last rising edge pulse detected before off)
    time_measure_stop: i64,
    /// total # of gallons+1 from flow_start to flow_stop
    volume: u64,

    /// current flow count
    flow_count: i64,

    previous_logic_level: Option<gpio::Level>,
}

impl FlowState {
    #[cfg(not(test))]
    const MIN_MILLIS: i64 = 90000;

    #[cfg(test)]
    const MIN_MILLIS: i64 = 0;

    pub fn poll(&mut self, logic_level: gpio::Level) {
        if let Some(previous_logic_level) = self.previous_logic_level {
            if !(previous_logic_level == gpio::Level::High && logic_level == gpio::Level::Low) {
                // only record on falling edge
                self.previous_logic_level = Some(logic_level);
                return;
            }
        }
        self.previous_logic_level = Some(logic_level);
        let now_millis = chrono::Utc::now().timestamp_millis();
        self.flow_count += 1;

        /* RAH implementation of flow sensor */
        if self.time_measure_start == 0 {
            // if first pulse, record time
            self.volume = 0;
            self.time_measure_start = now_millis;
        }
        if now_millis - self.time_measure_start < Self::MIN_MILLIS {
            // wait 90 seconds before recording time_begin
            self.volume = 0;
        } else {
            if self.volume == 1 {
                self.time_begin = now_millis;
            }
        }
        // get time in ms for stop
        self.time_measure_stop = now_millis;
        // increment gallon count for each poll
        self.volume += 1;
    }

    /// Reset the current flow measurement state
    pub fn reset(&mut self) {
        self.time_measure_start = 0;
    }

    /// last flow rate measured (averaged over flow_gallons) from last valve stopped (used to write to log file).
    /// 
    /// L/min
    pub fn measure(&mut self) -> f64 {
        if self.volume > 1 {
            // RAH calculate GPM, 1 pulse per gallon
            if self.get_duration() > 0 {
                return 60000.0 / (self.get_duration() as f64 / (self.volume - 1) as f64);
            }
        }

        // RAH if not one gallon (two pulses) measured then return 0 gpm
        return 0.0;
    }

    pub fn get_flow_count(&self) -> i64 {
        self.flow_count
    }

    fn get_duration(&self) -> i64 {
        self.time_measure_stop - self.time_measure_start
    }
}

impl Default for FlowState {
    fn default() -> Self {
        Self {
            time_begin: 0,
            time_measure_start: 0,
            time_measure_stop: 0,
            volume: 0,
            flow_count: 0,
            previous_logic_level: None,

            count_log_start: 0,
            count_realtime_now: 0,
            count_realtime_start: 0,
        }
    }
}

#[derive(Default)]
pub struct WeatherState {
    /// time when weather was checked (seconds)
    pub checkwt_lasttime: Option<i64>,

    /// time when weather check was successful (seconds)
    pub checkwt_success_lasttime: Option<i64>,

    /// Result of the most recent request to the weather service
    pub last_response_code: Option<weather::ErrorCode>,

    /// Data returned by the weather service (used by web server)
    pub raw_data: weather::WeatherServiceRawData,
}

impl WeatherState {
    pub fn last_response_was_successful(&self) -> bool {
        self.last_response_code == Some(weather::ErrorCode::Success)
    }
}

pub struct ControllerState {
    pub station: StationState,
    pub program: ProgramState,
    pub rain_delay: RainDelayState,
    pub sensor: SensorStateVec,
    pub flow: FlowState,
    /// Weather service status
    pub weather: WeatherState,
    /// time when controller is powered up most recently (seconds)
    ///
    /// When the process was started
    pub startup_time: chrono::DateTime<chrono::Utc>,

    /// A safe reboot has been requested
    pub reboot_request: bool,

    /// Timestamp to reboot
    pub reboot_timestamp: i64,
}

impl Default for ControllerState {
    fn default() -> Self {
        Self {
            station: StationState::default(),
            program: ProgramState::default(),
            rain_delay: RainDelayState::default(),
            sensor: SensorStateVec::new(),
            flow: FlowState::default(),
            weather: WeatherState::default(),
            startup_time: chrono::Utc::now(),
            reboot_request: false,
            reboot_timestamp: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use crate::opensprinkler::gpio;


    /// Test Flow Pulse
    /// 
    /// Simulation:
    /// - Sensor: 1L/pulse
    /// - Duration: 10 seconds.
    /// - Pulses: 50
    /// - Total flow: 5L/s or 300L/min
    #[test]
    fn test_flow_state() {
        let mut flow_state = super::FlowState::default();

        let duration = 10;
        let pulses = 50;
        let sleep_dur = Duration::from_secs_f64(duration as f64 / pulses as f64);

        for p in 0..pulses {
            flow_state.poll(gpio::Level::Low);
            flow_state.poll(gpio::Level::High);

            thread::sleep(sleep_dur);

            assert_eq!(flow_state.get_flow_count(), p + 1);
        }

        assert_eq!(flow_state.get_flow_count(), pulses, "Testing {} pulses", pulses);
        assert_eq!(flow_state.get_duration() / 1000, duration, "Testing {}s duration", duration);
        // todo: determine what the measurement value should be
    }
}