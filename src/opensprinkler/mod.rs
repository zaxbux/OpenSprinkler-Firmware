pub mod config;
pub mod events;
pub mod gpio;
mod http;
pub mod program;
#[cfg(feature = "station-rf")]
mod rf;
pub mod sensor;
pub mod station;
pub mod weather;

pub mod errors;

mod mqtt;
pub mod scheduler;
pub mod state;
#[cfg(target_os = "linux")]
pub mod system;

use std::cmp::max;
use std::path::PathBuf;

use crate::utils;

use self::http::request;
use self::station::MAX_MASTER_STATIONS;

/// Default reboot timer (seconds)
pub const REBOOT_DELAY: i64 = 65;

pub const MINIMUM_ON_DELAY: u8 = 5;
pub const MINIMUM_OFF_DELAY: u8 = 5;

const SPECIAL_CMD_REBOOT: &'static str = ":>reboot";
const SPECIAL_CMD_REBOOT_NOW: &'static str = ":>reboot_now";

/// Flow Count Window (seconds)
///
/// For computing real-time flow rate.
pub const FLOW_COUNT_REALTIME_WINDOW: i64 = 30;

pub struct Controller {
    pub config: config::Config,
    pub state: state::ControllerState,
    gpio: Option<gpio::Gpio>,
    pub events: events::Events,
}

impl Controller {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config_path(config_path: PathBuf) -> Self {
        Self {
            config: config::Config::new(config_path),
            state: state::ControllerState::default(),
            events: events::Events::new().unwrap(),
            gpio: None,
        }
    }

    /// Setup controller
    pub fn setup(&mut self) -> errors::Result<()> {
        // Read configuration from file
        if !self.config.exists() {
            tracing::debug!("Config file does not exist");
            self.config.write_default()?;
        }

        // Check reset conditions
        if let Ok(config) = self.config.read() {
            if config.check()? {
                // Replace defaults with config from file.
                self.config = config;
            }
        }

        #[cfg(not(feature = "demo"))]
        self.setup_gpio();

        self.events.setup(&self.config);

        // Store the last reboot cause in memory and set the new cause
        self.state.last_reboot_cause = self.config.reboot_cause;
        self.config.reboot_cause = config::RebootCause::PowerOn;
        self.config.write()?;

        Ok(())
    }

    /// Setup GPIO peripheral and pins
    #[cfg(not(feature = "demo"))]
    fn setup_gpio(&mut self) {
        // Setup GPIO peripheral
        let gpio = gpio::Gpio::new();
        if let Err(ref error) = gpio {
            tracing::error!("Cannot access GPIO peripheral: {:?}", error);
        } else if let Ok(gpio) = gpio {
            self.gpio = Some(gpio);
        }

        // Setup GPIO pins
        if let Some(ref gpio) = self.gpio {
            if let Err(ref error) = gpio.get(gpio::SHIFT_REGISTER_OE).and_then(|pin| Ok(pin.into_output().set_high())) {
                tracing::error!("GPIO Error (SHIFT_REGISTER_OE): {:?}", error);
            }
            if let Err(ref error) = gpio.get(gpio::SHIFT_REGISTER_LATCH).and_then(|pin| Ok(pin.into_output().set_high())) {
                tracing::error!("GPIO Error (SHIFT_REGISTER_LATCH): {:?}", error);
            }
            if let Err(ref error) = gpio.get(gpio::SHIFT_REGISTER_CLOCK).and_then(|pin| Ok(pin.into_output().set_high())) {
                tracing::error!("GPIO Error (SHIFT_REGISTER_CLOCK): {:?}", error);
            }
            if let Err(ref error) = gpio.get(gpio::SHIFT_REGISTER_DATA).and_then(|pin| Ok(pin.into_output().set_high())) {
                tracing::error!("GPIO Error (SHIFT_REGISTER_DATA): {:?}", error);
            }
            for i in 0..sensor::MAX_SENSORS {
                if let Err(ref error) = gpio.get(gpio::SENSOR[i]).and_then(|pin| Ok(pin.into_input_pullup().set_reset_on_drop(false))) {
                    // @todo Catch abnormal process terminations and reset pullup
                    tracing::error!("GPIO Error (SENSOR[{}]): {:?}", i, error);
                }
            }
            if let Err(ref error) = gpio.get(gpio::RF_TX).and_then(|pin| Ok(pin.into_output().set_low())) {
                tracing::error!("GPIO Error (RF_TX): {:?}", error);
            }
        }
    }

    pub fn push_event(&self, event: &dyn events::Event) {
        if let Err(ref err) = self.events.push(&self.config, event) {
            tracing::error!("MQTT Push Error: {:?}", err);
        }
    }

    /// Starts the MQTT client if it is enabled, configured, and the network is connected.

    pub fn try_mqtt_connect(&self) {
        if self.network_connected() && self.config.is_mqtt_enabled() && self.config.mqtt.uri().is_some() && !self.events.mqtt_client.is_connected() {
            self.events.mqtt_client.connect(events::Events::mqtt_connect_options(&self.config.mqtt));
        }
    }

    // region: GETTERS

    pub fn is_station_running(&self, station_index: station::StationIndex) -> bool {
        /* let bid = station_index >> 3;
        let s = station_index & 0x07;
        self.station_bits[bid] & (1 << s) != 0 */
        self.state.station.is_active(station_index)
    }

    /// Update the weather service request success timestamp
    pub fn update_check_weather_success_timestamp(&mut self) {
        self.state.weather.last_request_success_timestamp = Some(chrono::Utc::now().timestamp());
    }

    /// Realtime flow count
    pub fn update_realtime_flow_count(&mut self, now_seconds: i64) {
        if self.config.is_flow_sensor_enabled() && now_seconds % FLOW_COUNT_REALTIME_WINDOW == 0 {
            self.state.flow.count_realtime_now = max(0, self.state.flow.get_flow_count() - self.state.flow.count_realtime_start);
            self.state.flow.count_realtime_start = self.state.flow.get_flow_count();
        }
    }

    pub fn check_reboot_request(&mut self, now_seconds: i64) {
        if self.state.reboot_request && (now_seconds > self.state.reboot_timestamp) {
            // if no program is running at the moment and if no program is scheduled to run in the next minute
            if !self.state.program.busy && !self.program_pending_soon(now_seconds + 60) {
                self.reboot_dev(self.config.reboot_cause).unwrap();
            }
        } else if self.state.reboot_timestamp != 0 && (now_seconds > self.state.reboot_timestamp) {
            self.reboot_dev(config::RebootCause::Timer).unwrap();
        }
    }

    fn program_pending_soon(&self, timestamp: i64) -> bool {
        for program in self.config.programs.iter() {
            if program.check_match(self.config.get_sunrise_time() as i16, self.config.get_sunset_time() as i16, timestamp) {
                return true;
            }
        }
        return false;
    }

    /// @todo Define primary interface e.g. `eth0` and check status (IFF_UP).
    pub fn network_connected(&self) -> bool {
        if cfg!(not(feature = "demo")) {
            #[cfg(target_os = "linux")]
            return system::is_interface_online("eth0").unwrap_or(false);
        }

        return true;
    }

    pub fn get_hw_mac(&self) -> Option<mac_address::MacAddress> {
        // Use primary interface and get mac from it.
        let primary_iface = "eth0";

        mac_address::mac_address_by_name(primary_iface).unwrap_or(None)
    }

    pub fn reboot_dev(&mut self, cause: config::RebootCause) -> config::result::Result<()> {
        self.config.reboot_cause = cause;
        self.config.write()?;

        if cfg!(not(feature = "demo")) {
            // reboot via commandline, dbus, libc::reboot, etc.
            todo!();
        }

        Ok(())
    }

    /// Update software
    pub fn update_dev(&self) -> config::result::Result<()> {
        todo!();
    }

    #[cfg(not(feature = "demo"))]
    pub fn flow_poll(&mut self) {
        if let Some(ref gpio) = self.gpio {
            let sensor1_pin = gpio.get(gpio::SENSOR[0]).and_then(|pin| Ok(pin.into_input()));

            if let Err(ref error) = sensor1_pin {
                tracing::error!("GPIO Error (SENSOR[0]): {:?}", error);
            } else if let Ok(pin) = sensor1_pin {
                // Perform calculations using the current state of the sensor
                self.state.flow.poll(pin.read());
            }
        }
    }

    /// This function loops through the queue and schedules the start time of each station
    pub fn schedule_all_stations(&mut self, now_seconds: i64) {
        tracing::trace!("Scheduling all stations");
        let mut start_time_concurrent = now_seconds + 1; // concurrent start time
        let mut start_time_sequential = start_time_concurrent; // sequential start time

        let station_delay: i64 = utils::water_time_decode_signed(self.config.station_delay_time).into();

        // if the sequential queue has stations running
        if self.state.program.queue.last_seq_stop_time.unwrap_or(0) > now_seconds {
            start_time_sequential = self.state.program.queue.last_seq_stop_time.unwrap_or(0) + station_delay;
        }

        /*for qi in 0..open_sprinkler.state.program.queue.queue.len() {
        let mut q = &mut open_sprinkler.state.program.queue.queue[qi];*/
        for q in self.state.program.queue.queue.iter_mut() {
            // Skip if
            // - this queue element has already been scheduled; or
            // - if the element has been marked to reset
            if q.start_time > 0 || q.water_time == 0 {
                continue;
            }

            // if this is a sequential station and the controller is not in remote extension mode, use sequential scheduling. station delay time apples
            if self.config.stations[q.station_index].is_sequential() && self.config.enable_remote_ext_mode
            /* !open_sprinkler.is_remote_extension() */
            {
                // sequential scheduling
                q.start_time = start_time_sequential;
                // Update sequential start time for next station
                start_time_sequential += q.water_time + station_delay;
            } else {
                // otherwise, concurrent scheduling
                q.start_time = start_time_concurrent;
                // stagger concurrent stations by 1 second
                start_time_concurrent += 1;
            }

            if !self.state.program.busy {
                self.state.program.busy = true;

                // start flow count
                //if open_sprinkler.is_flow_sensor_enabled() {
                if self.config.sensors[0].sensor_type == Some(sensor::SensorType::Flow) {
                    // if flow sensor is connected
                    //self.start_flow_log_count();
                    self.state.flow.count_log_start = self.state.flow.get_flow_count();
                    self.state.sensor.set_timestamp_activated(0, Some(now_seconds));
                }
            }
        }
    }

    /// Turn on a station
    pub fn turn_on_station(&mut self, station_index: station::StationIndex) {
        self.state.flow.reset();

        if self.state.station.set_active(station_index, true) == state::StationChange::Change(true) {
            let station_name = self.config.stations.get(station_index).unwrap().name.to_string();
            self.push_event(&events::StationEvent::new(true, station_index, &station_name));
        }
    }

    /// Turn off a station
    ///
    /// Turns off a scheduled station, writes a log record, and pushes a notification event.
    pub fn turn_off_station(&mut self, now_seconds: i64, station_index: station::StationIndex) {
        self.state.station.set_active(station_index, false);

        if let Some(qid) = self.state.program.queue.station_qid[station_index] {
            // ignore if we are turning off a station that is not running or is not scheduled to run
            if let Some(q) = self.state.program.queue.queue.get(qid) {
                // RAH implementation of flow sensor
                let flow_volume = if self.config.is_flow_sensor_enabled() { Some(self.state.flow.measure()) } else { None };

                // check if the current time is past the scheduled start time,
                // because we may be turning off a station that hasn't started yet
                if now_seconds > q.start_time {
                    // record lastrun log (only for non-master stations)
                    if !self.config.is_master_station(station_index) {
                        let duration = now_seconds - q.start_time;

                        /* // log station run
                        let message = data_log::StationData::new(
                            q.program_index,
                            station_index,
                            duration, // Maximum duration is 18 hours (64800 seconds), which fits into a [u16]
                            now_seconds,
                        ); */
                        let event = events::StationEvent::new(false, station_index, &self.config.stations[station_index].name)
                            .end_time(now_seconds)
                            .duration(duration)
                            .flow_volume(flow_volume)
                            .program_index(q.program_index)
                            .program_type(q.program_start_type);

                        // Keep a copy for web
                        //self.state.program.queue.last_run = Some(message);
                        self.state.program.queue.last_run = Some(event.clone());

                        /* if self.is_flow_sensor_enabled() {
                            message.flow(flow_volume.unwrap());
                        }
                        self.write_log_message(message); */
                        self.push_event(&event);
                    }
                }

                // dequeue the element
                self.state.program.queue.dequeue(qid);
                self.state.program.queue.station_qid[station_index] = None;
            }
        }
    }

    /// Apply all station bits
    ///
    /// **This will actuate valves**
    ///
    /// @todo verify original functionality
    pub fn apply_all_station_bits(&mut self) {
        #[cfg(not(feature = "demo"))]
        if let Some(ref gpio) = self.gpio {
            let shift_register_latch = gpio.get(gpio::SHIFT_REGISTER_LATCH).and_then(|pin| Ok(pin.into_output()));
            if let Err(ref error) = shift_register_latch {
                tracing::error!("GPIO Error (SHIFT_REGISTER_LATCH): {:?}", error);
            }

            let shift_register_clock = gpio.get(gpio::SHIFT_REGISTER_CLOCK).and_then(|pin| Ok(pin.into_output()));
            if let Err(ref error) = shift_register_clock {
                tracing::error!("GPIO Error (SHIFT_REGISTER_CLOCK): {:?}", error);
            }

            let shift_register_data = gpio.get(gpio::SHIFT_REGISTER_DATA).and_then(|pin| Ok(pin.into_output()));
            if let Err(ref error) = shift_register_data {
                tracing::error!("GPIO Error (SHIFT_REGISTER_DATA): {:?}", error);
            }

            if let (Ok(mut shift_register_latch), Ok(mut shift_register_clock), Ok(mut shift_register_data)) = (shift_register_latch, shift_register_clock, shift_register_data) {
                shift_register_latch.set_low();

                // Shift out all station bit values from the highest bit to the lowest
                for board_index in 0..station::MAX_EXT_BOARDS {
                    //let sbits = if self.config.enable_controller { self.station_bits[station::MAX_EXT_BOARDS - board_index] } else { 0 };
                    let sbits = match self.config.enable_controller {
                        false => [false; station::SHIFT_REGISTER_LINES],
                        true => self.state.station.active[station::MAX_EXT_BOARDS - board_index],
                    };

                    for s in 0..station::SHIFT_REGISTER_LINES {
                        shift_register_clock.set_low();

                        //if sbits & (1 << (7 - s)) != 0 {
                        if sbits[s] {
                            shift_register_data.set_high();
                            shift_register_data.set_low();
                        }

                        shift_register_clock.set_high();
                    }
                }

                shift_register_latch.set_high();
            }
        }

        if self.config.enable_special_stn_refresh {
            self.check_special_station_auto_refresh();
        }
    }

    /// Handle refresh of special stations
    ///
    /// Original implementation details: [OpenSprinkler/OpenSprinkler-Firmware@d8c1bc0](https://github.com/OpenSprinkler/OpenSprinkler-Firmware/commit/d8c1bc0)
    ///
    /// Refresh station that is next in line. This deliberately starts with station `101` to avoid startup delays.
    ///
    /// @todo Async
    /// @todo Skip non-special stations
    fn check_special_station_auto_refresh(&mut self) {
        let timestamp = chrono::Utc::now().timestamp();

        if timestamp > self.state.station.auto_refresh_timestamp() {
            // Perform this no more than once per second
            self.state.station.auto_refresh_timestamp = timestamp;
            self.state.station.auto_refresh_next_index = (self.state.station.auto_refresh_next_index + 1) % station::MAX_NUM_STATIONS;
            let board_index = self.state.station.auto_refresh_next_index >> 3;
            let line = self.state.station.auto_refresh_next_index & 0x07;
            //self.switch_special_station(self.state.station.auto_refresh_next_index, (self.station_bits[board_index] >> s) & 0x01 != 0);
            self.switch_special_station(self.state.station.auto_refresh_next_index, self.state.station.active[board_index][line]);
        }
    }

    /// Check rain delay status
    pub fn check_rain_delay_status(&mut self, now_seconds: i64) {
        if self.state.rain_delay.active_now {
            if now_seconds >= self.config.rain_delay_stop_time.unwrap_or(0) {
                // rain delay is over
                self.rain_delay_stop();
            }
        } else {
            if self.config.rain_delay_stop_time.unwrap_or(0) > now_seconds {
                // rain delay starts now
                //self.rain_delay_start();
                self.state.rain_delay.active_previous = false;
                self.state.rain_delay.active_now = true;
            }
        }

        // Check controller status changes and write log
        if self.state.rain_delay.active_previous != self.state.rain_delay.active_now {
            if self.state.rain_delay.active_now {
                // rain delay started, record time
                self.state.rain_delay.timestamp_active_last = Some(now_seconds);
            } /*  else {
                  // rain delay stopped, write log
                  self.write_log_message(data_log::RainDelayData::new(now_seconds));
              } */
            tracing::trace!("Rain Delay state changed {}", self.state.rain_delay.active_now);
            self.push_event(&events::RainDelayEvent::new(self.state.rain_delay.active_now, now_seconds, self.state.rain_delay.timestamp_active_last));
        }
    }

    /// @todo compare to original implementation
    #[cfg(not(feature = "demo"))]
    fn detect_sensor_status(&mut self, i: usize, now_seconds: i64) {
        if let Some(sensor::SensorType::Rain) | Some(sensor::SensorType::Soil) = self.config.get_sensor_type(i) {
            self.state.sensor.set_detected(i, self.get_sensor_detected(i));

            if self.state.sensor.detected(i) {
                if let Some(timestamp_on) = self.state.sensor.timestamp_on(i) {
                    if now_seconds > timestamp_on {
                        self.state.sensor.set_state(i, true);
                    }
                } else {
                    // add minimum of 5 seconds on-delay
                    self.state.sensor.set_timestamp_on(i, Some(max(self.config.get_sensor_on_delay(i) * 60, MINIMUM_ON_DELAY).into()));
                    self.state.sensor.set_timestamp_off(i, Some(0));
                }
            } else {
                if let Some(timestamp_off) = self.state.sensor.timestamp_off(i) {
                    if now_seconds > timestamp_off {
                        self.state.sensor.set_state(i, false);
                    }
                } else {
                    // add minimum of 5 seconds off-delay
                    self.state.sensor.set_timestamp_on(i, None);
                    self.state.sensor.set_timestamp_off(i, Some(max(self.config.get_sensor_off_delay(i) * 60, MINIMUM_OFF_DELAY).into()));
                }
            }
        }
    }

    /// Check binary sensor status (e.g. rain, soil)
    #[cfg(not(feature = "demo"))]
    pub fn check_binary_sensor_status(&mut self, now_seconds: i64) {
        for i in 0..sensor::MAX_SENSORS {
            self.detect_sensor_status(i, now_seconds);

            // State change
            if !self.state.sensor.state_equal(i) {
                // send notification when sensor becomes active
                if self.state.sensor.state(i) {
                    self.state.sensor.set_timestamp_activated(i, Some(now_seconds));
                } else {
                    /*let message = data_log::SensorData::new(i, now_seconds);
                    self.write_log_message(message, now_seconds);*/
                }
                self.push_event(&events::BinarySensorEvent::new(i, self.state.sensor.state(i), now_seconds, self.state.sensor.timestamp_activated(i)));
            }
            self.state.sensor.set_state_equal(i);
        }
    }

    /// Check program switch status
    #[cfg(not(feature = "demo"))]
    pub fn check_program_switch_status(&mut self) {
        if self.detect_program_switch_status() {
            // immediately stop all stations
            self.reset_all_stations_immediate();
        }

        for i in 0..sensor::MAX_SENSORS {
            if self.config.programs.len() > i {
                // Program switch sensors start the same program index
                self.manual_start_program(Some(i), program::ProgramStartType::User, false);
            }
        }
    }

    /// Immediately reset all stations
    ///
    /// No log records will be written
    pub fn reset_all_stations_immediate(&mut self) {
        self.clear_all_station_bits();
        self.apply_all_station_bits();
        self.state.program.queue.reset_runtime();
    }

    /// Check and process special program command
    pub fn process_special_program_command(&mut self, now_seconds: i64, program_name: &String) -> bool {
        if !program_name.starts_with(':') {
            return false;
        }

        if program_name == SPECIAL_CMD_REBOOT_NOW || program_name == SPECIAL_CMD_REBOOT {
            // reboot regardless of program status
            self.state.reboot_request = match program_name.as_str() {
                SPECIAL_CMD_REBOOT_NOW => false,
                SPECIAL_CMD_REBOOT => true,
                _ => true,
            };
            // set a timer to reboot in 65 seconds
            self.state.reboot_timestamp = now_seconds + REBOOT_DELAY;
            // this is to avoid the same command being executed again right after reboot
            return true;
        }

        false
    }

    /// Gets the current state of the sensor (evaluates the normal state)
    ///
    /// |                               | [gpio::Level::Low] | [gpio::Level::High] |
    /// | ----------------------------- | ------------------ | ------------------- |
    /// | [sensor::NormalState::Closed] | [false]            | [true]              |
    /// | [sensor::NormalState::Open]   | [true]             | [false]             |
    #[cfg(not(feature = "demo"))]
    fn get_sensor_detected(&self, i: usize) -> bool {
        let normal_state = self.config.get_sensor_normal_state(i);

        if let Some(ref gpio) = self.gpio {
            let sensor = gpio.get(gpio::SENSOR[i]).and_then(|pin| Ok(pin.into_input()));

            if let Err(ref error) = sensor {
                tracing::error!("GPIO Error (SENSOR[{}]): {:?}", i, error);
            } else if let Ok(sensor) = sensor {
                return match sensor.read() {
                    gpio::Level::Low => match normal_state {
                        sensor::NormalState::Closed => false,
                        sensor::NormalState::Open => true,
                    },
                    gpio::Level::High => match normal_state {
                        sensor::NormalState::Closed => true,
                        sensor::NormalState::Open => false,
                    },
                };
            }
        }

        false
    }

    /// Return program switch status
    #[cfg(not(feature = "demo"))]
    pub fn detect_program_switch_status(&mut self) -> bool /* [bool; sensor::MAX_SENSORS] */ {
        let mut detected = false /* [false; sensor::MAX_SENSORS] */;

        for i in 0..sensor::MAX_SENSORS {
            if self.config.get_sensor_type(i) == Some(sensor::SensorType::ProgramSwitch) {
                //self.status_current.sensors[i].detected = self.get_sensor_detected(i);
                //self.sensor_status[i].detected = self.get_sensor_detected(i);
                self.state.sensor.set_detected(i, self.get_sensor_detected(i));

                //self.sensor_status[i].history = (self.sensor_status[i].history << 1) | if self.status_current.sensors[i].detected { 1 } else { 0 };
                //self.sensor_status[i].history = (self.sensor_status[i].history << 1) | if self.sensor_status[i].detected { 1 } else { 0 };
                //self.state.sensor.set_history(i, (self.state.sensor.history(i) << 1) | if self.state.sensor.detected(i) { 1 } else { 0 });
                self.state.sensor.push_history(i, self.state.sensor.detected(i));

                // basic noise filtering: only trigger if sensor matches pattern:
                // i.e. two consecutive lows followed by two consecutive highs
                //if (self.sensor_status[i].history & 0b1111) == 0b0011 {
                /*if (self.state.sensor.history(i) & 0b1111) == 0b0011 {
                    detected[i] = true;
                }*/
                /* detected[i] = self.state.sensor.history_filter(i); */
                if self.state.sensor.history_filter(i) == true {
                    detected = true;
                }
            }
        }

        detected
    }

    /// Reset all sensor states
    ///
    /// @todo call if sensor configuration is changed
    pub fn sensor_reset_all(&mut self) {
        self.state.sensor.reset(None);
    }

    pub fn activate_master_stations(&mut self, now_seconds: i64) {
        for i in 0..MAX_MASTER_STATIONS {
            self.activate_master_station(i, now_seconds);
        }
    }

    /// Actuate master stations based on need
    ///
    /// This function iterates over all stations and activates the necessary "master" station.
    fn activate_master_station(&mut self, master_station: usize, now_seconds: i64) {
        let config = self.config.get_master_station(master_station);

        if let Some(station_index_master) = config.station {
            let adjusted_on = config.get_adjusted_on_time();
            let adjusted_off = config.get_adjusted_off_time();

            for station_index in 0..self.config.get_station_count() {
                // skip if this is the master station
                if station_index_master == station_index {
                    continue;
                }

                // if this station is running and is set to activate master
                if self.is_station_running(station_index) && self.config.stations[station_index].attrib.use_master[master_station] {
                    if let Some(qid) = self.state.program.queue.station_qid[station_index] {
                        if let Some(q) = self.state.program.queue.queue.get(qid) {
                            // check if timing is within the acceptable range
                            let start_time = q.start_time + adjusted_on;
                            let stop_time = q.start_time + q.water_time + adjusted_off;
                            if now_seconds >= start_time && now_seconds <= stop_time {
                                self.state.station.set_active(station_index_master, true);
                                return;
                            }
                        } else {
                            panic!("This should not happen");
                        }
                    } else {
                        panic!("This should not happen");
                    }
                }
            }
            self.state.station.set_active(station_index_master, false);
        }
    }

    /// Switch Radio Frequency (RF) station
    ///
    /// This function takes an RF code, parses it into signals and timing, and sends it out through the RF transmitter.
    fn switch_rf_station(&mut self, data: station::RFStationData, value: bool) {
        let code = if value { data.on } else { data.off };

        #[cfg(feature = "station-rf")]
        match rf::send_rf_signal(self.gpio.as_ref(), code.into(), data.timing.into()) {
            Ok(_) => {}
            Err(ref error) => tracing::error!("[RF Station] Error: {:?}", error),
        };
    }

    /// Switch GPIO station
    ///
    /// Special data for GPIO Station is three bytes of ascii decimal (not hex).
    fn switch_gpio_station(&self, data: station::GPIOStationData, value: bool) {
        tracing::trace!("[GPIO Station] pin: {} state: {}", data.pin, value);

        #[cfg(all(not(feature = "demo"), feature = "station-gpio"))]
        if let Some(ref gpio) = self.gpio {
            let pin = gpio.get(data.pin).and_then(|pin| Ok(pin.into_output()));

            if let Err(ref error) = pin {
                tracing::error!("GPIO Error (GPIO Station Pin {}): {:?}", data.pin, error);
            } else if let Ok(mut pin) = pin {
                pin.write(match value {
                    false => !data.active_level(),
                    true => data.active_level(),
                });
            }
        }
    }

    /// Switch Remote Station
    /// This function takes a remote station code, parses it into remote IP, port, station index, and makes a HTTP GET request.
    /// The remote controller is assumed to have the same password as the main controller.
    fn switch_remote_station(&self, data: station::RemoteStationData, value: bool) {
        let mut host = String::from("http://");
        host.push_str(&data.host.to_string());
        let timer = match self.config.enable_special_stn_refresh {
            true => (station::MAX_NUM_STATIONS * 4) as i64,
            false => 64800, // 18 hours
        };

        // @todo log request failures
        let client = request::build_client().unwrap();
        let response = client
            .get(host)
            .query(&http::request::RemoteStationRequestParametersV2_1_9::new(&self.config.device_key, data.station_index, value, timer))
            .send();

        if let Err(error) = response {
            tracing::error!("[Remote Station] HTTP request error: {:?}", error);
        }
    }

    /// Switch HTTP station
    ///
    /// This function takes an http station code, parses it into a server name and two HTTP GET requests.
    fn switch_http_station(&self, data: station::HTTPStationData, value: bool) {
        let mut origin: String = String::new();
        origin.push_str(&data.uri);
        if value {
            origin.push_str(&data.cmd_on);
        } else {
            origin.push_str(&data.cmd_off);
        }

        // @todo log request failures
        let response = reqwest::blocking::get(origin);
        if let Err(error) = response {
            tracing::error!("[HTTP Station] HTTP request error: {:?}", error);
        }
    }

    /// Switch Special Station
    pub fn switch_special_station(&mut self, station_index: station::StationIndex, value: bool) {
        if let Some(station) = self.config.stations.get(station_index) {
            match station.station_type {
                station::StationType::RadioFrequency => self.switch_rf_station(station::RFStationData::try_from(station.sped.as_ref().unwrap()).unwrap(), value),
                station::StationType::Remote => self.switch_remote_station(station::RemoteStationData::try_from(station.sped.as_ref().unwrap()).unwrap(), value),
                station::StationType::GPIO => self.switch_gpio_station(station::GPIOStationData::try_from(station.sped.as_ref().unwrap()).unwrap(), value),
                station::StationType::HTTP => self.switch_http_station(station::HTTPStationData::try_from(station.sped.as_ref().unwrap()).unwrap(), value),
                _ => (), // Nothing to do for [StationType::Standard] and [StationType::Other]
            }
        }
    }

    /// get free pins, minus pins used by firmware
    pub fn get_available_gpio_pins(&self) -> Vec<u8> {
        let mut free: Vec<u8> = Vec::new();

        if let Some(ref gpio) = self.gpio {
            for pin in u8::MIN..u8::MAX {
                if let Ok(_) = gpio.get(pin) {
                    if !gpio::is_pin_reserved(&pin) {
                        free.push(pin);
                    }
                }
            }
        }

        free
    }

    /// "Factory Reset
    ///
    /// This function should be called if the config does not exist.
    pub fn reset_to_defaults(&self) -> config::result::Result<()> {
        tracing::info!("Resetting controller to defaults.");
        self.config.write_default()
    }

    /// Enable controller operation
    ///
    /// Saves config.
    pub fn enable(&mut self) -> config::result::Result<()> {
        tracing::info!("Controller enabled");
        self.config.enable_controller = true;
        self.config.write()
    }

    /// Disable controller operation
    ///
    /// Saves config.
    pub fn disable(&mut self) -> config::result::Result<()> {
        tracing::info!("Controller disabled");
        self.config.enable_controller = false;
        self.config.write()
    }

    /// Start rain delay
    pub fn rain_delay_start(&mut self, rain_delay_stop_time: chrono::DateTime<chrono::Utc>) {
        self.state.rain_delay.active_previous = self.state.rain_delay.active_now;
        self.state.rain_delay.active_now = true;
        self.config.rain_delay_stop_time = Some(rain_delay_stop_time.timestamp());
        //self.config.write().unwrap();
    }

    /// Stop rain delay
    ///
    /// Saves config.
    pub fn rain_delay_stop(&mut self) {
        self.state.rain_delay.active_previous = self.state.rain_delay.active_now;
        self.state.rain_delay.active_now = false;
        self.config.rain_delay_stop_time = None;
        self.config.write().unwrap();
    }

    /// Clear all station bits
    pub fn clear_all_station_bits(&mut self) {
        self.state.station.clear();
    }

    /// Process dynamic events
    ///
    /// Processes events such as: Rain delay, rain sensing, station state changes, etc.
    pub fn process_dynamic_events(&mut self, now_seconds: i64) {
        // Determine which rain/soil sensors are currently active
        let mut sn = [false; sensor::MAX_SENSORS];
        for i in 0..sensor::MAX_SENSORS {
            if let Some(sensor::SensorType::Rain) | Some(sensor::SensorType::Soil) = self.config.get_sensor_type(i) {
                sn[i] = self.state.sensor.state(i);
            }
        }

        for board_index in 0..self.config.get_board_count() {
            for line in 0..station::SHIFT_REGISTER_LINES {
                let station_index = board_index * station::SHIFT_REGISTER_LINES + line;

                // Ignore master stations because they are handled separately
                if self.config.is_master_station(station_index) {
                    continue;
                }

                // If this is a normal program (not a run-once or test program) and either the controller is disabled, or if raining and ignore rain bit is cleared
                if let Some(qid) = self.state.program.queue.station_qid[station_index] {
                    if let Some(q) = self.state.program.queue.queue.get(qid).cloned() {
                        if q.program_start_type != program::ProgramStartType::User {
                            // This is a manually started program, skip
                            continue;
                        }

                        // If system is disabled, turn off zone
                        if !self.config.enable_controller {
                            self.turn_off_station(now_seconds, station_index);
                        }

                        // if rain delay is on and zone does not ignore rain delay, turn it off
                        if self.state.rain_delay.active_now && !self.config.stations[station_index].attrib.ignore_rain_delay {
                            self.turn_off_station(now_seconds, station_index);
                        }

                        for i in 0..sensor::MAX_SENSORS {
                            if sn[i] && !self.config.stations[station_index].attrib.ignore_sensor[i] {
                                self.turn_off_station(now_seconds, station_index);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Manually start a program
    pub fn manual_start_program(&mut self, program_index: Option<usize>, program_start_type: program::ProgramStartType, use_water_scale: bool) {
        let mut match_found = false;
        self.reset_all_stations_immediate();

        let program = match program_start_type {
            program::ProgramStartType::Test => program::Program::test_program(60),
            program::ProgramStartType::TestShort => program::Program::test_program(2),
            program::ProgramStartType::RunOnce => todo!(),
            program::ProgramStartType::User => self.config.programs[program_index.unwrap()].clone(),
        };

        let water_scale = if use_water_scale { self.config.water_scale } else { 1.0 };

        if program_start_type == program::ProgramStartType::User {
            self.push_event(&events::ProgramStartEvent::new(program_index.unwrap(), program.name, water_scale));
        }

        for station_index in 0..self.config.get_station_count() {
            // skip if the station is a master station (because master cannot be scheduled independently
            if self.config.is_master_station(station_index) {
                continue;
            }

            let water_time = match program_start_type {
                program::ProgramStartType::Test => 60.0,
                program::ProgramStartType::TestShort => 2.0,
                program::ProgramStartType::RunOnce => todo!(),
                program::ProgramStartType::User => utils::water_time_resolve(program.durations[station_index], self.config.get_sunrise_time(), self.config.get_sunset_time()),
            };

            let water_time = water_time * water_scale;

            if water_time > 0.0 && !self.config.stations.get(station_index).unwrap().attrib.is_disabled {
                if let Ok(Some(_)) = self.state.program.queue.enqueue(program::QueueElement::new(0, water_time as i64, station_index, None, program::ProgramStartType::Test)) {
                    match_found = true;
                }
            }
        }
        if match_found {
            self.schedule_all_stations(chrono::Utc::now().timestamp());
        }
    }
}

impl Default for Controller {
    fn default() -> Self {
        Self {
            config: config::Config::default(),
            state: state::ControllerState::default(),
            events: events::Events::new().unwrap(),
            gpio: None,
        }
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        if self.events.mqtt_client.is_connected() {
            self.events
                .mqtt_client
                .disconnect(paho_mqtt::DisconnectOptionsBuilder::new().reason_code(paho_mqtt::ReasonCode::DisconnectWithWillMessage).finalize());
        }
    }
}
