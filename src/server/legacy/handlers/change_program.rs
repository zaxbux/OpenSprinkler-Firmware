use std::sync;

use actix_web::{web, Responder, Result};
use serde::Deserialize;

use crate::{
    opensprinkler::{program, OpenSprinkler},
    server::legacy::{
        self, error,
        values::programs::{LegacyProgramFlags, ProgramData},
    },
};

#[derive(Debug, Deserialize)]
pub struct ChangeProgramRequest {
    /// Program index (starting from 0). Acceptable range is -1 to N-1, where N is number of existing programs.
    /// If `-1`, this is adding a new program; otherwise this is modifying an existing program.
    #[serde(rename = "pid")]
    program_index: isize,
    /// program data (JSON array)
    #[serde(rename = "v")]
    data: ProgramData,
    /// Set program enabled/disabled
    #[serde(rename = "en", deserialize_with = "legacy::de::bool_from_int_option", default)]
    enabled: Option<bool>,
    /// use weather
    #[serde(rename = "uwt", deserialize_with = "legacy::de::bool_from_int_option", default)]
    use_water_scale: Option<bool>,
    /// program name
    #[serde(default)]
    name: Option<String>,
}

/// URI: `/cp`
pub async fn handler(open_sprinkler: web::Data<sync::Arc<sync::Mutex<OpenSprinkler>>>, parameters: web::Query<ChangeProgramRequest>) -> Result<impl Responder> {
    let mut open_sprinkler = open_sprinkler.lock().map_err(|_| error::InternalError::SyncError)?;

    // validate program_index
    if parameters.program_index < -1 || parameters.program_index >= open_sprinkler.config.programs.len() as isize {
        return Ok(error::ReturnErrorCode::DataOutOfBound);
    }

    let mut program = open_sprinkler.config.programs.get(parameters.program_index as usize).map(|p| p.clone()).unwrap_or_else(|| program::Program::default());

    // enabled/disabled
    if let Some(enabled) = parameters.enabled {
        if parameters.program_index < 0 {
            return Ok(error::ReturnErrorCode::DataOutOfBound);
        }

        program.enabled = enabled;
    }

    // use weather
    if let Some(use_water_scale) = parameters.use_water_scale {
        if parameters.program_index < 0 {
            return Ok(error::ReturnErrorCode::DataOutOfBound);
        }

        program.use_weather = use_water_scale;
    }

    // name
    if let Some(ref name) = parameters.name {
        if parameters.program_index >= 0 {
            program.name = name.to_string();
        }
    }

    // flags
    let flags: LegacyProgramFlags = parameters.data.flag.into();
    //program.enabled = ((parameters.data.flag >> 0) & 0x01) != 0;
    program.enabled = flags.enabled;
    //program.use_weather = ((parameters.data.flag >> 1) & 0x01) != 0;
    program.use_weather = flags.use_weather;
    //program.odd_even = (parameters.data.flag >> 2) & 0x03;
    program.odd_even = flags.odd_even;
    /* program.schedule_type = match (parameters.data.flag >> 4) & 0x03 {
        0 => program::ProgramScheduleType::Weekly,
        1 => program::ProgramScheduleType::BiWeekly,
        2 => program::ProgramScheduleType::Monthly,
        3 => program::ProgramScheduleType::Interval,
        _ => unimplemented!(),
    }; */
    program.schedule_type = flags.schedule_type;
    /* program.start_time_type = match (parameters.data.flag >> 6) & 0x01 {
        0 => program::ProgramStartTime::Repeating,
        1 => program::ProgramStartTime::Fixed,
        _ => unimplemented!(),
    }; */
    program.start_time_type = flags.start_time_type;

    // days
    program.days = parameters.data.days;

    // start times
    program.start_times = parameters.data.start_times;

    // durations
    // @todo limit to value of open_sprinkler.get_station_count(), set extras to 0
    program.durations = parameters.data.durations;

    // interval day remainder
    if program.schedule_type == program::ProgramScheduleType::Interval && program.days[1] >= 1 {
        drem_to_absolute(&mut program.days);
    }

    if parameters.program_index == -1 {
        // New program
        open_sprinkler.config.programs.push(program);
    } else {
        // Existing program
        open_sprinkler.config.programs[parameters.program_index as usize] = program;
    }

    open_sprinkler.config.write()?;

    Ok(error::ReturnErrorCode::Success)
}

/// days remaining - relative to absolute reminder conversion
fn drem_to_absolute(days: &mut [u8; 2]) {
    let [rem_rel, inv] = days;
    let now: u8 = (chrono::Utc::now().timestamp() / program::SECS_PER_DAY).try_into().unwrap();
    days[0] = (now + (*rem_rel)) % (*inv);
}

#[cfg(test)]
mod tests {
    use crate::{
        opensprinkler::program,
        server::legacy::values::programs::{ProgramData, ProgramDataLegacy},
    };

    #[test]
    fn serialize_program_data_legacy() {
        let program = program::Program {
            enabled: true,
            use_weather: true,
            odd_even: 0,
            schedule_type: program::ProgramScheduleType::Weekly,
            start_time_type: program::ProgramStartTime::Repeating,
            days: [127, 0],
            start_times: [0, 0, 0, 0],
            durations: [0u16; 200],
            name: "Program 1".into(),
        };

        //let program_data = ProgramDataLegacy::new(1, [127,0], [0,0,0,0], vec![120,0,0,0,0,0,0,0], "Program 1");
        let program_data: ProgramDataLegacy = (&program).into();

        assert_eq!(serde_json::to_string(&program_data).unwrap(), "[\
            3,\
            127,\
            0,\
            [0,0,0,0],\
            [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],\
            \"Program 1\"\
        ]".to_owned());
    }

    #[test]
    fn deserialize_program_data_legacy() {
        let program_str = "[1,127,0,[60,120,3600,0],[120,0,120,0,0,0,0,0]]";

        let program_data: ProgramDataLegacy = serde_json::from_str(program_str).unwrap();

        assert_eq!(program_data.flag(), 1);
        assert_eq!(program_data.days0(), 127);
        assert_eq!(program_data.days1(), 0);
        assert_eq!(program_data.start_times(), [60, 120, 3600, 0]);
        assert_eq!(program_data.durations()[0], 120);
        assert_eq!(program_data.durations()[1], 0);
        assert_eq!(program_data.durations()[2], 120);

        let program_data: ProgramData = program_data.into();

        assert_eq!(program_data.flag, 1);
        assert_eq!(program_data.days[0], 127);
        assert_eq!(program_data.days[1], 0);
        assert_eq!(program_data.start_times, [60, 120, 3600, 0]);
        assert_eq!(program_data.durations[0], 120);
        assert_eq!(program_data.durations[1], 0);
        assert_eq!(program_data.durations[2], 120);
    }
}
