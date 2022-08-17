use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Seek, Write}, path::{PathBuf, Path},
};

use core::fmt::Debug;

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::opensprinkler::{program, station};

pub type Timestamp = chrono::DateTime<chrono::Utc>;

pub trait LogData: Debug + Serialize {
    const DIR: &'static str;
    fn get_timestamp(&self) -> Timestamp;
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct StationData {
    #[serde(rename = "ts")]
    pub timestamp: Timestamp,
    #[serde(rename = "program")]
    pub program_index: program::ProgramStart,
    #[serde(rename = "station")]
    pub station_index: station::StationIndex,
    pub duration: i64,
    pub flow: Option<f64>,
}

impl StationData {
    pub fn new(program_index: program::ProgramStart, station_index: station::StationIndex, duration: i64, timestamp: i64) -> Self {
        Self {
            program_index,
            station_index,
            duration,
            timestamp: chrono::DateTime::from_utc(chrono::NaiveDateTime::from_timestamp(timestamp, 0), chrono::Utc),
            flow: None,
        }
    }

    pub fn flow(mut self, flow: f64) -> Self {
        self.flow = Some(flow);
        self
    }
}

impl LogData for StationData {
    const DIR: &'static str = "station";

    fn get_timestamp(&self) -> Timestamp {
        self.timestamp
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FlowData {
    #[serde(rename = "ts")]
    pub timestamp: Timestamp,
    #[serde(rename = "count")]
    pub flow_count: i64,
    pub duration: Option<i64>,
}

impl FlowData {
    pub fn new(flow_count: i64, timestamp: i64) -> Self {
        Self { flow_count, timestamp: chrono::DateTime::from_utc(chrono::NaiveDateTime::from_timestamp(timestamp, 0), chrono::Utc), duration: None }
    }

    pub fn duration(mut self, duration: i64) -> Self {
        if duration > 0 {
            self.duration = Some(duration)
        }
        self
    }
}

impl LogData for FlowData {
    const DIR: &'static str = "flow";

    fn get_timestamp(&self) -> Timestamp {
        self.timestamp
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WaterScaleData {
    #[serde(rename = "ts")]
    pub timestamp: Timestamp,
    #[serde(rename = "scale")]
    pub water_scale: f32,
}

impl WaterScaleData {
    pub fn new(water_scale: f32, timestamp: i64) -> Self {
        Self { water_scale, timestamp: chrono::DateTime::from_utc(chrono::NaiveDateTime::from_timestamp(timestamp, 0), chrono::Utc) }
    }
}

impl LogData for WaterScaleData {
    const DIR: &'static str = "water_scale";

    fn get_timestamp(&self) -> Timestamp {
        self.timestamp
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SensorData {
    #[serde(rename = "ts")]
    pub timestamp: Timestamp,
    #[serde(rename = "i")]
    pub index: usize,
    pub duration: Option<i64>,
}

impl SensorData {
    pub fn new(index: usize, timestamp: i64) -> Self {
        Self { index, timestamp: chrono::DateTime::from_utc(chrono::NaiveDateTime::from_timestamp(timestamp, 0), chrono::Utc), duration: None }
    }

    pub fn duration(mut self, duration: i64) -> Self {
        if duration > 0 {
            self.duration = Some(duration)
        }
        self
    }
}

impl LogData for SensorData {
    const DIR: &'static str = "sensor";

    fn get_timestamp(&self) -> Timestamp {
        self.timestamp
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RainDelayData {
    #[serde(rename = "ts")]
    pub timestamp: Timestamp,
    pub duration: Option<i64>,
}

impl RainDelayData {
    pub fn new(timestamp: i64) -> Self {
        Self { timestamp: chrono::DateTime::from_utc(chrono::NaiveDateTime::from_timestamp(timestamp, 0), chrono::Utc), duration: None }
    }

    pub fn duration(mut self, duration: i64) -> Self {
        if duration > 0 {
            self.duration = Some(duration)
        }
        self
    }
}

impl LogData for RainDelayData {
    const DIR: &'static str = "rain_delay";

    fn get_timestamp(&self) -> Timestamp {
        self.timestamp
    }
}

pub struct DataLogger {
    log_dir: PathBuf,
}

impl DataLogger {
    pub fn new<S: AsRef<std::ffi::OsStr> + ?Sized>(log_dir: &S) -> Self {
        Self {
            log_dir: PathBuf::from(log_dir),
        }
    }

    pub fn read<T: LogData + DeserializeOwned>(&self, timestamp: Timestamp) -> result::Result<Vec<T>> {
        let mut path = self.log_dir.clone();
        path.push(T::DIR);

        // Filename
        path.push(timestamp.format("%Y%m%d").to_string());
        path.set_extension("bin");

        let file = OpenOptions::new().read(true).open(path)?;
        let reader = io::BufReader::new(&file);
        let decoded: Vec<T> = rmp_serde::decode::from_read(reader)?;
        Ok(decoded)
    }

    pub fn write<T: LogData>(&self, data: T) -> result::Result<()> {
        let mut path = self.log_dir.clone();
        path.push(T::DIR);

        // Create dir if necessary
        if !path.exists() {
            fs::create_dir_all(&path)?;
        } else {
            if !path.is_dir() {
                return Err(result::Error::IoError(io::Error::new(io::ErrorKind::AlreadyExists, "Log path is not a directory.")))
            }
        }

        // Filename
        path.push(chrono::Utc::now().format("%Y%m%d").to_string());
        path.set_extension("bin");

        let file = OpenOptions::new().read(true).write(true).create(true).open(path)?;

        let encoded = rmp_serde::encode::to_vec_named(&data).unwrap();
        self.append(file, encoded.as_slice())
    }

    /// Delete log files.
    ///
    /// ### Arguments
    /// * `time` - If a [None] value, the log directory is emptied, otherwise the log file for the specified day is deleted.
    pub fn delete<T: LogData>(&self, timestamp: Option<Timestamp>) -> result::Result<(Box<Path>, i32)> {
        let mut path = self.log_dir.clone();
        path.push(T::DIR);

        let mut counter = 0;

        if let Some(timestamp) = timestamp {
            // Filename
            path.push(timestamp.format("%Y%m%d").to_string());
            path.set_extension("bin");

            fs::remove_file(&path)?;
            counter += 1;
        } else {
            // Empty the log directory
            // Note: don't delete the directory since that could delete a symlink
            for entry in fs::read_dir(&path)? {
                if let Ok(entry) = entry {
                    fs::remove_file(entry.path())?;
                }
                counter += 1;
            }
        }

        Ok((path.into_boxed_path(), counter))
    }

    fn append(&self, file: fs::File, data: &[u8]) -> result::Result<()> {
        let mut header = [0xDD, 0, 0, 0, 0];

        let mut reader = io::BufReader::new(&file);
        let mut writer = io::BufWriter::new(&file);

        let read_size = reader.read(&mut header)?;
        let count = Self::get_header_size(&header, read_size).unwrap() + 1;
        Self::set_header_size(&mut header, count);

        writer.rewind()?;
        writer.write(&header)?;
        writer.seek(io::SeekFrom::End(0))?;
        writer.write(data)?;
        writer.flush()?;

        Ok(())
    }

    fn get_header_size(buf: &[u8], read_size: usize) -> result::Result<u32> {
        if read_size < 5 {
            if read_size > 0 && buf[0] != 0xDD {
                return Err(result::Error::DecodeError(rmp_serde::decode::Error::Uncategorized("Invalid file header.".into())));
            }
        }

        return Ok(u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]));
    }

    fn set_header_size(buf: &mut [u8], count: u32) {
        buf[0] = 0xDD;
        let (_, length) = buf.split_at_mut(1);
        length.copy_from_slice(&count.to_be_bytes());
    }
}

impl Default for DataLogger {
    fn default() -> Self {
        Self { log_dir: "./logs/".into() }
    }
}

pub mod result {
    use std::{io, error, result, fmt};

    pub type Result<T> = result::Result<T, Error>;

    #[derive(Debug)]
    pub enum Error {
        IoError(io::Error),
        EncodeError(rmp_serde::encode::Error),
        DecodeError(rmp_serde::decode::Error),
    }

    impl fmt::Display for Error {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match *self {
                Error::IoError(ref err) =>write!(f, "IO Error: {:?}", err),
                Error::EncodeError(ref err) =>write!(f, "Encode Error: {:?}", err),
                Error::DecodeError(ref err) =>write!(f, "Decode Error: {:?}", err),
            }
        }
    }

    impl error::Error for Error {}

    impl From<io::Error> for Error {
        fn from(err: io::Error) -> Self {
            Self::IoError(err)
        }
    }

    impl From<rmp_serde::encode::Error> for Error {
        fn from(err: rmp_serde::encode::Error) -> Self {
            Self::EncodeError(err)
        }
    }

    impl From<rmp_serde::decode::Error> for Error {
        fn from(err: rmp_serde::decode::Error) -> Self {
            Self::DecodeError(err)
        }
    }

}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};
    use serial_test::serial;
    use crate::opensprinkler::program;

    fn setup() -> super::DataLogger {
        let path = Path::new("./logs");

        if path.exists() {
            // Delete existing
            fs::remove_dir_all(path).expect("log directory should be deleted");
        }

        super::DataLogger::new(path)
    }

    #[test]
    #[serial]
    fn station_data_log() {
        let logger = setup();

        logger.write(super::StationData::new(program::ProgramStart::RunOnce, 0, 1000, chrono::Utc::now().timestamp()).flow(10.4)).expect("write");
        logger.write(super::StationData::new(program::ProgramStart::User(4), 1, 1200, chrono::Utc::now().timestamp())).expect("write");

        let data = logger.read::<super::StationData>(chrono::Utc::now()).expect("data should be read from log");

        assert_eq!(data.len(), 2);
        assert_eq!(data[0].flow, Some(10.4));
        assert_eq!(data[1].program_index, program::ProgramStart::User(4));
    }

    #[test]
    #[serial]
    fn sensor_data_log() {
        let logger = setup();

        logger.write(super::SensorData::new(0, chrono::Utc::now().timestamp())).expect("write");
        logger.write(super::SensorData::new(1, chrono::Utc::now().timestamp()).duration(120)).expect("write");

        let data = logger.read::<super::SensorData>(chrono::Utc::now()).expect("data should be read from log");

        assert_eq!(data.len(), 2);
        assert_eq!(data[0].index, 0);
        assert_eq!(data[1].duration, Some(120));
    }

    #[test]
    #[serial]
    fn flow_data_log() {
        let logger = setup();

        logger.write(super::FlowData::new(1000, chrono::Utc::now().timestamp())).expect("write");
        logger.write(super::FlowData::new(1200, chrono::Utc::now().timestamp()).duration(120)).expect("write");

        let data = logger.read::<super::FlowData>(chrono::Utc::now()).expect("data should be read from log");

        assert_eq!(data.len(), 2);
        assert_eq!(data[0].flow_count, 1000);
        assert_eq!(data[1].duration, Some(120));
    }

    #[test]
    #[serial]
    fn water_scale_data_log() {
        let logger = setup();

        logger.write(super::WaterScaleData::new(1.0, chrono::Utc::now().timestamp())).expect("write");
        logger.write(super::WaterScaleData::new(1.2, chrono::Utc::now().timestamp())).expect("write");

        let data = logger.read::<super::WaterScaleData>(chrono::Utc::now()).expect("data should be read from log");

        assert_eq!(data.len(), 2);
        assert_eq!(data[0].water_scale, 1.0);
        assert_eq!(data[1].water_scale, 1.2);
    }

    #[test]
    #[serial]
    fn rain_delay_data_log() {
        let logger = setup();

        logger.write(super::RainDelayData::new(chrono::Utc::now().timestamp())).expect("write");
        logger.write(super::RainDelayData::new(chrono::Utc::now().timestamp()).duration(120)).expect("write");

        let data = logger.read::<super::RainDelayData>(chrono::Utc::now()).expect("data should be read from log");

        assert_eq!(data.len(), 2);
        assert_eq!(data[0].duration, None);
        assert_eq!(data[1].duration, Some(120));
    }
}
