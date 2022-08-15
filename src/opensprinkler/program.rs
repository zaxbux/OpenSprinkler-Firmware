#![allow(deprecated)]

use std::{
    cmp::{max, min},
    collections,
};

use chrono::{Datelike, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

use super::{log, station};

const SECS_PER_MIN: u32 = 60;
const SECS_PER_HOUR: i64 = 3600;
const SECS_PER_DAY: i64 = SECS_PER_HOUR * 24;

const MAX_NUM_PROGRAMS: usize = 40;
pub const MAX_NUM_START_TIMES: usize = 4;
const PROGRAM_NAME_SIZE: usize = 32;
const RUNTIME_QUEUE_SIZE: usize = station::MAX_NUM_STATIONS;

const START_TIME_SUNRISE_BIT: u8 = 14;
const START_TIME_SUNSET_BIT: u8 = 13;
const START_TIME_SIGN_BIT: u8 = 12;

const PROGRAM_STRUCT_EN_BIT: u8 = 0;
const PROGRAM_STRUCT_UWT_BIT: u8 = 1;

pub const TEST_PROGRAM_ID: usize = 99;
pub const MANUAL_PROGRAM_ID: usize = 254;

pub type Programs = Vec<Program>;

#[derive(Clone, Serialize, Deserialize)]
#[repr(u8)]
pub enum ProgramType {
    Weekly = 0,
    #[deprecated]
    BiWeekly = 1,
    Monthly = 2,
    Interval = 3,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Program {
    /// Program enabled
    pub enabled: bool,
    /// Weather data
    pub use_weather: bool,
    /// Odd/Even day restriction
    pub odd_even: u8,
    /// Schedule type
    pub schedule_type: ProgramType,
    /// Start time type
    ///
    /// - `0` = repeating (give start time, repeat every, number of repeats)
    /// - `1` = fixed start time (give arbitrary start times up to MAX_NUM_STARTTIMEs)
    pub start_time_type: u8,
    pub days: [u8; 2],
    pub start_times: [i16; MAX_NUM_START_TIMES],
    #[serde(with = "BigArray")]
    pub durations: [u16; station::MAX_NUM_STATIONS],
    pub name: String,
}

impl Program {
    pub fn test_program(duration: u16) -> Program {
        Program {
            enabled: false,
            use_weather: false,
            odd_even: 0,
            schedule_type: ProgramType::Interval,
            start_time_type: 1,
            days: [0, 0],
            start_times: [-1; MAX_NUM_START_TIMES],
            durations: [duration; station::MAX_NUM_STATIONS],
            name: String::from(""),
        }
    }

    /// Check if a given time matches program's start time
    ///
    /// This also checks for programs that started the previous day and ran over night.
    /// @todo Test
    pub fn check_match(&self, sunrise_time: i16, sunset_time: i16, timestamp: i64) -> bool {
        if !self.enabled {
            return false;
        }

        let start = self.start_time_decode(sunrise_time, sunset_time, self.start_times[0]);
        let repeat = self.start_times[1];
        let interval = self.start_times[2];
        let current_minute = i16::try_from((timestamp % 86400) / 60).unwrap();

        // first, assume program starts today
        if self.check_day_match(timestamp) {
            // t matches the program's start day

            for i in 0..MAX_NUM_START_TIMES {
                if current_minute == self.start_time_decode(sunrise_time, sunset_time, self.start_times[i]) {
                    // if current_minute matches any of the given start times, return true
                    return true;
                }
            }
            return false;
        } else {
            // repeating type
            // if current_minute matches start time, return 1
            if current_minute == start {
                return true;
            }

            // otherwise, current_minute must be larger than start time, and interval must be non-zero
            if current_minute > start && interval != 0 {
                // check if we are on any interval match
                let c = (current_minute - start) / interval;
                if (c * interval == (current_minute - start)) && c <= repeat {
                    return true;
                }
            }
        }

        // to proceed, program has to be repeating type, and interval and repeat must be non-zero
        if self.start_time_type != 0 || interval == 0 {
            return false;
        }

        // next, assume program started the previous day and ran over night
        if self.check_day_match(timestamp - 86400) {
            // t-86400L matches the program's start day
            let c = (current_minute - start + 1440) / interval;
            if (c * interval == (current_minute - start + 1440)) && c <= repeat {
                return true;
            }
        }
        return false;
    }

    /// Decode a sunrise/sunset start time to actual start time
    /// @todo Test
    pub fn start_time_decode(&self, sunrise_time: i16, sunset_time: i16, t: i16) -> i16 {
        if (t >> 15) & 1 != 0 {
            return -1;
        }

        let mut offset: i16 = t & 0x7FF;

        if (t >> START_TIME_SIGN_BIT) & 1 != 0 {
            offset = -offset;
        }

        if (t >> START_TIME_SUNRISE_BIT) & 1 != 0 {
            // limit to 0
            return max(0, sunrise_time as i16 + offset);
        } else if (t >> START_TIME_SUNSET_BIT) & 1 != 0 {
            // limit to 1440
            return min(1440, sunset_time as i16 + offset);
        }

        return t;
    }

    /// check odd/even day restriction
    ///
    /// Returns `true` if the odd/even restriction is satisfied
    /// @todo Test
    fn check_odd_even(&self, day: u32, month: u32) -> Option<bool> {
        if self.odd_even == 1 {
            // Odd-numbered day restriction

            if day == 31 {
                return Some(false);
            }

            if day == 29 && month == 2 {
                return Some(false);
            }

            return Some((day % 2) == 1);
        } else if self.odd_even == 2 {
            // Even-numbered day restriction

            return Some((day % 2) == 0);
        }

        None
    }

    /// Check if a given time matches the program's start day
    ///
    /// @todo Test
    fn check_day_match(&self, t: i64) -> bool {
        let ti = Utc.timestamp(i64::try_from(t).unwrap(), 0);

        // check day match
        let day_match = match self.schedule_type {
            ProgramType::Weekly => self.match_weekly_program((ti.weekday().num_days_from_monday() as u8 + 5) % 7),
            ProgramType::BiWeekly => None,
            ProgramType::Monthly => self.match_monthly_program(ti.day().try_into().unwrap()),
            ProgramType::Interval => self.match_interval_program(t),
        };

        if day_match.unwrap_or(true) == false {
            return false;
        }

        if self.check_odd_even(ti.day(), ti.month()).unwrap_or(false) {
            return false;
        }

        return true;
    }

    /// @todo Test
    fn match_weekly_program(&self, week_day: u8) -> Option<bool> {
        if !(self.days[0] & (1 << week_day) != 0) {
            return Some(false);
        }

        None
    }

    /// @todo Test
    fn match_monthly_program(&self, day: u8) -> Option<bool> {
        if day != (self.days[0] & 0b11111) {
            return Some(false);
        }

        None
    }

    /// @todo Test
    fn match_interval_program(&self, timestamp: i64) -> Option<bool> {
        if (u8::try_from(timestamp / SECS_PER_DAY).unwrap() % self.days[1]) != self.days[0] {
            return Some(false);
        }
        None
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProgramStart {
    /// Test program (60 seconds per station)
    Test,
    /// Short test program (two seconds per station)
    TestShort,
    // Run-once program
    RunOnce,
    /// User program
    User(usize),
}

#[derive(Clone)]
pub struct QueueElement {
    /// Start time
    pub start_time: i64,
    /// Water time
    pub water_time: i64,
    /// Station
    pub station_index: station::StationIndex,
    /// Program
    pub program_index: ProgramStart,
}

impl QueueElement {
    pub fn new(start_time: i64, water_time: i64, station_index: station::StationIndex, program_index: ProgramStart) -> Self {
        Self {
            start_time,
            water_time,
            station_index,
            program_index,
        }
    }
}

pub type QueueElements = collections::VecDeque<QueueElement>;

pub struct ProgramQueue {
    pub queue: QueueElements,
    /// this array stores the queue element index for each scheduled station
    pub station_qid: [Option<usize>; station::MAX_NUM_STATIONS],
    /// Number of programs
    pub last_run: Option<log::message::StationMessage>,
    // the last stop time of a sequential station
    pub last_seq_stop_time: Option<i64>,
}
impl ProgramQueue {
    pub fn new() -> Self {
        ProgramQueue {
            queue: collections::VecDeque::new(),
            station_qid: [None; station::MAX_NUM_STATIONS],
            last_run: None,
            last_seq_stop_time: None,
        }
    }

    pub fn reset_runtime(&mut self) {
        self.last_seq_stop_time = None;
        self.station_qid = [None; station::MAX_NUM_STATIONS];
    }

    // this returns a pointer to the next available slot in the queue
    pub fn enqueue(&mut self, value: QueueElement) -> result::Result<&mut QueueElement> {
        if self.queue.len() < RUNTIME_QUEUE_SIZE {
            self.queue.push_back(value);
            return Ok(self.queue.back_mut().unwrap());
        }

        Err(result::ProgramError {
            message: String::from("runtime queue is full"),
        })
    }

    /// Remove an element from the queue
    pub fn dequeue(&mut self, qid: usize) {
        if qid >= self.queue.len() {
            return;
        }
        if qid < self.queue.len() - 1 {
            let _ = self.queue.remove(qid);
        }
    }

    /// Clean Queue
    ///
    /// This removes queue elements if:
    /// - water_time is not greater than zero; or
    /// - if current time is greater than element duration
    pub fn clean_queue(&mut self, now_seconds: i64) {
        for qi in 0..self.queue.len() {
            let q = self.queue.get(qi).unwrap();
            if !(q.water_time > 0) || now_seconds >= q.start_time + q.water_time {
                self.dequeue(qi);
            }
        }
    }

    /// Reset all stations
    ///
    /// This function sets the duration of every station to 0, which causes all stations to turn off in the next processing cycle.
    /// Stations will be logged
    pub fn reset_all_stations(&mut self) {
        // go through runtime queue and assign water time to 0
        for q in self.queue.iter_mut() {
            q.water_time = 0;
        }
    }

    /// Add a program
    ///
    /// @todo used by web server
    pub fn add(&mut self, _program: Program) {
        todo!();
    }

    /// Delete a program
    ///
    /// @todo used by web server
    pub fn remove(&mut self, _index: usize) {
        todo!();
    }

    /// Modify a program
    ///
    /// @todo used by web server
    pub fn modify(&self, _index: usize, _value: Program) {
        todo!();
    }

    /// Move a program up (i.e. swap a program with the one above it)
    ///
    /// @todo used by web server
    pub fn move_up(&self, _index: usize) {
        todo!();
    }

    /// Delete all programs
    ///
    /// @todo used by web server
    pub fn erase_all(&mut self) {
        todo!();
    }
}

/// days remaining - absolute to relative reminder conversion
/// convert absolute remainder (reference time 1970 01-01) to relative remainder (reference time today)
/// absolute remainder is stored in flash, relative remainder is presented to web
/// @todo move into server module
pub fn drem_to_relative(days: &mut [u8; 2]) {
    let [rem_abs, inv] = days;
    let now: u8 = (chrono::Utc::now().timestamp() / SECS_PER_DAY).try_into().unwrap();
    days[0] = ((*rem_abs) + (*inv) - now % (*inv)) % (*inv);
}

/// days remaining - relative to absolute reminder conversion
/// @todo move into server module
pub fn drem_to_absolute(days: &mut [u8; 2]) {
    let [rem_rel, inv] = days;
    let now: u8 = (chrono::Utc::now().timestamp() / SECS_PER_DAY).try_into().unwrap();
    days[0] = (now + (*rem_rel)) % (*inv);
}

pub mod result {
    use core::fmt;

    pub type Result<T> = std::result::Result<T, ProgramError>;

    #[derive(Debug)]
    pub struct ProgramError {
        pub message: String,
    }

    impl fmt::Display for ProgramError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "ProgramError {{ message: {} }}", self.message)
        }
    }
}
