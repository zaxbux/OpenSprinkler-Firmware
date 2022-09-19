use std::sync::{Arc, Mutex};

use actix_web::{web, Responder, Result};

use crate::{
    opensprinkler::OpenSprinkler,
    server::legacy::{error, payload},
};

/// URI: `/jn`
pub async fn handler(open_sprinkler: web::Data<Arc<Mutex<OpenSprinkler>>>) -> Result<impl Responder> {
    let open_sprinkler = open_sprinkler.lock().map_err(|_| error::InternalError::SyncError)?;

    Ok(web::Json(payload::Stations::new(&open_sprinkler)))
}
