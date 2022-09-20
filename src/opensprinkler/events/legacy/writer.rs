use std::{
    fs,
    io::{self, Seek, Write},
    path::PathBuf,
};

use crate::opensprinkler::Controller;

use super::LogEvent;

pub fn write_log(open_sprinkler: &Controller, event: impl LogEvent) -> io::Result<()> {
    if !open_sprinkler.config.is_logging_enabled() {
        return Ok(());
    }

    let timestamp = chrono::Utc::now().timestamp();

    let mut file_path = PathBuf::new();
    file_path.push(&open_sprinkler.config.event_log.path);
    file_path.push(format!("{}.txt", timestamp / 86400));

    // Create directory, if missing
    if !open_sprinkler.config.event_log.path.exists() {
        fs::create_dir_all(&open_sprinkler.config.event_log.path)?;
    }

    // Open (or create) log file and seek to end
    let file = fs::OpenOptions::new().write(true).create(true).open(file_path)?;
    let mut writer = io::BufWriter::new(file);
    writer.seek(io::SeekFrom::End(0))?;

    if let Ok(buf) = event.try_into_bytes() {
        writer.write_all(&buf)?;
    }

    Ok(())
}

pub fn delete_log(day: Option<u64>) {
    if let Some(day) = day {
        // Delete specific day
    } else {
        // Delete all files
    }
}
