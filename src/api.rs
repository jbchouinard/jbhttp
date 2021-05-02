//! Base for APIs using HTTP methods.
use crate::handler::{FnHandler, Res};
use crate::request::{Method, Request};
use crate::response::Response;

fn not_implemented<O, E>() -> Res<O, E> {
    Err(Response::new(405))
}

/// Implement get, post, etc. to handle requests with the corresponding
/// HTTP verb. Unimplemented methods return 405.
pub trait Api<I, O, E, C> {
    fn get(&self, _request: Request<I>, _context: &mut C) -> Res<O, E> {
        not_implemented()
    }
    fn post(&self, _request: Request<I>, _context: &mut C) -> Res<O, E> {
        not_implemented()
    }
    fn put(&self, _request: Request<I>, _context: &mut C) -> Res<O, E> {
        not_implemented()
    }
    fn patch(&self, _request: Request<I>, _context: &mut C) -> Res<O, E> {
        not_implemented()
    }
    fn delete(&self, _request: Request<I>, _context: &mut C) -> Res<O, E> {
        not_implemented()
    }

    fn handler(self) -> FnHandler<I, O, E, C>
    where
        Self: 'static + Sized + Sync + Send,
    {
        FnHandler::new(Box::new(
            move |request: Request<I>, context: &mut C| match request.method {
                Method::GET => self.get(request, context),
                Method::POST => self.post(request, context),
                Method::PUT => self.put(request, context),
                Method::PATCH => self.patch(request, context),
                Method::DELETE => self.delete(request, context),
                _ => not_implemented(),
            },
        ))
    }
}
