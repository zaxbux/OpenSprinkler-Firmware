use super::{gpio, sensor, station, weather, controller};

pub type StationBits = [bool; controller::SHIFT_REGISTER_LINES];


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
            active: [[false; controller::SHIFT_REGISTER_LINES]; station::MAX_NUM_BOARDS],
            auto_refresh_next_index: station::MAX_NUM_STATIONS >> 1,
            auto_refresh_timestamp: 0,
        }
    }
}

pub struct ProgramState {
    /// A program is currently being executed
    pub busy: bool,
}

impl Default for ProgramState {
    fn default() -> Self {
        Self { busy: false }
    }
}

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
pub struct FlowState {
    /// Flow count (initial)
    pub count_log_start: u64,
    /// Flow count
    pub count_realtime_now: u64,
    /// Flow count
    pub count_realtime_start: u64,

    /* Flow Rate */
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

    previous_logic_level: Option<gpio::Level>,
}

impl FlowState {
    pub fn poll(&mut self, logic_level: gpio::Level) {
        if self.previous_logic_level.unwrap_or(gpio::Level::Low) == gpio::Level::Low && logic_level != gpio::Level::Low {
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

impl Default for FlowState {
    fn default() -> Self {
        Self {
            time_begin: 0,
            time_measure_start: 0,
            time_measure_stop: 0,
            gallons: 0,
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
