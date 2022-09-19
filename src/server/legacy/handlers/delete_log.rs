use std::sync;

use actix_web::{web, Responder, Result};

use serde::{de, Deserialize, Deserializer};

use crate::{opensprinkler::OpenSprinkler, server::legacy::error};

enum DeleteLogDay {
    All,
    Day(u64),
}

impl<'de> de::Deserialize<'de> for DeleteLogDay {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match String::deserialize(deserializer)?.as_str() {
            "all" => Ok(Self::All),
            day => day.parse().map(|day| Self::Day(day)).map_err(|_| serde::de::Error::invalid_value(de::Unexpected::Str(day), &"u64")),
        }
    }
}

#[derive(Deserialize)]
pub struct DeleteLogsRequest {
    day: DeleteLogDay,
}

pub async fn handler(open_sprinkler: web::Data<sync::Arc<sync::Mutex<OpenSprinkler>>>, parameters: web::Query<DeleteLogsRequest>) -> Result<impl Responder> {
    let open_sprinkler = open_sprinkler.lock().map_err(|_| error::InternalError::SyncError)?;

    // delete log
    todo!();

    Ok(error::ReturnErrorCode::Success)
}
