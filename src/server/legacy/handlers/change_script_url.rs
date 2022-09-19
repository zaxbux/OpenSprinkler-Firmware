use crate::{opensprinkler::OpenSprinkler, server::legacy::error};

use actix_web::{http, web, HttpResponse, Responder, Result};
use serde::Deserialize;
use std::sync;

#[derive(Debug, Deserialize)]
pub struct ChangeScriptUrlRequest {
    /// JavaScript URL
    #[serde(default, rename = "jsp")]
    pub jsp: Option<String>,
    /// Weather Service
    #[serde(default, rename = "wsp")]
    pub wsp: Option<String>,
}

/// URI: `/cu`
pub async fn handler(open_sprinkler: web::Data<sync::Arc<sync::Mutex<OpenSprinkler>>>, params: web::Query<ChangeScriptUrlRequest>) -> Result<impl Responder> {
    let mut open_sprinkler = open_sprinkler.lock().map_err(|_| error::InternalError::SyncError)?;

    if let Some(ref js_url) = params.jsp {
        open_sprinkler.config.js_url = js_url.clone();
    }

    if let Some(ref weather_service_url) = params.wsp {
        open_sprinkler.config.weather.service_url = weather_service_url.clone();
    }

    // Redirect to home
    Ok(HttpResponse::TemporaryRedirect().append_header((http::header::LOCATION, "/")).finish())
}
