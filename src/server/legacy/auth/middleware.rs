//! HTTP Authentication middleware.

use std::{
    future::Future,
    marker::PhantomData,
    pin::Pin,
    rc::Rc,
    sync::Arc,
    task::{Context, Poll},
};

use actix_web::{
    body::{EitherBody, MessageBody},
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    Error,
};
use futures_core::ready;
use futures_util::future::{self, FutureExt as _, LocalBoxFuture, TryFutureExt as _};

use super::extractors::{FromServiceRequest, DeviceKeyExtractor};

/// Middleware for checking HTTP authentication.
///
/// If there is no `Authorization` header in the request, this middleware returns an error
/// immediately, without calling the `F` callback.
///
/// Otherwise, it will pass both the request and the parsed credentials into it. In case of
/// successful validation `F` callback is required to return the `ServiceRequest` back.
#[derive(Debug, Clone)]
pub struct DeviceKeyMiddleware<T, F>
where
    T: FromServiceRequest,
{
    process_fn: Arc<F>,
    _extractor: PhantomData<T>,
}

impl<T, F, O> DeviceKeyMiddleware<T, F>
where
    T: FromServiceRequest,
    F: Fn(ServiceRequest, T) -> O,
    O: Future<Output = Result<ServiceRequest, Error>>,
{
    /// Construct `HttpAuthentication` middleware with the provided auth extractor `T` and
    /// validation callback `F`.
    pub fn with_fn(process_fn: F) -> DeviceKeyMiddleware<T, F> {
        DeviceKeyMiddleware {
            process_fn: Arc::new(process_fn),
            _extractor: PhantomData,
        }
    }
}

impl<F, O> DeviceKeyMiddleware<DeviceKeyExtractor, F>
where
    F: Fn(ServiceRequest, DeviceKeyExtractor) -> O,
    O: Future<Output = Result<ServiceRequest, Error>>,
{
    /// Construct `HttpAuthentication` middleware for the HTTP "Bearer" authentication scheme.
    pub fn device_key_md5(process_fn: F) -> Self {
        Self::with_fn(process_fn)
    }
}

impl<S, B, T, F, O> Transform<S, ServiceRequest> for DeviceKeyMiddleware<T, F>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    F: Fn(ServiceRequest, T) -> O + 'static,
    O: Future<Output = Result<ServiceRequest, Error>> + 'static,
    T: FromServiceRequest + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Transform = AuthenticationMiddleware<S, F, T>;
    type InitError = ();
    type Future = future::Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        future::ok(AuthenticationMiddleware {
            service: Rc::new(service),
            process_fn: self.process_fn.clone(),
            _extractor: PhantomData,
        })
    }
}

#[doc(hidden)]
pub struct AuthenticationMiddleware<S, F, T>
where
    T: FromServiceRequest,
{
    service: Rc<S>,
    process_fn: Arc<F>,
    _extractor: PhantomData<T>,
}

impl<S, B, F, T, O> Service<ServiceRequest> for AuthenticationMiddleware<S, F, T>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    F: Fn(ServiceRequest, T) -> O + 'static,
    O: Future<Output = Result<ServiceRequest, Error>> + 'static,
    T: FromServiceRequest + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = S::Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    actix_service::forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let process_fn = Arc::clone(&self.process_fn);

        let service = Rc::clone(&self.service);

        async move {
            let (req, credentials) = match Extract::<T>::new(req).await {
                Ok(req) => req,
                Err((err, req)) => {
                    return Ok(req.error_response(err).map_into_right_body());
                }
            };

            // TODO: alter to remove ? operator; an error response is required for downstream
            // middleware to do their thing (eg. cors adding headers)
            let req = process_fn(req, credentials).await?;

            service.call(req).await.map(|res| res.map_into_left_body())
        }
        .boxed_local()
    }
}

struct Extract<T> {
    req: Option<ServiceRequest>,
    f: Option<LocalBoxFuture<'static, Result<T, Error>>>,
    _extractor: PhantomData<fn() -> T>,
}

impl<T> Extract<T> {
    pub fn new(req: ServiceRequest) -> Self {
        Extract {
            req: Some(req),
            f: None,
            _extractor: PhantomData,
        }
    }
}

impl<T> Future for Extract<T>
where
    T: FromServiceRequest,
    T::Future: 'static,
    T::Error: 'static,
{
    type Output = Result<(ServiceRequest, T), (Error, ServiceRequest)>;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.f.is_none() {
            let req = self.req.as_ref().expect("Extract future was polled twice!");
            let f = T::from_service_request(req).map_err(Into::into);
            self.f = Some(f.boxed_local());
        }

        let f = self
            .f
            .as_mut()
            .expect("Extraction future should be initialized at this point");

        let credentials = ready!(f.as_mut().poll(ctx)).map_err(|err| {
            (
                err,
                // returning request allows a proper error response to be created
                self.req.take().expect("Extract future was polled twice!"),
            )
        })?;

        let req = self.req.take().expect("Extract future was polled twice!");
        Poll::Ready(Ok((req, credentials)))
    }
}