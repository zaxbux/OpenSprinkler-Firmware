use std::sync;

use actix_web::{body::BoxBody, dev::ServiceResponse, http::header::ContentType, web, HttpResponse};

use handlebars::Handlebars;
use serde_json::json;

use crate::opensprinkler::OpenSprinkler;

/// Generic error handler.
pub fn error<B>(res: &ServiceResponse<B>, error: &str) -> HttpResponse<BoxBody> {
    let request = res.request();

    // Provide a fallback to a simple plain text response in case an error occurs during the rendering of the error page.
    let fallback = |e: &str| HttpResponse::build(res.status()).content_type(ContentType::plaintext()).body(e.to_string());

    let hb = request.app_data::<web::Data<Handlebars>>().map(|t| t.get_ref());
    match hb {
        Some(hb) => {
            // @todo: use compiled firmware version (this code is just proof-of-concept)
            let firmware_version = if let Some(open_sprinkler) = request.app_data::<web::Data<sync::Arc<sync::Mutex<OpenSprinkler>>>>() {
                Some(open_sprinkler.lock().unwrap().config.firmware_version.to_string())
            } else {
                None
            };

            let data = json!({
                "error": error,
                "status_code": res.status().as_str(),
                "firmware_version": firmware_version
            });
            let body = hb.render("error", &data);

            match body {
                Ok(body) => HttpResponse::build(res.status()).content_type(ContentType::html()).body(body),
                Err(_) => fallback(error),
            }
        }
        None => fallback(error),
    }
}
