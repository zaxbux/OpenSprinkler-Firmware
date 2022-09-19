use actix_web::{web, Responder, Result};
use serde::Deserialize;
use std::sync;

use crate::{
    opensprinkler::{program, station, OpenSprinkler},
    server::legacy::{self, error},
};

#[derive(Debug, Deserialize)]
pub struct ChangeManualRequest {
    /// station index (starting from 0)
    #[serde(rename = "sid")]
    station_index: usize,
    ///  enable (0 or 1)
    #[serde(rename = "en", deserialize_with = "legacy::de::bool_from_int")]
    enable: bool,
    ///   timer (required if en=1)
    #[serde(rename = "t")]
    timer: Option<u16>,
}

/// Test station.
///
/// URI: `/cm`
pub async fn handler(open_sprinkler: web::Data<sync::Arc<sync::Mutex<OpenSprinkler>>>, parameters: web::Query<ChangeManualRequest>) -> Result<impl Responder> {
    let mut open_sprinkler = open_sprinkler.lock().map_err(|_| error::InternalError::SyncError)?;

    // validate station_index
    if parameters.station_index >= station::MAX_NUM_STATIONS {
        return Ok(error::ReturnErrorCode::DataOutOfBound);
    }

    if parameters.enable {
        if let Some(timer) = parameters.timer {
            // validate timer value
            if timer == 0 || timer > station::MAX_WATER_TIME {
                return Ok(error::ReturnErrorCode::DataOutOfBound);
            }

            // schedule manual station
            for master in open_sprinkler.config.master_stations {
                if Some(parameters.station_index) == master.station {
                    // master stations cannot be scheduled separately from non-master stations
                    return Ok(error::ReturnErrorCode::NotPermitted);
                }
            }

            let q = program::QueueElement::new(0, timer.into(), parameters.station_index, None, program::ProgramStartType::Test);

            if let Some(sqi) = open_sprinkler.state.program.queue.station_qid[parameters.station_index] {
                // overwrite existing schedule
                open_sprinkler.state.program.queue.queue[sqi] = q;
            } else {
                // create new queue element, or return error response if queue is full
                if let Err(_) = open_sprinkler.state.program.queue.enqueue(q) {
                    return Ok(error::ReturnErrorCode::NotPermitted);
                }
            };

            open_sprinkler.schedule_all_stations(chrono::Utc::now().timestamp());
        } else {
            return Ok(error::ReturnErrorCode::DataMissing);
        }
    } else {
        open_sprinkler.turn_off_station(chrono::Utc::now().timestamp(), parameters.station_index);
    }

    Ok(error::ReturnErrorCode::Success)
}
