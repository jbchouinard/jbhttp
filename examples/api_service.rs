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

lazy_static! {
    static ref CONNECTION: Arc<Mutex<Connection>> =
        Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
}

const CREATE_TABLE_PERSON: &str = "CREATE TABLE person
(
    id        INTEGER PRIMARY KEY,
    name      TEXT NOT NULL,
    age       INTEGER
)";

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct Person {
    #[serde(default)]
    id: i64,
    name: String,
    age: u16,
}

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

#[derive(Debug)]
struct User(String);

static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
struct Context {
    request_id: u64,
    user: Option<User>,
}

impl Context {
    fn set_user(&mut self, user: User) {
        self.user = Some(user)
    }
}

impl Default for Context {
    fn default() -> Self {
        Context {
            user: None,
            request_id: REQUEST_COUNTER.fetch_add(1, Ordering::SeqCst),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct ApiError {
    status_code: u16,
    reason: String,
    request_id: u64,
}

impl ApiError {
    fn new(status_code: u16, reason: &str, context: &Context) -> Self {
        ApiError {
            status_code: status_code,
            reason: reason.to_string(),
            request_id: context.request_id,
        }
    }
    fn into_response(self) -> Response<Self> {
        Response::new(self.status_code).with_body(self)
    }
}

fn generate_error(mut response: Response<ApiError>, context: &mut Context) -> Response<ApiError> {
    if response.body.is_none() {
        response.body = Some(ApiError::new(
            response.status_code,
            &response.status,
            context,
        ));
    }
    response
}

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

struct PersonApi;

impl PersonApi {
    fn error(&self, code: u16, reason: &str, context: &Context) -> Response<ApiError> {
        ApiError::new(code, reason, context).into_response()
    }
}

impl Api<Person, Person, ApiError, Context> for PersonApi {
    fn get(&self, request: Request<Person>, context: &mut Context) -> Res<Person, ApiError> {
        let id = match request.params.get_any("id") {
            Some(id) => match str::parse::<i64>(&id) {
                Ok(id) => id,
                _ => return Err(self.error(400, "id must be an integer", context)),
            },
            None => return Err(self.error(400, "missing parameter id", context)),
        };
        let conn = CONNECTION.lock().unwrap();
        match Person::select(&conn, id) {
            Some(person) => Ok(Response::new(200).with_body(person)),
            None => Err(self.error(404, "resource not found", context)),
        }
    }
    fn post(&self, request: Request<Person>, context: &mut Context) -> Res<Person, ApiError> {
        let mut person = match request.body {
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
                response.body.unwrap().delete(&conn);
                Ok(Response::new(204))
            }
            Err(response) => Err(response),
        }
    }
}

fn setup_db() {
    let conn = CONNECTION.lock().unwrap();
    conn.execute(CREATE_TABLE_PERSON, []).unwrap();
}

fn setup_logging(verbosity: usize) {
    stderrlog::new()
        .module(module_path!())
        .module("jbhttp")
        .verbosity(verbosity)
        .timestamp(stderrlog::Timestamp::Millisecond)
        .init()
        .unwrap();
}

#[derive(Debug, StructOpt)]
#[structopt(name = "api_service", about = "Example REST API server.")]
struct Opt {
    #[structopt(short, long, default_value = "8080")]
    port: u16,
    #[structopt(long, default_value = "1")]
    threads: usize,
    #[structopt(short = "v", long = "verbose", parse(from_occurrences))]
    verbose: usize,
}

fn main() {
    let opt = Opt::from_args();
    setup_logging(opt.verbose);
    setup_db();

    let person_api = PersonApi
        .handler()
        .serdeserialized()
        .with_media_type::<ApplicationJson>(true);

    let app = Router::new()
        .with_route("/person/?id", person_api)
        .authenticated(authenticate)
        .error_filter(generate_error)
        .serialized_error()
        .with_media_type::<ApplicationJson>(true)
        .res_filter(add_request_id);

    let bind = format!("0.0.0.0:{}", opt.port);
    let mut server = TcpServer::new(&bind, opt.threads, None, app).unwrap();
    info!("listening on {}", &bind);
    server.serve_forever();
}
