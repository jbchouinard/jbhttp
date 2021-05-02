use crate::handler::{Handler, Res};
use crate::request::Request;
use crate::response::Response;

#[derive(Debug)]
pub struct AuthError {
    reason: String,
}

impl AuthError {
    pub fn new(reason: &str) -> Self {
        Self {
            reason: reason.to_string(),
        }
    }
}

pub struct Authenticator<F, H> {
    handler: H,
    fauth: F,
}

impl<F, H> Authenticator<F, H> {
    pub fn new(fauth: F, handler: H) -> Self {
        Self { handler, fauth }
    }
}

impl<F, H, I, O, E, C> Handler<I, O, E, C> for Authenticator<F, H>
where
    F: Fn(&Request<I>, &mut C) -> Result<(), AuthError> + 'static + Send + Sync,
    H: Handler<I, O, E, C>,
    O: 'static + Sync,
    I: 'static + Sync,
    E: 'static + Sync,
{
    fn handle(&self, request: Request<I>, context: &mut C) -> Res<O, E> {
        match (self.fauth)(&request, context) {
            Ok(()) => self.handler.handle(request, context),
            Err(_) => Err(Response::new(401)),
        }
    }
}
