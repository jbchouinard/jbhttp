//! A collection of components for building HTTP servers. This is a learning project, use at your own risk.
//! * Multi-threaded [TCP server](crate::server::tcp::TcpServer)
//! * Content-Type based [de/serialization](crate::content::MediaTypeSerde)
//! * [JSON de/serialization](crate::content::json) with [`serde_json`](serde_json)
//! * Path-based [request routing](crate::router::Router)
//! * HTTP method handlers for [APIs](crate::api::Api)
//!
//! # Example
//! ```
//! use jbhttp::io::ReadWriteAdapter;
//! use jbhttp::prelude::*;
//! use jbhttp::router::Router;
//! use jbhttp::server::StreamServer;
//!
//! #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
//! struct Person {
//!     name: String,
//! }
//!
//! impl Person {
//!     pub fn new(name: &str) -> Self {
//!         Self { name: name.to_string() }
//!     }
//! }
//!
//! impl Serialize<TextPlain> for Person {
//!     fn serialize(self) -> Result<Vec<u8>, SerializationError> {
//!         Ok(format!("{}", self.name).into_bytes())
//!     }
//! }
//!
//! impl Deserialize<Person> for TextPlain {
//!     fn deserialize(bytes: Vec<u8>) -> Result<Person, SerializationError> {
//!         Ok(Person::new(std::str::from_utf8(&bytes).unwrap()))
//!     }
//! }
//!
//! fn api() -> Router<Vec<u8>, Vec<u8>, Vec<u8>, ()> {
//!     let handle_sleep = |_req: Request<Vec<u8>>, _ctx: &mut ()| {
//!         std::thread::sleep(std::time::Duration::from_secs(5));
//!         let response: Res<Vec<u8>, Vec<u8>> = Ok(Response::new(200));
//!         response
//!     };
//!
//!     let handle_person = (|req: Request<Person>, _ctx: &mut ()| {
//!         let default_name = "John".to_string();
//!         let name = req.params.get_any("name").unwrap_or(&default_name);
//!         let response: Res<Person, Vec<u8>> = Ok(
//!             Response::new(200).with_body(Person::new(name))
//!         );
//!         response
//!     })
//!     .serdeserialized()
//!     .with_media_type::<ApplicationJson>(true)
//!     .with_media_type::<TextPlain>(false);
//!
//!     Router::new()
//!         .with_route("/person/?name", handle_person)
//!         .with_route("/sleep", handle_sleep)
//! }
//!
//! fn main() {
//!     let request = b"GET /person/Bob HTTP/1.0\r\nAccept: */*\r\n\r\n";
//!     println!("Request:\n{}", std::str::from_utf8(request).unwrap());
//!     let mut write_buf = vec![];
//!     let stream = ReadWriteAdapter::new(&request[..], &mut write_buf);
//!     let mut server = StreamServer::new(stream, api());
//!     server.serve_one().unwrap();
//!     println!("Response:\n{}", std::str::from_utf8(&write_buf[..]).unwrap());
//! }
//! ```
pub mod api;
pub mod auth;
pub mod content;
pub mod filter;
pub mod handler;
pub mod io;
pub mod prelude;
pub mod request;
pub mod response;
pub mod router;
pub mod runner;
pub mod server;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
