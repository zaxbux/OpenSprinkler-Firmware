use actix_web::{
    body::BoxBody,
    dev::ServiceResponse,
    http::{Method, StatusCode},
    middleware::{ErrorHandlerResponse, ErrorHandlers},
    Either, HttpResponse, Responder, Result,
};

use super::views;

/// Default request handler
pub async fn default(req_method: Method) -> Result<impl Responder> {
    match req_method {
        Method::GET => Ok(Either::Left(HttpResponse::NotFound().finish())),
        _ => Ok(Either::Right(HttpResponse::MethodNotAllowed().finish())),
    }
}

/// Custom error handler, to return HTML responses when an error occurs.
pub fn errors() -> ErrorHandlers<BoxBody> {
    ErrorHandlers::new().handler(StatusCode::NOT_FOUND, not_found)
}

/// Error handler for a 404 Page not found error.
fn not_found<B>(res: ServiceResponse<B>) -> Result<ErrorHandlerResponse<BoxBody>> {
    let response = views::error(&res, StatusCode::NOT_FOUND.canonical_reason().unwrap());
    Ok(ErrorHandlerResponse::Response(ServiceResponse::new(res.into_parts().0, response.map_into_left_body())))
}
