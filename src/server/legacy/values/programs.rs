use serde::{de, Deserialize, Deserializer, Serialize};

use crate::{
    opensprinkler::{program, station},
    utils,
};

#[derive(Debug)]
pub struct ProgramData {
    pub flag: u8,
    pub days: [u8; 2],
    pub start_times: [i16; program::MAX_NUM_START_TIMES],
    pub durations: [u16; station::MAX_NUM_STATIONS],
    pub name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProgramDataLegacy(
    /// flags
    u8,
    /// days0
    u8,
    /// days1
    u8,
    /// start_times
    [i16; program::MAX_NUM_START_TIMES],
    /// durations
    Vec<u16>,
    /// name (only serialized)
    #[serde(skip_deserializing)]
    Option<String>,
);

impl ProgramDataLegacy {
    pub fn new(flags: u8, days: [u8; 2], start_times: [i16; program::MAX_NUM_START_TIMES], durations: Vec<u16>, name: &str) -> Self {
        Self(flags, days[0], days[1], start_times, durations, Some(name.into()))
    }

    pub fn flag(&self) -> u8 {
        self.0
    }

    pub fn days0(&self) -> u8 {
        self.1
    }

    pub fn days1(&self) -> u8 {
        self.2
    }

    pub fn start_times(&self) -> [i16; program::MAX_NUM_START_TIMES] {
        self.3
    }

    pub fn durations(&self) -> [u16; station::MAX_NUM_STATIONS] {
        let mut durations = Vec::new();
        durations.extend_from_slice(&self.4);
        durations.resize_with(station::MAX_NUM_STATIONS, || 0);
        durations.as_slice().try_into().unwrap()
    }

    pub fn name(self) -> Option<String> {
        self.5
    }
}

impl From<&program::Program> for ProgramDataLegacy {
    /// [flags, days0, days1, [start0, start1, start2, start3], [dur0, dur1, dur2, ...], name]
    fn from(program: &program::Program) -> Self {
        // Convert days remaining to relative (only for interval programs with one or more interval days)
        let days = if program.schedule_type == program::ProgramScheduleType::Interval && program.days[1] >= 1 {
            drem_to_relative(&program.days)
        } else {
            [program.days[0], program.days[1]]
        };
        let flags: u8 = LegacyProgramFlags::from(&*program).into();
        Self(flags, days[0], days[1], program.start_times, program.durations.to_vec(), Some(program.name.to_owned()))
    }
}

impl Into<ProgramData> for ProgramDataLegacy {
    fn into(self) -> ProgramData {
        ProgramData {
            flag: self.flag(),
            days: [self.days0(), self.days1()],
            start_times: self.start_times(),
            durations: self.durations(),
            name: self.name(),
        }
    }
}

impl<'de> de::Deserialize<'de> for ProgramData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        serde_json::from_str(de::Deserialize::deserialize(deserializer)?).map(|data: ProgramDataLegacy| data.into()).map_err(de::Error::custom)
    }
}

/// Flags
/// - bit 0:   program enable
/// - bit 1:   use weather adjustment
/// - bit 2-3: odd/even restriction   (0: none; 1: odd-day restriction; 2: even-day restriction; 3: undefined)
/// - bit 4-5: program schedule type  (0: weekday; 1: undefined; 2: undefined; 3: interval day)
/// - bit 6:   start time type        (0: repeating type; 1: fixed time type)
/// - bit 7:   undefined
///
/// ````
///       7             6             5             4             3             2             1             0
/// +-------------+-------------+-------------+-------------+-------------+-------------+-------------+-------------+
/// |             | Start Type  | Schedule Type             | Odd/Even                  | Use Weather | Enable      |
/// ````
pub struct LegacyProgramFlags {
    pub enabled: bool,
    pub use_weather: bool,
    pub odd_even: u8,
    pub schedule_type: program::ProgramScheduleType,
    pub start_time_type: program::ProgramStartTime,
}

impl From<&program::Program> for LegacyProgramFlags {
    fn from(program: &program::Program) -> Self {
        Self {
            enabled: program.enabled,
            use_weather: program.use_weather,
            odd_even: program.odd_even,
            schedule_type: program.schedule_type.to_owned(),
            start_time_type: program.start_time_type.to_owned(),
        }
    }
}

impl From<u8> for LegacyProgramFlags {
    fn from(flag: u8) -> Self {
        Self {
            enabled: utils::get_bit_flag_bool(flag, 0),
            use_weather: utils::get_bit_flag_bool(flag, 1),
            odd_even: utils::get_bit_flag(flag, 2, 0b11),
            schedule_type: match utils::get_bit_flag(flag, 4, 0b11) {
                0 => program::ProgramScheduleType::Weekly,
                1 => program::ProgramScheduleType::BiWeekly,
                2 => program::ProgramScheduleType::Monthly,
                3 => program::ProgramScheduleType::Interval,
                _ => unreachable!("3 is the greatest value that can be represented by 2 bits"),
            },
            start_time_type: match utils::get_bit_flag(flag, 6, 0b1) {
                0 => program::ProgramStartTime::Repeating,
                1 => program::ProgramStartTime::Fixed,
                _ => unreachable!("1 is the greatest value that can be represented by 1 bit"),
            },
        }
    }
}

impl Into<u8> for LegacyProgramFlags {
    fn into(self) -> u8 {
        let mut flags: u8 = 0x00000000;

        if self.enabled {
            flags = utils::apply_bit_flag(flags, 0, 1);
        }

        if self.use_weather {
            flags = utils::apply_bit_flag(flags, 1, 1);
        }

        flags = utils::apply_bit_flag(flags, 2, self.odd_even);

        flags = utils::apply_bit_flag(
            flags,
            4,
            match self.schedule_type {
                program::ProgramScheduleType::Weekly => 0,
                program::ProgramScheduleType::BiWeekly => 1,
                program::ProgramScheduleType::Monthly => 2,
                program::ProgramScheduleType::Interval => 3,
            },
        );

        if self.start_time_type == program::ProgramStartTime::Fixed {
            flags = utils::apply_bit_flag(flags, 6, 1);
        }

        flags
    }
}

/// days remaining - absolute to relative reminder conversion
///
/// convert absolute remainder (reference time 1970 01-01) to relative remainder (reference time today)
/// absolute remainder is stored in flash, relative remainder is presented to web
fn drem_to_relative(days: &[u8; 2]) -> [u8; 2] {
    let [rem_abs, inv] = days;
    let now: u8 = (chrono::Utc::now().timestamp() / program::SECS_PER_DAY).try_into().unwrap();
    [((*rem_abs) + (*inv) - now % (*inv)) % (*inv), days[1]]
}
