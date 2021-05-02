use std::io::prelude::*;
use std::net::TcpListener;

use crate::http::{Handler, RequestParser, Response, Server};

pub struct SimpleTcpServer {
    listener: TcpListener,
}

impl SimpleTcpServer {
    pub fn new(bind_addr: &str) -> Result<Self, std::io::Error> {
        let listener = TcpListener::bind(bind_addr)?;
        Ok(Self { listener })
    }
}

impl Server for SimpleTcpServer {
    fn serve(&self, handler: Box<dyn Handler>) {
        for stream in self.listener.incoming() {
            let mut stream = stream.unwrap();
            let mut parser = RequestParser::new(&mut stream);
            let mut response = match parser.parse() {
                Ok(request) => handler.handle(request),
                Err(_) => {
                    let mut response = Response::new();
                    response.set_status(400, "Bad Request");
                    response
                }
            };
            response.set_header("Connection", "closed");
            let response = response.to_bytes();
            stream.write(&response).unwrap();
        }
    }
}
