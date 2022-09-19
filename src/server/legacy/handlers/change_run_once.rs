use crate::{
    opensprinkler::{program, OpenSprinkler},
    server::legacy::{self, error},
    utils,
};

use actix_web::{web, Responder, Result};
use serde::Deserialize;
use std::sync;

#[derive(Debug, Deserialize)]
pub struct ChangeRunOnceRequest {
    #[serde(rename = "t", deserialize_with = "legacy::de::int_array_from_string")]
    pub times: Vec<u16>,
}

/// URI: `/cr`
pub async fn handler(open_sprinkler: web::Data<sync::Arc<sync::Mutex<OpenSprinkler>>>, parameters: web::Query<ChangeRunOnceRequest>) -> Result<impl Responder> {
    let mut open_sprinkler = open_sprinkler.lock().map_err(|_| error::InternalError::SyncError)?;

    // reset all stations and prepare to run one-time program
    open_sprinkler.reset_all_stations_immediate();

    let mut match_found = false;
    let sunrise_time = open_sprinkler.config.sunrise_time;
    let sunset_time = open_sprinkler.config.sunset_time;

    for station_index in 0..open_sprinkler.get_station_count() {
        let water_time = parameters.times[station_index];

        if water_time > 0 && !open_sprinkler.config.stations[station_index].attrib.is_disabled {
            let water_time = utils::water_time_resolve(water_time, sunrise_time, sunset_time);
            let value = program::QueueElement::new(0, water_time as i64, station_index, None, program::ProgramStartType::RunOnce);
            if let Ok(_) = open_sprinkler.state.program.queue.enqueue(value) {
                match_found = true;
            }
        }
    }

    if !match_found {
        return Ok(error::ReturnErrorCode::DataMissing);
    }

    open_sprinkler.schedule_all_stations(chrono::Utc::now().timestamp());

    Ok(error::ReturnErrorCode::Success)
}
