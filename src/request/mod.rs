//! HTTP request and parser.
use std::collections::HashMap;

pub use header::*;

pub mod header;
pub mod parser;

/// An HTTP Request.
#[derive(Debug, Clone, PartialEq)]
pub struct Request<T> {
    pub method: Method,
    pub path: String,
    pub query: String,
    pub fragment: String,
    pub headers: HashMap<Header, String>,
    pub payload: Option<T>,
    pub content_length: usize,
    pub params: Params,
}

pub type RawRequest = Request<Vec<u8>>;

impl<T> Default for Request<T> {
    fn default() -> Self {
        Self {
            method: Method::GET,
            path: "/".to_string(),
            query: "".to_string(),
            fragment: "".to_string(),
            headers: vec![("Host".to_string().into(), "localhost".to_string())]
                .into_iter()
                .collect(),
            payload: None,
            content_length: 0,
            params: Params::new(),
        }
    }
}

impl<T> Request<T> {
    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.insert(Header::new(name), value.to_string());
        self
    }
    pub fn into_type<S>(self) -> Request<S> {
        Request {
            method: self.method,
            path: self.path,
            query: self.query,
            fragment: self.fragment,
            headers: self.headers,
            payload: None,
            content_length: self.content_length,
            params: self.params,
        }
    }
    pub fn accept(&self) -> Result<Option<Accept>, HeaderParseError> {
        match self.headers.get(&Header::new("accept")) {
            Some(s) => Ok(Some(str::parse::<Accept>(s)?)),
            None => Ok(None),
        }
    }
    pub fn content_type(&self) -> Result<Option<ContentType>, HeaderParseError> {
        match self.headers.get(&Header::new("content-type")) {
            Some(s) => Ok(Some(str::parse::<ContentType>(s)?)),
            None => Ok(None),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Method {
    GET,
    HEAD,
    POST,
    PUT,
    PATCH,
    DELETE,
    CONNECT,
    OPTIONS,
    TRACE,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Param {
    Path(String),
    Query(String),
    Body(String),
}

impl ToString for Param {
    fn to_string(&self) -> String {
        match self {
            Self::Path(s) => s.clone(),
            Self::Query(s) => s.clone(),
            Self::Body(s) => s.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Params(HashMap<Param, Vec<String>>);

impl Default for Params {
    fn default() -> Self {
        Self::new()
    }
}

impl Params {
    pub fn new() -> Self {
        Params(HashMap::new())
    }
    pub fn add(&mut self, p: Param, s: String) {
        self.0.entry(p).or_insert_with(Vec::new).push(s);
    }
    // Get all named params of specific type (path, query or body)
    pub fn get_all(&self, p: &Param) -> Option<&Vec<String>> {
        self.0.get(p)
    }
    // Get first named param of specific type (path, query, or body)
    pub fn get_first(&self, p: &Param) -> Option<&String> {
        match self.0.get(p).map(|v| &v[..]) {
            None => None,
            Some([]) => None,
            Some(ps) => Some(&ps[0]),
        }
    }
    // Get named param from anywhere, return first found from (in order): path, query, body
    pub fn get_any(&self, name: &str) -> Option<&String> {
        let try_params = [
            Param::Path(name.to_string()),
            Param::Query(name.to_string()),
            Param::Body(name.to_string()),
        ];
        for p in try_params.iter() {
            if let Some(val) = self.get_first(p) {
                return Some(val);
            }
        }
        None
    }
}
