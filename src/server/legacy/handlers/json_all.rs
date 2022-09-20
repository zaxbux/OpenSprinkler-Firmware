use std::sync::{Arc, Mutex};

use crate::{
    opensprinkler::Controller,
    server::legacy::{error, payload},
};
use actix_web::{web, Responder, Result};

/// Get All JSON Data
///
/// URI: `/ja`
pub async fn handler(open_sprinkler: web::Data<Arc<Mutex<Controller>>>) -> Result<impl Responder> {
    let open_sprinkler = open_sprinkler.lock().map_err(|_| error::InternalError::SyncError)?;

    Ok(web::Json(payload::All::new(&open_sprinkler)))
}
