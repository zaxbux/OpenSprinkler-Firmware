use std::{
    fs::{create_dir_all, read_dir, remove_file, OpenOptions},
    io::{Write, self},
    path::PathBuf,
};

use super::OpenSprinkler;

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
    /*     #[deprecated(
        since = "3.0.0",
        note = "OpenSprinkler Pi hardware does not include current sensing circuit"
    )]
    Current = 0x80, */
}

const T24H_SECS: i64 = 86400;

/// Delete log files.
///
/// ### Arguments
/// * `time` - If a [None] value, the log directory is emptied, otherwise the log file for the specified day is deleted.
pub fn delete_log(time: Option<i64>) -> io::Result<()> {
    let log_path = PathBuf::from("./logs/");

    if time.is_none() {
        // Empty the log directory
        // Note: we don't delete the directory since that could delete a symlink
        for file in read_dir(log_path).unwrap() {
            remove_file(file.unwrap().path())?;
        }
        return Ok(());
    }

    // Delete specific file
    let mut log_file_path = log_path.clone();
    log_file_path.set_file_name((time.unwrap() / 86400).to_string());
    log_file_path.set_extension("txt");

    if log_file_path.exists() {
        remove_file(log_file_path)?;
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
        //LogDataType::Current => "cu",
    }
}

fn get_writer(timestamp: i64) -> Result<io::BufWriter<std::fs::File>, std::io::Error> {
    let log_path = PathBuf::from("./logs/");

    // file name will be logs/xxxxx.tx where xxxxx is the day in epoch time
    let mut log_file_path = log_path.clone();
    log_file_path.set_file_name((timestamp / T24H_SECS).to_string());
    log_file_path.set_extension("txt");

    // Step 1 - open file if exists, or create new otherwise, and move file pointer to the end prepare log folder for RPI
    if !log_path.is_dir() {
        create_dir_all(log_path)?;
    }

    Ok(io::BufWriter::new(OpenOptions::new().create(true).append(true).open(log_file_path)?))
}

pub mod message {
    use std::io::Write;

    //use serde::{Deserialize, Serialize};
    use super::{get_log_type_name, LogDataType, OpenSprinkler};

    pub trait Message {
        fn to_string(&self) -> String;
    }

    pub trait Writable {
        fn write(&self, open_sprinkler: &OpenSprinkler, curr_time: i64) -> Option<Result<usize, std::io::Error>>;
    }

    impl<T> Writable for T
    where
        T: Message,
    {
        fn write(&self, open_sprinkler: &OpenSprinkler, curr_time: i64) -> Option<Result<usize, std::io::Error>> {
            open_sprinkler.is_logging_enabled().then(|| super::get_writer(curr_time)?.write(self.to_string().as_bytes()))
        }
    }

    #[derive(Copy, Clone)]
    pub struct StationMessage {
        pub program_id: usize,
        pub station_id: usize,
        pub duration: u16,
        pub timestamp: i64,
        pub flow: Option<f32>,
    }

    impl StationMessage {
        pub fn new(program_id: usize, station_id: usize, duration: u16, timestamp: i64) -> StationMessage {
            StationMessage {
                program_id,
                station_id,
                duration,
                timestamp,
                flow: None,
            }
        }

        pub fn with_flow(&mut self, flow: f32) -> &mut Self {
            self.flow = Some(flow);
            self
        }
    }
    impl Message for StationMessage {
        fn to_string(&self) -> String {
            if self.flow.is_some() {
                serde_json::json!([self.program_id, self.station_id, self.duration, self.timestamp, format!("{:5.2}", self.flow.unwrap()),]).to_string()
            } else {
                serde_json::json!([self.program_id, self.station_id, self.duration, self.timestamp,]).to_string()
            }
        }
    }

    pub struct FlowSenseMessage {
        flow_count: u32,
        timestamp: i64,
        duration: Option<i64>,
    }

    impl FlowSenseMessage {
        pub fn new(flow_count: u32, timestamp: i64) -> FlowSenseMessage {
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
        water_level: u8,
        timestamp: i64,
    }

    impl WaterLevelMessage {
        pub fn new(water_level: u8, timestamp: i64) -> WaterLevelMessage {
            WaterLevelMessage { water_level, timestamp }
        }
    }
    impl Message for WaterLevelMessage {
        fn to_string(&self) -> String {
            serde_json::json!([0, get_log_type_name(&LogDataType::WaterLevel), self.water_level, self.timestamp,]).to_string()
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

pub fn write_log_message(open_sprinkler: &OpenSprinkler, message: &dyn message::Message, timestamp: i64) -> Result<(), std::io::Error> {
    open_sprinkler.is_logging_enabled().then(|| get_writer(timestamp)?.write(message.to_string().as_bytes()));

    Ok(())
}
