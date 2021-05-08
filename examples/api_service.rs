use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use lazy_static::lazy_static;
use log::*;
use rusqlite::{params, Connection};
use structopt::StructOpt;

use jbhttp::auth::AuthError;
use jbhttp::prelude::*;
use jbhttp::router::Router;
use jbhttp::server::TcpServer;

// jbhttp uses the log crate for logging
// TcpServer has detailed request-response logging at TRACE level
// just a very basic stderr setup for demonstration
fn setup_logging(verbosity: usize) {
    stderrlog::new()
        .module(module_path!())
        .module("jbhttp")
        .verbosity(verbosity)
        .timestamp(stderrlog::Timestamp::Millisecond)
        .init()
        .unwrap();
}

fn main() {
    let opt = Opt::from_args();
    setup_logging(opt.verbose);
    setup_db();

    let bind = format!("0.0.0.0:{}", opt.port);
    let mut server = TcpServer::new(
        &bind,
        // multithread config:
        // n=0: create a new thread for each request (not recommended!)
        // n=1: single threaded
        // n=2+: threadpool of size n
        opt.threads,
        // network socket read/write timeout
        Some(std::time::Duration::from_secs(30)),
        // An application is a request handler, which accepts Request<Vec<u8>>
        // and returns Response<Vec<u8>>. Request and Response can hold other
        // payload types, but Server can only deal with bytes.
        app(),
    )
    .unwrap();
    info!("listening on {}", &bind);
    server.serve_forever();
}

// A request handler must implement the Handler trait, which has 4 type parameters:
//  - Request payload
//  - Ok response payload
//  - Err response payload
//  - Context
//
// To handle requests from a server, all payload types must be bytes (Vec<u8>).
// This can be done by implementing a bytes Handler directly, or by
// implementing Handler for other types and adding de/serialization filters.
//
// The Ok and Err response payloads can be the same type, but they
// might not be; for example, for Person API it doesn't make sense to return
// a Person on error, we want to return a detailed error, which isn't
// anything like a Person.
//
// The context type can be anything, as long as it implements Default.
// Each request is tied to a context; the server creates a new context with default()
// for each, and request handlers and filters get a mutable ref to it.
// It can be used to keep track of whatever, for example, the Context
// type referred to here is implemented later, it is used to keep track of
// authentication and other details.
fn app() -> impl Handler<Vec<u8>, Vec<u8>, Vec<u8>, Context> {
    // The app is built up in layers. PersonApi is a REST endpoint which handles
    // GET, POST and DELETE operations for Person. It takes Request<Person> and returns
    // Result<Response<Person>, Response<ApiError>>, or Res<Person, ApiError> for short.
    // To use it to for handling requests from Server, we must add de/serialization for:
    //  - Request<Vec<u8>> -> Request<Person>
    //  - Response<Person> -> Response<Vec<u8>>
    //  - Response<ApiError> -> Response<Vec<u8>>
    //
    // Apps are built from a few different kinds of components:
    //  - A Handler handles a request and returns an Ok or Err response.
    //  - A Router dispatches requests to any number of Handlers
    //  - A RequestFilter can modify (or replace) requests and pass them forward,
    //    or pre-empt the request with its own response (typically, in case of error)
    //  - An OkFilter can modify (or replace) Ok responses
    //  - An ErrFilter can modify (or replace) Err responses
    //  - A ResFilter can modify (or replace) both Ok and Err responses
    let person_api = PersonApi
        // Api is a convenience trait to help implement handlers with methods
        // that map to HTTP verbs - see the implementation of PersonApi below.
        // The handler method just constructs a Handler for types implementing Api.
        .handler()
        // returns impl Handler<Person, Person, ApiError, Context>
        //
        // jbhttp includes filters for automatic serialization based on Accept
        // request headers, and deserialization based on Content-Type request headers.
        // To use them, Serialize and Deserialize traits must be implemented for
        // Person for the relevant media types. The following methods are
        // provided for any type implementing Handler:
        //  - serialized: Serializes Ok response payloads
        //  - serialized_error: Serializes Err response payloads
        //  - deserialized:  Deserializes request payloads
        //  - serdeserialized: Deserializes request payloads and serializes
        //    Ok response payloads
        .serdeserialized()
        // returns impl Handler<Vec<u8>, Vec<u8>, ApiError, Context>
        //
        // Media types are represented as Rust types (unit structs).
        // To support application/json for PersonApi, the following must be implemented:
        //  - jbhttp::content::Serialize<ApplicationJson> for Person
        // - jbhttp::content::Deserialize<Person> for ApplicationJson
        //
        // With the json crate feature enabled, since our Person struct
        // derives serde::Serialize and serde::Deserialize, ApplicationJson
        // de/serialization traits are automatically implemented for Person.
        //
        // with_media_type can be called multiple times; order matters,
        // if a client accepts multiple types equally, like Accept: */*, the first
        // acceptable media type is used.
        .with_media_type::<ApplicationJson>();

    // Router associates URLs with handlers, and produces 404s response otherwise.
    Router::new()
        // Route's have limited pattern matching, see Router documentation for details.
        // For our case here, we want to match an id parameter; as of now,
        // Router does not do parameter parsing, so id will be a String
        // not, necessarily numeric.
        .with_route("/person/?id", person_api)
        //
        // authenticated is a convenience method on Handler's which constructs
        // a specialized request filter for authentication.
        // It takes a callable of type
        // Fn(&Request<T>, &mut Context) -> Result<(), AuthError>
        // and runs on it on each request; on Ok(()), the request is
        // passed forward; on Err(e), it is pre-empted with a 401 response.
        //
        // Alternatively, authentication could be implemented with a generic
        // RequestFilter using (.request_filter(f)), which takes a callable of type:
        // Fn(Request<I>, &mut C) -> Result<Request<FI>, Response<E>>
        // where a return of Ok(some_request) passes some_request forward,
        // while a return of Err(some_response) pre-empts the request and
        // returns some_response.
        .authenticated(authenticate)
        //
        // An ErrorFilter takes a callable with signature:
        // Fn(Response<FE>, &mut C) -> Response<E>
        // and applies it (only) to Err responses.
        //
        // Various parts of the framework can generate error responses;
        // de/serializers can generate 400, 405, 415, etc.; framework-generated
        // errors always have a None payload, since the implementation is
        // agnostic to payload type.
        //
        // This function generates default ApiError payloads based on status
        // for empty framework-generated error responses. Note that some errors may
        // occur at a higher level than here, i.e. a 400 or 500 error originating
        // from TcpServer will not go through generate_error and will be empty.
        .error_filter(generate_error)
        // Serialize ApiError; again, since ApiError derives
        // serde::Serialize, ApplicationJson is automatically supported.
        .serialized_error()
        // returns impl Handler<Vec<u8>, Vec<u8>, Vec<u8>, Context>
        .with_media_type::<ApplicationJson>()
        //
        // res_filter constructs a ResFilter from a callable of type
        // Fn(Res<O, E>, &mut Context) -> Res<FO, FE>
        // ResFilter is a combination OkFilter and ErrFilter, it processes
        // both types of response; consequently it's the only filter that can change
        // an Ok response into an Err response, or vice-versa.
        //
        // It's used here just to add a request ID to all responses,
        // whether they are Ok or Err.
        .res_filter(add_request_id)

    // Filters are composable, we could have called the methods here
    // in different order, but the order of RequestFilters especially
    // matters, since they can pre-empt requests.
    //
    // For example, if we called authenticated after res_filter instead,
    // a request that fails authentication would not have the request ID
    // header set, since it would be pre-empted by the authenticator before
    // reaching the add_request_id filter.
    //
    // Here is a request/response flow diagram of the service, with indications
    // where a request could be pre-empted with error codes.
    //
    // PersonApi
    //   ↑   ↓
    // MediaTypeSerde
    //   ↑   ↓   ↧ [400, 405, 415, 500]
    // Router
    //   ↑   ↓   ↧ [404]
    // Authenticator
    //   ↑   ↓   ↧ [401]
    // ErrorFilter(generate_error)
    //   ↑   ↓
    // MediaTypeErrorSerialize
    //   ↑   ↓
    // ResFilter(add_request_id)
    //   ↑   ↓
    // TcpServer
    //   ↑   ↓   ↧ [400, 500]
    //
    // Handlers routed by Router must have matching types,
    // if we had multiple APIs with heterogenous types, each would have
    // to be de/serialized before routing - here it doesn't matter
    // since there's only one handler.
}

// Derive serde traits to automatically implement JSON de/serialization.
#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct Person {
    /// Database ID, should be zero for newly created data
    #[serde(default)]
    id: i64,
    name: String,
    age: u16,
}

struct PersonApi;

impl PersonApi {
    fn error(&self, code: u16, reason: &str, context: &Context) -> Response<ApiError> {
        ApiError::new(code, reason, context.request_id).into_response()
    }
}

// The Api trait can be used to implement a handler with methods mapped
// to HTTP verbs GET, POST, PUT, PATCH an DELETE. The methods have
// default implementations returning 405 for unimplemented verbs.
// The Api trait provides a handler() method, it does not impl Handler itself.
//
// This is just a basic CRUD REST API (well, without the U) for demonstration.
impl Api<Person, Person, ApiError, Context> for PersonApi {
    fn get(&self, request: Request<Person>, context: &mut Context) -> Res<Person, ApiError> {
        // All params are String, so it must be parsed here
        let id = match request.params.get_any("id") {
            Some(id) => match str::parse::<i64>(&id) {
                Ok(id) => id,
                _ => return Err(self.error(400, "id must be an integer", context)),
            },
            None => return Err(self.error(400, "missing parameter id", context)),
        };
        let conn = CONNECTION.lock().unwrap();
        match Person::select(&conn, id) {
            Some(person) => Ok(Response::new(200).with_payload(person)),
            None => Err(self.error(404, "resource not found", context)),
        }
    }
    fn post(&self, request: Request<Person>, context: &mut Context) -> Res<Person, ApiError> {
        let mut person = match request.payload {
            Some(person) => person,
            _ => return Err(self.error(400, "expected request body", context)),
        };
        if person.id != 0 {
            return Err(self.error(400, "id must be 0", context));
        }
        let conn = CONNECTION.lock().unwrap();
        person.insert(&conn);
        Ok(Response::new(201).with_header("Location", &format!("/person/{}", person.id)))
    }
    fn delete(&self, request: Request<Person>, context: &mut Context) -> Res<Person, ApiError> {
        match self.get(request, context) {
            Ok(response) => {
                let conn = CONNECTION.lock().unwrap();
                response.payload.unwrap().delete(&conn);
                Ok(Response::new(204))
            }
            Err(response) => Err(response),
        }
    }
}

/// API auth username
#[derive(Debug)]
struct User(String);

// For generating request IDs.
static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);

// Types used as context for Handler must implement Default, each request
// gets a default() context from the server.
#[derive(Debug)]
struct Context {
    request_id: u64,
    user: Option<User>,
}

impl Default for Context {
    fn default() -> Self {
        Context {
            user: None,
            request_id: REQUEST_COUNTER.fetch_add(1, Ordering::SeqCst),
        }
    }
}

impl Context {
    fn set_user(&mut self, user: User) {
        self.user = Some(user)
    }
}

// Error type for the API; derive serde traits to automatically
// get JSON serialization.
#[derive(serde::Serialize, Debug)]
struct ApiError {
    status_code: u16,
    reason: String,
    request_id: u64,
}

impl ApiError {
    fn new(status_code: u16, reason: &str, request_id: u64) -> Self {
        ApiError {
            status_code,
            reason: reason.to_string(),
            request_id,
        }
    }
    fn into_response(self) -> Response<Self> {
        Response::new(self.status_code).with_payload(self)
    }
}

// Adds a default ApiError payload to framework-geneated error responses with no payload
fn generate_error(mut response: Response<ApiError>, context: &mut Context) -> Response<ApiError> {
    if response.payload.is_none() {
        response.payload = Some(ApiError::new(
            response.status_code,
            &response.status,
            context.request_id,
        ));
    }
    response
}

// Add request id header to all responses
fn add_request_id<O, E>(res: Res<O, E>, context: &mut Context) -> Res<O, E> {
    match res {
        Ok(response) => {
            Ok(response.with_header("X-Request-Id", &format!("{}", context.request_id)))
        }
        Err(response) => {
            Err(response.with_header("X-Request-Id", &format!("{}", context.request_id)))
        }
    }
}

// API key based "authentication", obviously a real implementation would
// have an actual system for generating and validating keys, not hardcoded
fn authenticate<I>(request: &Request<I>, context: &mut Context) -> Result<(), AuthError> {
    match request.headers.get(&Header::new("x-api-key")) {
        Some(api_key) => match &api_key[..] {
            "secret" => {
                context.set_user(User("admin".to_string()));
                Ok(())
            }
            _ => Err(AuthError::new("invalid API key")),
        },
        None => Err(AuthError::new("missing API key")),
    }
}

// CLI arg parsing
#[derive(Debug, StructOpt)]
#[structopt(name = "api_service", about = "Example REST API service.")]
struct Opt {
    #[structopt(short, long, default_value = "8080")]
    port: u16,
    #[structopt(long, default_value = "1")]
    threads: usize,
    #[structopt(short = "v", long = "verbose", parse(from_occurrences))]
    verbose: usize,
}

// Everything beyond this point is just the most terrible, minimal
// sqlite setup to get the example working with no external DB setup.
lazy_static! {
    static ref CONNECTION: Arc<Mutex<Connection>> =
        Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
}

fn setup_db() {
    let conn = CONNECTION.lock().unwrap();
    conn.execute(CREATE_TABLE_PERSON, []).unwrap();
}

const CREATE_TABLE_PERSON: &str = "CREATE TABLE person
(
    id        INTEGER PRIMARY KEY,
    name      TEXT NOT NULL,
    age       INTEGER
)";

impl Person {
    fn insert(&mut self, conn: &Connection) {
        assert_eq!(self.id, 0);
        conn.execute(
            "INSERT INTO person (name, age) VALUES (?1, ?2)",
            params![self.name, self.age],
        )
        .unwrap();
        self.id = conn.last_insert_rowid();
    }
    fn delete(&mut self, conn: &Connection) {
        conn.execute("DELETE FROM person WHERE id=?1", params![self.id])
            .unwrap();
        self.id = 0;
    }
    fn select(conn: &Connection, id: i64) -> Option<Self> {
        let mut stmt = conn
            .prepare("SELECT id, name, age FROM person WHERE id=?1")
            .unwrap();
        let mut person_iter = stmt
            .query_map([id], |row| {
                Ok(Person {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    age: row.get(2)?,
                })
            })
            .ok()?;
        person_iter.next().map(|r| r.unwrap())
    }
}
