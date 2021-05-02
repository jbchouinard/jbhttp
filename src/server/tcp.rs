//! TCP HTTP server.
use std::io::prelude::*;
use std::net::TcpListener;
use std::sync::Arc;
use std::time::{Duration, Instant};

use log::*;

use crate::{
    handler::Handler,
    request::parser::RequestParser,
    response::Response,
    runner::Runner,
    server::{Server, ServerError},
    VERSION,
};

/// A single or multi-threaded TCP server.
pub struct TcpServer<H> {
    listener: TcpListener,
    runner: Runner,
    handler: Arc<H>,
    timeout: Option<Duration>,
}

impl<H> TcpServer<H> {
    /// Create a new TCP server
    ///
    /// # Arguments
    /// * `bind_addr`: Address to listen on, such as "0.0.0.0:8080"
    /// * `n_threads`: Number of threads.
    ///   - 0: create a new thread for each request (not recommended)
    ///   - 1: single-threaded
    ///   - 2+: threadpool with n threads
    /// * `timeout`: network socket timeout
    /// * `handler`: request handler
    pub fn new(
        bind_addr: &str,
        n_threads: usize,
        timeout: Option<Duration>,
        handler: H,
    ) -> Result<Self, std::io::Error> {
        Ok(Self {
            listener: TcpListener::bind(bind_addr)?,
            runner: Runner::new(n_threads),
            timeout,
            handler: Arc::new(handler),
        })
    }
}

impl<H, C> Server<C> for TcpServer<H>
where
    C: std::fmt::Debug + Default,
    H: 'static + Handler<Vec<u8>, Vec<u8>, Vec<u8>, C>,
{
    /// Serve one request.
    fn serve_one(&mut self) -> Result<(), ServerError> {
        // TODO: keep-alive
        let (mut stream, addr) = self.listener.accept()?;
        debug!("accepted connection from {:?}", addr);
        stream.set_read_timeout(self.timeout).unwrap();
        stream.set_write_timeout(self.timeout).unwrap();
        let handler = self.handler.clone();
        self.runner.run(move || {
            let start = Instant::now();
            let mut context = C::default();
            trace!("CONTEXT {:?}", &context);
            debug!("parsing request");
            let mut parser = RequestParser::new(&mut stream);
            let response;
            let path;
            let method;
            let content_length;
            match parser.parse() {
                Ok(request) => {
                    debug!("done parsing request");
                    trace!("REQUEST {:?}", &request);
                    content_length = request.content_length;
                    path = request.path.clone();
                    method = format!("{:?}", request.method);
                    debug!("running request handler");
                    response = handler.handle(request, &mut context);
                }
                Err(e) => {
                    error!("{}", e);
                    response = Err(Response::new(400));
                    path = "<none>".to_string();
                    method = "<none>".to_string();
                    content_length = 0;
                }
            };
            let (variant, response) = match response {
                Ok(response) => ("Ok".to_string(), response),
                Err(response) => ("Err".to_string(), response),
            };
            let response = response
                .with_header("Server", &format!("jbhttp::TcpServer/{}", VERSION))
                .with_header("Connection", "closed");
            trace!("CONTEXT: {:?}", &context);
            trace!("RESPONSE: {:?}", &response);
            info!(
                "{:?} - {}ms - {} {} {} ({} bytes) -> {} {} {} ({} bytes)",
                std::thread::current().id(),
                start.elapsed().as_millis(),
                addr,
                method,
                path,
                content_length,
                variant,
                response.status_code,
                &response.status,
                response.content_length(),
            );
            debug!("writing response");
            match stream.write_all(&response.into_bytes()) {
                Ok(_) => (),
                Err(e) => error!("IO error: {}", e),
            }
        });
        Ok(())
    }
}
