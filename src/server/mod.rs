//! HTTP Server components.
use std::fmt;
use std::io;

use log::error;

pub mod stream;
pub mod tcp;

pub use stream::StreamServer;
pub use tcp::TcpServer;

#[derive(Debug)]
pub struct ServerError {
    message: String,
}

impl ServerError {
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "server error: {}", &self.message)
    }
}

impl From<io::Error> for ServerError {
    fn from(err: io::Error) -> Self {
        ServerError::new(&format!("IOError({})", err))
    }
}

pub trait Server<C: Default> {
    /// Serve one request, must be implemented.
    fn serve_one(&mut self) -> Result<(), ServerError>;
    /// Serve requests forever (default implementation).
    fn serve_forever(&mut self) {
        loop {
            match self.serve_one() {
                Ok(()) => (),
                Err(e) => error!("{}", e),
            }
        }
    }
}
