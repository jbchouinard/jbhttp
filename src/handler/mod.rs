//! Base for all request handlers.
use crate::auth::{AuthError, Authenticator};
use crate::content::{
    MediaTypeDeserializer, MediaTypeErrorSerializer, MediaTypeSerde, MediaTypeSerializer,
};
use crate::filter::{ErrFilter, OkFilter, RequestFilter, ResFilter};
use crate::request::Request;
use crate::response::Response;

pub mod directory;

pub type Res<O, E> = std::result::Result<Response<O>, Response<E>>;
pub type RawResult = Res<Vec<u8>, Vec<u8>>;

/// An HTTP application is made of one or more handlers, composed
/// together.
pub trait Handler<I, O, E, C>: Sync + Send
where
    I: 'static + Sync,
    O: 'static + Sync,
    E: 'static + Sync,
{
    /// Handle an HTTP request.
    fn handle(&self, request: Request<I>, context: &mut C) -> Res<O, E>;

    fn authenticated<F>(self, f: F) -> Authenticator<F, Self>
    where
        F: Fn(&Request<I>, &mut C) -> Result<(), AuthError> + 'static + Send + Sync,
        Self: Sized,
    {
        Authenticator::new(f, self)
    }
    fn res_filter<F, FO, FE>(self, f: F) -> ResFilter<Self, F, O, E>
    where
        F: Fn(Res<O, E>, &mut C) -> Res<FO, FE> + Send + Sync,
        Self: Sized,
    {
        ResFilter::new(f, self)
    }
    fn request_filter<F, FI>(self, f: F) -> RequestFilter<Self, F, I>
    where
        F: Fn(Request<I>, &mut C) -> Request<FI> + Send + Sync,
        Self: Sized,
    {
        RequestFilter::new(f, self)
    }
    fn response_filter<F, FO>(self, f: F) -> OkFilter<Self, F, O>
    where
        F: Fn(Response<O>, &mut C) -> Response<FO> + Send + Sync,
        Self: Sized,
    {
        OkFilter::new(f, self)
    }

    fn error_filter<F, FE>(self, f: F) -> ErrFilter<Self, F, E>
    where
        F: Fn(Response<E>, &mut C) -> Response<FE> + Send + Sync,
        Self: Sized,
    {
        ErrFilter::new(f, self)
    }
    fn serialized(self) -> MediaTypeSerializer<Self, I, O>
    where
        Self: Sized,
    {
        MediaTypeSerializer::new(self)
    }
    fn deserialized(self) -> MediaTypeDeserializer<Self, I, O>
    where
        Self: Sized,
    {
        MediaTypeDeserializer::new(self)
    }
    fn serdeserialized(self) -> MediaTypeSerde<Self, I, O>
    where
        Self: Sized,
    {
        MediaTypeSerde::new(self)
    }
    fn serialized_error(self) -> MediaTypeErrorSerializer<Self, I, E>
    where
        Self: Sized,
    {
        MediaTypeErrorSerializer::new(self)
    }
}

pub type HandlerFunc<I, O, E, C> = Box<dyn Fn(Request<I>, &mut C) -> Res<O, E> + Send + Sync>;

pub struct FnHandler<I, O, E, C> {
    f: HandlerFunc<I, O, E, C>,
}

impl<I, O, E, C> FnHandler<I, O, E, C> {
    pub fn new(f: HandlerFunc<I, O, E, C>) -> Self {
        Self { f }
    }
}

impl<I, O, E, C> Handler<I, O, E, C> for FnHandler<I, O, E, C>
where
    I: 'static + Sync,
    O: 'static + Sync,
    E: 'static + Sync,
{
    fn handle(&self, request: Request<I>, context: &mut C) -> Res<O, E> {
        (self.f)(request, context)
    }
}

impl<F, I, O, E, C> Handler<I, O, E, C> for F
where
    F: Fn(Request<I>, &mut C) -> Res<O, E> + Send + Sync,
    O: Sync + 'static,
    I: Sync + 'static,
    E: Sync + 'static,
{
    fn handle(&self, request: Request<I>, context: &mut C) -> Res<O, E> {
        (self)(request, context)
    }
}
