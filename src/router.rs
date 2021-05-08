//! Path based request routing.
use crate::handler::{Handler, Res};
use crate::request::{Param, Request};
use crate::response::Response;

enum RoutePart {
    Exact(String),
    Param(String),
    Any,
}

impl RoutePart {
    fn from_str(s: &str) -> Self {
        if s == "*" {
            Self::Any
        } else if let Some(s) = s.strip_prefix('?') {
            Self::Param(s.to_string())
        } else {
            Self::Exact(s.to_string())
        }
    }
    fn matches(&self, s: &str) -> (bool, Option<(String, String)>) {
        match self {
            Self::Exact(p) => (s == &p[..], None),
            Self::Any => (true, None),
            Self::Param(p) => (true, Some((p.clone(), s.to_string()))),
        }
    }
}

struct RoutePath {
    parts: Vec<RoutePart>,
    is_prefix: bool,
}

impl RoutePath {
    fn from_str(s: &str) -> Self {
        let mut route_parts = vec![];
        let mut is_prefix = false;
        let parts = match s.ends_with("**") {
            true => {
                is_prefix = true;
                s[..s.len() - 2].split('/')
            }
            false => s.split('/'),
        };
        for part in parts {
            route_parts.push(RoutePart::from_str(part));
        }
        Self {
            parts: route_parts,
            is_prefix,
        }
    }
    fn matches(&self, s: &str) -> (bool, Vec<(String, String)>) {
        let parts: Vec<&str> = s.split('/').collect();
        let mut params = vec![];

        if parts.len() < self.parts.len() {
            return (false, params);
        }

        if parts.len() > self.parts.len() && !self.is_prefix {
            return (false, params);
        }

        for (i, part) in parts.iter().enumerate() {
            let (matches, param) = self.parts[i].matches(part);
            if !matches {
                return (false, params);
            }
            if let Some((name, val)) = param {
                params.push((name, val));
            }
        }
        (true, params)
    }
}

struct Route<I, O, E, C> {
    path: RoutePath,
    // Use boxdyn cause I can't have a type parameter H for handler, because
    // Router must hold Vec<Route> for heterogenous H.
    handler: Box<dyn Handler<I, O, E, C>>,
}

/// Router is a Handler which dispatches requests to any number of other
/// Handlers based on the request path and method.
///
/// # Usage - route patterns
/// * `/foo`: matches exactly /foo
/// * `/foo/*/bar`: matches /foo/anything/bar
/// * `/foo/**`: matches /foo/bar/... (only at end of route)
/// * `/foo/?name`: matches /foo/bar, add name="bar" to request.params
///
/// # Example
/// ```
/// use jbhttp::prelude::*;
/// use jbhttp::router::Router;
///
/// fn handle_hello(req: RawRequest, _context: &mut ()) -> Res<Vec<u8>, Vec<u8>> {
///     Ok(Response::new(200).with_payload(b"Hello!".to_vec()))
/// }
///
/// fn handle_bye(req: RawRequest, _context: &mut ()) -> Res<Vec<u8>, Vec<u8>> {
///     Ok(Response::new(200).with_payload(b"Bye!".to_vec()))
/// }
///
/// let router = Router::new()
///    .with_route("/hello", handle_hello)
///    .with_route("/bye", handle_bye);
///
/// let mut req_hello = Request::default();
/// req_hello.path = "/hello".to_string();
/// let response_hello = router.handle(req_hello, &mut ()).unwrap();
/// # assert_eq!(response_hello.payload, Some(b"Hello!".to_vec()));
///
/// let mut req_bye = Request::default();
/// req_bye.path = "/bye".to_string();
/// let response_bye = router.handle(req_bye, &mut ()).unwrap();
/// # assert_eq!(response_bye.payload, Some(b"Bye!".to_vec()));
/// ```
pub struct Router<I, O, E, C> {
    routes: Vec<Route<I, O, E, C>>,
}

impl<I: 'static + Sync, O: 'static + Sync, E: 'static + Sync, C> Router<I, O, E, C> {
    pub fn new() -> Self {
        Self { routes: vec![] }
    }
    pub fn with_route<H>(mut self, path: &str, handler: H) -> Self
    where
        H: 'static + Handler<I, O, E, C>,
    {
        self.routes.push(Route {
            path: RoutePath::from_str(path),
            handler: Box::new(handler),
        });
        self
    }
}

impl<I: 'static + Sync, O: 'static + Sync, E: 'static + Sync, C> Default for Router<I, O, E, C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<I: 'static + Sync, O: 'static + Sync, E: 'static + Sync, C> Handler<I, O, E, C>
    for Router<I, O, E, C>
{
    fn handle(&self, mut request: Request<I>, context: &mut C) -> Res<O, E> {
        for route in &self.routes {
            let (matches, params) = route.path.matches(&request.path);
            if matches {
                for (name, val) in params {
                    request.params.add(Param::Path(name), val)
                }
                return route.handler.handle(request, context);
            }
        }
        Err(Response::new(404))
    }
}
