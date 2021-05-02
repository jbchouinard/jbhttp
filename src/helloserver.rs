use jbhttp::tcp::SimpleTcpServer;
use jbhttp::{Handler, Request, Response, Server};

const HTML: &[u8] = b"<!DOCTYPE html>
<html lang=\"en\">
  <head>
    <meta charset=\"utf-8\">
    <title>Hello!</title>
  </head>
  <body>
    <h1>Hello!</h1>
    <p>Hi from Rust</p>
  </body>
</html>";

struct HelloHandler;

impl Handler for HelloHandler {
    fn handle(&self, request: Request) -> Response {
        println!("{:#?}", request);
        let mut response = Response::new();
        response.set_status(200, "OK");
        response.set_body(HTML);
        response.set_header("Content-Type", "text/html");
        return response;
    }
}

fn main() {
    let server = SimpleTcpServer::new("0.0.0.0:8080").unwrap();
    println!("Listening on 0.0.0.0:8080...");
    server.serve(Box::new(HelloHandler));
}
