//! Generic IO Stream HTTP server.
use std::io::prelude::*;

use crate::{
    handler::Handler,
    request::parser::RequestParser,
    response::Response,
    server::{Server, ServerError},
    VERSION,
};

/// Serve HTTP requests over a generic stream. The stream is not closed,
/// multiple requests can be served.
///
/// # Example
/// ```
/// use jbhttp::prelude::*;
/// use jbhttp::io::ReadWriteAdapter;
/// use jbhttp::server::StreamServer;
///
/// fn handle_hello(req: RawRequest, _: &mut ()) -> Res<Vec<u8>, Vec<u8>> {
///     Ok(Response::new(200).with_body(b"Hello!".to_vec()))
/// }
///
/// let read_buf = b"GET / HTTP/1.1\r\nHost:localhost\r\n\r\n";
/// let mut write_buf = vec![];
/// let stream = ReadWriteAdapter::new(&read_buf[..], &mut write_buf);
/// let mut server = StreamServer::new(stream, handle_hello);
/// server.serve_one();
///
/// assert_eq!(
///     std::str::from_utf8(&write_buf[..]).unwrap(),
///     &format!(
///       "HTTP/1.1 200 OK\r\n\
///        Server: jbhttp::StreamServer/{}\r\n\
///        Connection: keep-alive\r\n\
///        Content-Length: 6\r\n\
///        \r\n\
///        Hello!", jbhttp::VERSION
///     )
/// );
/// ```
pub struct StreamServer<H, S> {
    handler: H,
    stream: S,
    prompt: Option<String>,
}

impl<H, S> StreamServer<H, S> {
    pub fn new(stream: S, handler: H) -> Self {
        Self {
            handler,
            stream,
            prompt: None,
        }
    }
    pub fn set_prompt(&mut self, prompt: &str) {
        self.prompt = Some(prompt.to_string());
    }
}

impl<H, S, C> Server<C> for StreamServer<H, S>
where
    C: Default,
    H: Handler<Vec<u8>, Vec<u8>, Vec<u8>, C>,
    S: Read + Write,
{
    fn serve_one(&mut self) -> Result<(), ServerError> {
        if let Some(prompt) = &self.prompt {
            self.stream.write_all(prompt.as_bytes())?;
        }
        let mut parser = RequestParser::new(&mut self.stream);
        let response = match parser.parse() {
            Ok(request) => self.handler.handle(request, &mut C::default()),
            Err(e) => Err(Response::new(400).with_body(format!("{}", e).as_bytes().to_vec())),
        };
        let response = match response {
            Ok(response) => response,
            Err(response) => response,
        }
        .with_header("Server", &format!("jbhttp::StreamServer/{}", VERSION))
        .with_header("Connection", "keep-alive");
        self.stream.write_all(&response.into_bytes())?;
        self.stream.flush()?;
        Ok(())
    }
}
