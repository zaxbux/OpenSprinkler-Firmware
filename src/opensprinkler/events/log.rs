use std::{path, io::{self, Read, Write, Seek}, fs};

use serde::{Deserialize, Serialize};

use crate::opensprinkler::config;

use super::result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub path: path::PathBuf,

    /// Enabled events
    pub events: config::EventsEnabled,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            path: "./logs".into(),
            events: config::EventsEnabled::default(),
        }
    }
}

pub fn append(file: fs::File, data: &[u8]) -> result::Result<()> {
    let mut header = [0xDD, 0, 0, 0, 0];

    let mut reader = io::BufReader::new(&file);
    let mut writer = io::BufWriter::new(&file);

    let read_size = reader.read(&mut header)?;
    let count = get_header_size(&header, read_size).unwrap() + 1;
    set_header_size(&mut header, count);

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
