use crate::handler::{Handler, Res};
use crate::request::Request;
use crate::response::Response;
use std::marker::PhantomData;

pub struct RequestFilter<H, F, I: 'static> {
    f: F,
    handler: H,
    phantom_i: PhantomData<&'static I>,
}

impl<H, F, I> RequestFilter<H, F, I> {
    pub fn new(f: F, handler: H) -> Self {
        Self {
            f,
            handler,
            phantom_i: PhantomData,
        }
    }
}

/// Return Ok(Request) to continue processing, or Err(Response) to abort
impl<F, FI, H, I, O, E, C> Handler<I, O, E, C> for RequestFilter<H, F, I>
where
    H: Handler<FI, O, E, C>,
    F: Fn(Request<I>, &mut C) -> Result<Request<FI>, Response<E>> + Send + Sync,
    I: 'static + Sync,
    FI: 'static + Sync,
    O: 'static + Sync,
    E: 'static + Sync,
{
    fn handle(&self, request: Request<I>, context: &mut C) -> Res<O, E> {
        match (self.f)(request, context) {
            Ok(request) => self.handler.handle(request, context),
            Err(response) => Err(response),
        }
    }
}

pub struct ResponseFilter<H, F, O: 'static> {
    f: F,
    handler: H,
    phantom_o: PhantomData<&'static O>,
}

impl<H, F, O> ResponseFilter<H, F, O> {
    pub fn new(f: F, handler: H) -> Self {
        Self {
            f,
            handler,
            phantom_o: PhantomData,
        }
    }
}

impl<F, FO, H, I, O, E, C> Handler<I, FO, E, C> for ResponseFilter<H, F, O>
where
    H: Handler<I, O, E, C>,
    O: 'static + Sync,
    F: Fn(Response<O>, &mut C) -> Response<FO> + Send + Sync,
    I: 'static + Sync,
    FO: 'static + Sync,
    E: 'static + Sync,
{
    fn handle(&self, request: Request<I>, context: &mut C) -> Res<FO, E> {
        match self.handler.handle(request, context) {
            Ok(r) => Ok((self.f)(r, context)),
            Err(r) => Err(r),
        }
    }
}

pub struct ErrorFilter<H, F, E: 'static> {
    f: F,
    handler: H,
    phantom_e: PhantomData<&'static E>,
}

impl<H, F, E> ErrorFilter<H, F, E> {
    pub fn new(f: F, handler: H) -> Self {
        Self {
            f,
            handler,
            phantom_e: PhantomData,
        }
    }
}

impl<F, FE, H, I, O, E, C> Handler<I, O, E, C> for ErrorFilter<H, F, FE>
where
    H: Handler<I, O, FE, C>,
    F: Fn(Response<FE>, &mut C) -> Response<E> + Send + Sync,
    FE: 'static + Sync,
    O: 'static + Sync,
    E: 'static + Sync,
    I: 'static + Sync,
{
    fn handle(&self, request: Request<I>, context: &mut C) -> Res<O, E> {
        match self.handler.handle(request, context) {
            Err(r) => Err((self.f)(r, context)),
            Ok(r) => Ok(r),
        }
    }
}

pub struct ResFilter<H, F, O: 'static, E: 'static> {
    f: F,
    handler: H,
    phantom_o: PhantomData<&'static O>,
    phantom_e: PhantomData<&'static E>,
}

impl<H, F, O, E> ResFilter<H, F, O, E> {
    pub fn new(f: F, handler: H) -> Self {
        Self {
            f,
            handler,
            phantom_o: PhantomData,
            phantom_e: PhantomData,
        }
    }
}

impl<F, FO, FE, H, I, O, E, C> Handler<I, FO, FE, C> for ResFilter<H, F, O, E>
where
    H: Handler<I, O, E, C>,
    F: Fn(Res<O, E>, &mut C) -> Res<FO, FE> + Send + Sync,
    O: 'static + Sync,
    E: 'static + Sync,
    FO: 'static + Sync,
    FE: 'static + Sync,
    I: 'static + Sync,
{
    fn handle(&self, request: Request<I>, context: &mut C) -> Res<FO, FE> {
        (self.f)(self.handler.handle(request, context), context)
    }
}
