use crate::{
    opensprinkler::{program, Controller},
    server::legacy::{self, error},
};

use actix_web::{web, Responder, Result};
use serde::Deserialize;
use std::sync;

#[derive(Debug, Deserialize)]
pub struct ManualProgramRequest {
    /// program index (0 refers to the first program)
    #[serde(rename = "pid")]
    program_index: usize,
    /// use weather
    #[serde(rename = "uwt", deserialize_with = "legacy::de::bool_from_int")]
    use_water_scale: bool,
}

/// URI: `/mp`
pub async fn handler(open_sprinkler: web::Data<sync::Arc<sync::Mutex<Controller>>>, parameters: web::Query<ManualProgramRequest>) -> Result<impl Responder> {
    let mut open_sprinkler = open_sprinkler.lock().map_err(|_| error::InternalError::SyncError)?;

    // Validate program_index
    if parameters.program_index < open_sprinkler.config.programs.len() {
        return Ok(error::ReturnErrorCode::DataOutOfBound);
    }

    // Stop all stations and immediately start program
    open_sprinkler.reset_all_stations_immediate();
    open_sprinkler.manual_start_program(Some(parameters.program_index), program::ProgramStartType::User, parameters.use_water_scale);

    Ok(error::ReturnErrorCode::Success)
}
