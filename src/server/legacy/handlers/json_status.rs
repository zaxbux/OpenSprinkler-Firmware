use std::sync;

use actix_web::{web, Responder, Result};

use crate::{
    opensprinkler::OpenSprinkler,
    server::legacy::{error, payload},
};

/// Get Status
///
/// URI: `/js`
pub async fn handler(open_sprinkler: web::Data<sync::Arc<sync::Mutex<OpenSprinkler>>>) -> Result<impl Responder> {
    let open_sprinkler = open_sprinkler.lock().map_err(|_| error::InternalError::SyncError)?;

    Ok(web::Json(payload::Status::new(&open_sprinkler)))
}
