use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use actix_web::{web, Responder, Result};
use serde::Serialize;

use crate::{
    opensprinkler::{station, Controller},
    server::legacy::{error, IntoLegacyFormat},
};

#[derive(Serialize)]
struct LegacyFormat {
    /// Special station type
    #[serde(rename = "st")]
    station_type: u8,

    /// Sepcial station config
    #[serde(rename = "sd")]
    data: String,
}

struct Error;

/* impl TryFrom<&station::Station> for LegacyFormat {
    type Error = Error;

    fn try_from(station: &station::Station) -> Result<Self, Self::Error> {
        if let Some(Some(data)) = station.sped.map(|data| data.into_legacy_format()) {
            return Ok(Self {
                station_type: station.station_type.into(),
                data,
            });
        }

        Err(Error)
    }
} */

impl TryInto<LegacyFormat> for station::Station {
    type Error = Error;

    fn try_into(self) -> Result<LegacyFormat, Self::Error> {
        if let Some(Some(data)) = self.sped.map(|data| data.into_legacy_format()) {
            return Ok(LegacyFormat {
                station_type: self.station_type.into(),
                data,
            });
        }

        Err(Error)
    }
}

/// Get Special Station Data
///
/// URI: `/je`
pub async fn handler(open_sprinkler: web::Data<Arc<Mutex<Controller>>>) -> Result<impl Responder> {
    let open_sprinkler = open_sprinkler.lock().map_err(|_| error::InternalError::SyncError)?;

    // Format: JSON object where keys are the station index
    let mut special: HashMap<String, LegacyFormat> = HashMap::new();

    for i in 0..open_sprinkler.config.get_station_count() {
        if let Some(station) = open_sprinkler.config.stations.get(i) {
            if station.station_type != station::StationType::Standard {
                if let Ok(payload) = station.to_owned().try_into() {
                    special.insert(i.to_string(), payload);
                }
            }
        }
    }

    Ok(web::Json(special))
}
