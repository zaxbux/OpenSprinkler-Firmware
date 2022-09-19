use std::sync::{Arc, Mutex};

use actix_web::{web, Responder, Result};


use crate::{
    opensprinkler::OpenSprinkler,
    server::legacy::{error, payload},
};



/// Get Option
/// 
/// URI: `/jo`
pub async fn handler(open_sprinkler: web::Data<Arc<Mutex<OpenSprinkler>>>) -> Result<impl Responder> {
    let open_sprinkler = open_sprinkler.lock().map_err(|_| error::InternalError::SyncError)?;

    Ok(web::Json(payload::Options::new(&open_sprinkler)))
}
