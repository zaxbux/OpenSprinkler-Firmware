use std::error::Error;
use std::fmt;

use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError};
use serde_json::json;

use super::super::error::ReturnErrorCode;

/// Authentication error returned by authentication extractors.
#[derive(Debug)]
pub struct AuthenticationError {
    status_code: StatusCode,
    response: Option<HttpResponse>,
}

impl AuthenticationError {
    /// Creates new authentication error.
    pub fn new() -> AuthenticationError {
        AuthenticationError { status_code: StatusCode::OK, response: None }
    }

    /// Returns mutable reference to the inner status code.
    pub fn status_code_mut(&mut self) -> &mut StatusCode {
        &mut self.status_code
    }
}

impl fmt::Display for AuthenticationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.status_code, f)
    }
}

impl Error for AuthenticationError {}

impl ResponseError for AuthenticationError {
    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code).json(json!({
            "result": ReturnErrorCode::Unauthorized as u8,
        }))
    }

    fn status_code(&self) -> StatusCode {
        self.status_code
    }
}
