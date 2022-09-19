use std::future::Future;

use actix_web::dev::ServiceRequest;
use actix_web::Error;
use serde::Deserialize;


use actix_utils::future::{ready, Ready};
use actix_web::{dev::Payload, web::Query, FromRequest, HttpRequest};

use super::errors::AuthenticationError;

use std::{borrow::Cow, fmt};

/// Trait implemented by types that can extract
/// HTTP authentication scheme credentials from the request.
///
/// It is very similar to actix' `FromRequest` trait,
/// except it operates with a `ServiceRequest` struct instead,
/// therefore it can be used in the middlewares.
pub trait FromServiceRequest: Sized {
    /// The associated error which can be returned.
    type Error: Into<Error>;

    /// Future that resolves into extracted credentials type.
    type Future: Future<Output = Result<Self, Self::Error>>;

    /// Parse the authentication credentials from the actix' `ServiceRequest`.
    fn from_service_request(req: &ServiceRequest) -> Self::Future;
}

#[derive(Deserialize)]
pub struct LegacyRequest {
    /// The device key in hashed MD5 format.
    pub pw: String,
}


/// Credentials
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct Credentials {
    md5: Cow<'static, str>,
}

impl Credentials {
    /// Creates new `Bearer` credentials with the token provided.
    pub fn new<T>(md5: T) -> Self
    where
        T: Into<Cow<'static, str>>,
    {
        Self { md5: md5.into() }
    }

    /// Gets reference to the credentials token.
    pub fn device_key(&self) -> &str {
        self.md5.as_ref()
    }
}

impl From<Query<LegacyRequest>> for Credentials {
    fn from(query: Query<LegacyRequest>) -> Self {
        Self::new(query.pw.clone().to_lowercase())
    }
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("Credentials MD5(******)"))
    }
}

impl fmt::Display for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("Credentials MD5({})", self.md5))
    }
}

/// Extractor for HTTP Bearer auth
#[derive(Debug, Clone)]
pub struct DeviceKeyExtractor(Credentials);

impl DeviceKeyExtractor {
    /// Returns bearer token provided by client.
    pub fn device_key(&self) -> &str {
        self.0.device_key()
    }
}

impl FromRequest for DeviceKeyExtractor {
    type Future = Ready<Result<Self, Self::Error>>;
    type Error = AuthenticationError;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> <Self as FromRequest>::Future {
        ready(Query::<LegacyRequest>::from_query(req.query_string()).map(|query| DeviceKeyExtractor(query.into())).map_err(|_| AuthenticationError::new()))
    }
}

impl FromServiceRequest for DeviceKeyExtractor {
    type Future = Ready<Result<Self, Self::Error>>;
    type Error = AuthenticationError;

    fn from_service_request(req: &ServiceRequest) -> Self::Future {
        ready(Query::<LegacyRequest>::from_query(req.query_string()).map(|query| DeviceKeyExtractor(query.into())).map_err(|_| AuthenticationError::new()))
    }
}
