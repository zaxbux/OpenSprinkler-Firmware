use core::fmt;

use actix_web::{Responder, HttpResponse, ResponseError, HttpRequest, body::BoxBody};
use serde_json::json;

use crate::opensprinkler::config;

#[repr(u8)]
pub enum ReturnErrorCode {
    /// `0`
    Ok = 0x00,
    /// `1`
    Success = 0x01,
    /// `2`
    Unauthorized = 0x02,
    /// `3`
    Mismatch = 0x03,
    /// `16`
    DataMissing = 0x10,
    /// `17`
    DataOutOfBound = 0x11,
    /// `18`
    DataFormatError = 0x12,
    /// `19`
    #[cfg(feature = "station-rf")]
    RFCodeError = 0x13,
    /// `32`
    PageNotFound = 0x20,
    /// `48`
    NotPermitted = 0x30,
}

impl Responder for ReturnErrorCode {
    type Body = BoxBody;

    fn respond_to(self, _: &HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(json!({
            "result": self as u8,
        }))
    }
}

/// Implement [actix_web::ResponseError] for config errors so that IO/serde errors are returned as HTTP 500 response.
impl ResponseError for config::result::Error {}

#[derive(Debug)]
pub enum InternalError {
    SyncError,
}

impl fmt::Display for InternalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SyncError => write!(f, "internal error"),
        }
    }
}

impl ResponseError for InternalError {}
