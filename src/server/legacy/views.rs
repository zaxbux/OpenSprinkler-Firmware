use actix_web::{http::header::ContentType, web, HttpResponse, Responder, Result};
use handlebars::Handlebars;
use reqwest::StatusCode;
use serde_json::json;
use std::sync::{Arc, Mutex};

use crate::opensprinkler::Controller;

pub async fn index(hb: web::Data<Handlebars<'_>>, open_sprinkler: web::Data<Arc<Mutex<Controller>>>) -> Result<impl Responder> {
    let open_sprinkler = open_sprinkler.lock().unwrap();

    let data = json!({
        "firmware_version": open_sprinkler.config.firmware_version,
        "javascript_url": open_sprinkler.config.js_url,
    });

    let body = hb.render("legacy/index", &data).unwrap();

    Ok(HttpResponse::build(StatusCode::OK).content_type(ContentType::html()).body(body))
}

pub async fn script_url(hb: web::Data<Handlebars<'_>>, open_sprinkler: web::Data<Arc<Mutex<Controller>>>) -> Result<impl Responder> {
    let open_sprinkler = open_sprinkler.lock().unwrap();

    let defaults = crate::config::Config::default();

    let data = json!({
        "javascript_url": open_sprinkler.config.js_url,
        "default_javascript_url": defaults.js_url,
        "weather_service": open_sprinkler.config.firmware_version,
        "default_weather_service": defaults.weather.service_url,
    });

    let body = hb.render("legacy/script_url", &data).unwrap();

    Ok(HttpResponse::build(StatusCode::OK).content_type(ContentType::html()).body(body))
}
