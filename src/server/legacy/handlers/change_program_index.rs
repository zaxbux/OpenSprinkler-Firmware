use crate::{opensprinkler::OpenSprinkler, server::legacy::error};

use actix_web::{web, Responder, Result};
use serde::Deserialize;
use std::sync;

#[derive(Debug, Deserialize)]
pub struct ProgramMoveUpRequest {
    /// program index (0 refers to the first program)
    #[serde(rename = "pid")]
    program_index: usize,
}

/// URI: `/up`
pub async fn handler(open_sprinkler: web::Data<sync::Arc<sync::Mutex<OpenSprinkler>>>, parameters: web::Query<ProgramMoveUpRequest>) -> Result<impl Responder> {
    let mut open_sprinkler = open_sprinkler.lock().map_err(|_| error::InternalError::SyncError)?;

    // Validate program_index
    if parameters.program_index < 1 || parameters.program_index >= open_sprinkler.config.programs.len() {
        return Ok(error::ReturnErrorCode::DataOutOfBound);
    }

    open_sprinkler.config.programs.swap(parameters.program_index, parameters.program_index - 1);

    Ok(error::ReturnErrorCode::Success)
}
