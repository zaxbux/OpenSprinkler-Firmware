use std::sync;

use actix_web::{dev::ServiceRequest, web, Error};

use crate::opensprinkler::Controller;

use self::{errors::AuthenticationError, extractors::DeviceKeyExtractor};

pub mod errors;
pub mod extractors;
pub mod middleware;

pub async fn validator(req: ServiceRequest, credentials: DeviceKeyExtractor) -> Result<ServiceRequest, Error> {
    let device_key = if let Some(open_sprinkler) = req.app_data::<web::Data<sync::Arc<sync::Mutex<Controller>>>>() {
        let open_sprinkler = open_sprinkler.lock().unwrap();
        Some(open_sprinkler.config.device_key.clone())
    } else {
        None
    };

    if let Some(device_key) = device_key {
        if device_key == credentials.device_key() {
            return Ok(req);
        }

        // Original firmware would return this JSON: `{"fwv":219}` when the specified device key was incorrect for the `/ja` and `/ja` routes only.
        // It is unknown how that feature was used, so it has been left out.
    }

    Err(AuthenticationError::new().into())
}
