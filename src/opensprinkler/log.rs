/// @todo new logging format, configurable log directory, log crate?


use std::{
    fs::{self, OpenOptions},
    io::{Write, self},
    path::PathBuf,
};

#[derive(PartialEq)]
#[repr(u8)]
pub enum LogDataType {
    /// Format (sensor 1 != flow): [_program_,_station_,_duration_,_timestamp_]
    /// Format (sensor 1 = flow):  [_program_,_station_,_duration_,_timestamp_,_flow_]
    Station = 0x00,
    /// Format: [0,s1,_duration_|0,_timestamp_]
    Sensor1 = 0x01,
    /// Format: [0,rd,_duration_|0,_timestamp_]
    RainDelay = 0x02,
    /// Format: [0,wl,_water_level_,_timestamp_]
    WaterLevel = 0x03,
    /// Format: [_flow_count_|0,fl,_duration_|0,_timestamp_]
    FlowSense = 0x04,
    /// Format: [0,s2,_duration_|0,_timestamp_]
    Sensor2 = 0x05,
}

const T24H_SECS: i64 = 86400;

/// Delete log files.
///
/// ### Arguments
/// * `time` - If a [None] value, the log directory is emptied, otherwise the log file for the specified day is deleted.
pub fn delete_log(time: Option<i64>) -> io::Result<()> {
    let mut log_path = PathBuf::from("./logs/");

    if let Some(time) = time {
        // Delete specific file
        log_path.push((time / T24H_SECS).to_string());
        log_path.set_extension("txt");

        //if log_path.exists() {
            fs::remove_file(log_path)?;
        //}
    } else {
        // Empty the log directory
        // Note: we don't delete the directory since that could delete a symlink
        for file in fs::read_dir(log_path).unwrap() {
            fs::remove_file(file.unwrap().path())?;
        }
    }

    return Ok(());
}

fn get_log_type_name(log_type: &LogDataType) -> &'static str {
    match log_type {
        LogDataType::Station => "",
        LogDataType::Sensor1 => "s1",
        LogDataType::RainDelay => "rd",
        LogDataType::WaterLevel => "wl",
        LogDataType::FlowSense => "fl",
        LogDataType::Sensor2 => "s2",
    }
}

fn get_writer(timestamp: i64) -> Result<io::BufWriter<std::fs::File>, std::io::Error> {
    let mut log_path = PathBuf::from("./logs/");

    if !log_path.is_dir() {
        fs::create_dir_all(&log_path)?;
    }

    // file name will be logs/xxxxx.tx where xxxxx is the day in epoch time
    log_path.push((timestamp / T24H_SECS).to_string());
    log_path.set_extension("txt");

    Ok(io::BufWriter::new(OpenOptions::new().create(true).append(true).open(log_path)?))
}

pub mod message {
    use crate::opensprinkler::{station, program};

    use super::{get_log_type_name, LogDataType};

    pub trait Message {
        fn to_string(&self) -> String;
    }

    #[derive(Copy, Clone)]
    pub struct StationMessage {
        pub program_index: program::ProgramStart,
        pub station_index: station::StationIndex,
        pub duration: i64,
        pub timestamp: i64,
        pub flow: Option<f64>,
    }

    impl StationMessage {
        pub fn new(program_index: program::ProgramStart, station_index: station::StationIndex, duration: i64, timestamp: i64) -> StationMessage {
            StationMessage {
                program_index,
                station_index,
                duration,
                timestamp,
                flow: None,
            }
        }

        pub fn with_flow(&mut self, flow: f64) -> &mut Self {
            self.flow = Some(flow);
            self
        }
    }
    impl Message for StationMessage {
        fn to_string(&self) -> String {
            if self.flow.is_some() {
                serde_json::json!([self.program_index, self.station_index, self.duration, self.timestamp, format!("{:5.2}", self.flow.unwrap()),]).to_string()
            } else {
                serde_json::json!([self.program_index, self.station_index, self.duration, self.timestamp,]).to_string()
            }
        }
    }

    pub struct FlowSenseMessage {
        flow_count: i64,
        timestamp: i64,
        duration: Option<i64>,
    }

    impl FlowSenseMessage {
        pub fn new(flow_count: i64, timestamp: i64) -> FlowSenseMessage {
            FlowSenseMessage { flow_count, timestamp, duration: None }
        }

        pub fn with_duration(&mut self, duration: i64) -> &mut Self {
            if duration > 0 {
                self.duration = Some(duration)
            }
            self
        }
    }
    impl Message for FlowSenseMessage {
        fn to_string(&self) -> String {
            serde_json::json!([self.flow_count, get_log_type_name(&LogDataType::FlowSense), self.duration.unwrap_or(0), self.timestamp,]).to_string()
        }
    }

    pub struct WaterLevelMessage {
        water_scale: f32,
        timestamp: i64,
    }

    impl WaterLevelMessage {
        pub fn new(water_scale: f32, timestamp: i64) -> WaterLevelMessage {
            WaterLevelMessage { water_scale, timestamp }
        }
    }
    impl Message for WaterLevelMessage {
        fn to_string(&self) -> String {
            serde_json::json!([0, get_log_type_name(&LogDataType::WaterLevel), self.water_scale, self.timestamp,]).to_string()
        }
    }

    pub struct SensorMessage {
        log_type: LogDataType,
        timestamp: i64,
        duration: Option<i64>,
    }

    impl SensorMessage {
        pub fn new(log_type: LogDataType, timestamp: i64) -> SensorMessage {
            SensorMessage { log_type, timestamp, duration: None }
        }

        pub fn with_duration(&mut self, duration: i64) -> &mut Self {
            if duration > 0 {
                self.duration = Some(duration)
            }
            self
        }
    }
    impl Message for SensorMessage {
        fn to_string(&self) -> String {
            serde_json::json!([0, get_log_type_name(&self.log_type), self.duration.unwrap_or(0), self.timestamp,]).to_string()
        }
    }
}

pub fn write_log_message<T: message::Message>(message: T, timestamp: i64) -> Result<usize, std::io::Error> {
    get_writer(timestamp)?.write(&message.to_string().as_bytes())
}
