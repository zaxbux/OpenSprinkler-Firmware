use std::sync;

use actix_web::{web, Responder, Result};
use serde::Deserialize;

use crate::{opensprinkler::Controller, server::legacy::error};

#[derive(Debug, Deserialize)]
pub struct DeleteProgramRequest {
    pid: isize,
}

/// URI: `/dp`
pub async fn handler(open_sprinkler: web::Data<sync::Arc<sync::Mutex<Controller>>>, parameters: web::Query<DeleteProgramRequest>) -> Result<impl Responder> {
    let mut open_sprinkler = open_sprinkler.lock().map_err(|_| error::InternalError::SyncError)?;

    if parameters.pid == -1 {
        // Remove all programs
        open_sprinkler.config.programs.clear();
    } else if parameters.pid >= 0 {
        let index = parameters.pid as usize;

        if index < open_sprinkler.config.programs.len() {
            open_sprinkler.config.programs.remove(index);
        }
    } else {
        return Ok(error::ReturnErrorCode::DataOutOfBound);
    }

    open_sprinkler.config.write()?;

    Ok(error::ReturnErrorCode::Success)
}
