use std::sync::{Arc, Mutex};

use actix_web::{web, Responder, Result};
use serde::Deserialize;

use crate::{opensprinkler::OpenSprinkler, server::legacy::error};

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    /// New device key
    #[serde(rename = "npw")]
    pub new: Option<String>,
    /// Confirm new device key
    #[serde(rename = "cpw")]
    pub confirm: Option<String>,
}

/// Change Password
///
/// URI: `/sp`
pub async fn handler(_open_sprinkler: web::Data<Arc<Mutex<OpenSprinkler>>>, _params: web::Query<ChangePasswordRequest>) -> Result<impl Responder> {
    // Prohibit password changes for Demo
    #[cfg(not(feature = "demo"))]
    {
        if let Some(npw) = &_params.new {
            if let Some(cpw) = &_params.confirm {
                if npw == cpw {
                    let mut open_sprinkler = _open_sprinkler.lock().map_err(|_| error::InternalError::SyncError)?;

                    open_sprinkler.config.device_key = npw.clone();
                    return Ok(error::ReturnErrorCode::Success);
                } else {
                    return Ok(error::ReturnErrorCode::Mismatch);
                }
            }
        }
    }

    Ok(error::ReturnErrorCode::DataMissing)
}
