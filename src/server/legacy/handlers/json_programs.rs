use std::sync::{Arc, Mutex};

use actix_web::{web, Responder, Result};

use crate::{
    opensprinkler::Controller,
    server::legacy::{error, payload},
};

/// Get Programs
///
/// URI: `/jp`
pub async fn handler(open_sprinkler: web::Data<Arc<Mutex<Controller>>>) -> Result<impl Responder> {
    let open_sprinkler = open_sprinkler.lock().map_err(|_| error::InternalError::SyncError)?;

    Ok(web::Json(payload::Programs::new(&open_sprinkler)))
}
