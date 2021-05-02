use std::collections::HashMap;
use std::fmt;
use std::io::prelude::*;
use std::str::Utf8Error;

#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    pub uri: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub content_length: usize,
}

const REQUEST_PARSER_BUFFER_SIZE: usize = 1024;

#[derive(Debug, Clone)]
pub struct RequestParserError {
    position: usize,
    reason: String,
}

impl RequestParserError {
    pub fn new(position: usize, reason: &str) -> Self {
        Self {
            position,
            reason: reason.to_string(),
        }
    }
}

impl fmt::Display for RequestParserError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "error parsing request at {}: {}",
            self.position, self.reason
        )
    }
}

impl From<std::io::Error> for RequestParserError {
    fn from(err: std::io::Error) -> Self {
        RequestParserError::new(0, &err.to_string())
    }
}

impl From<Utf8Error> for RequestParserError {
    fn from(err: Utf8Error) -> Self {
        RequestParserError::new(0, &err.to_string())
    }
}

type Result<T> = std::result::Result<T, RequestParserError>;

pub struct RequestParser<T: Read> {
    buffer: [u8; REQUEST_PARSER_BUFFER_SIZE],
    buffer_position: usize,
    buffer_read_size: usize,
    peek: Option<u8>,
    stream_position: usize,
    eof: bool,
    stream: T,
}

impl<T: Read> RequestParser<T> {
    pub fn new(stream: T) -> Self {
        Self {
            peek: None,
            buffer: [0; REQUEST_PARSER_BUFFER_SIZE],
            stream,
            buffer_position: 0,
            buffer_read_size: 0,
            stream_position: 0,
            eof: false,
        }
    }
    fn error(&self, reason: &str) -> RequestParserError {
        RequestParserError::new(self.stream_position, reason)
    }
    fn read(&mut self) -> Result<()> {
        self.buffer_read_size = self.stream.read(&mut self.buffer)?;
        self.buffer_position = 0;
        self.stream_position += self.buffer_read_size;
        Ok(())
    }
    fn next(&mut self) -> Result<Option<u8>> {
        let curr = self.peek;
        if self.eof {
            self.peek = None;
            return Ok(curr);
        }
        if self.buffer_position >= self.buffer_read_size {
            self.read()?;
        }
        if self.buffer_position >= self.buffer_read_size {
            self.peek = None;
        } else {
            self.peek = Some(self.buffer[self.buffer_position]);
            self.buffer_position += 1;
        }
        Ok(curr)
    }
    fn expect(&mut self, b: u8) -> Result<()> {
        let next = self.next()?;
        if next == Some(b) {
            Ok(())
        } else {
            Err(self.error(&format!("expected '{}'", b as char)))
        }
    }
    fn expects(&mut self, bs: &[u8]) -> Result<()> {
        for b in bs {
            self.expect(*b)?;
        }
        Ok(())
    }
    fn space(&mut self) -> Result<()> {
        self.expect(b' ')
    }
    fn newline(&mut self) -> Result<()> {
        self.expects(b"\r\n")
    }
    fn until(&mut self, b: u8) -> Result<Vec<u8>> {
        let mut word: Vec<u8> = vec![];
        while self.peek != Some(b) {
            word.push(self.next()?.unwrap())
        }
        Ok(word)
    }
    fn method(&mut self) -> Result<String> {
        let method = self.until(b' ')?;
        Ok(std::str::from_utf8(&method)?.to_string())
    }
    fn uri(&mut self) -> Result<String> {
        let method = self.until(b' ')?;
        Ok(std::str::from_utf8(&method)?.to_string())
    }
    fn header(&mut self) -> Result<(String, String)> {
        let header = self.until(b':')?;
        self.expects(b": ")?;
        let value = self.until(b'\r')?;
        self.newline()?;
        Ok((
            std::str::from_utf8(&header)?.to_string().to_lowercase(),
            std::str::from_utf8(&value)?.to_string(),
        ))
    }
    fn headers(&mut self) -> Result<Vec<(String, String)>> {
        let mut headers = vec![];
        while self.peek != Some(b'\r') {
            headers.push(self.header()?);
        }
        Ok(headers)
    }
    fn body(&mut self, content_length: usize) -> Result<Vec<u8>> {
        let mut buf = vec![];
        for i in 0..content_length {
            if i == content_length - 1 {
                self.eof = true;
            }
            if let Some(b) = self.next()? {
                buf.push(b);
            } else {
                return Err(
                    self.error(&format!("expected {} more bytes", content_length - i).to_string())
                );
            }
        }
        Ok(buf)
    }
    pub fn parse(&mut self) -> Result<Request> {
        self.next()?;
        let method = self.method()?;
        self.space()?;
        let uri = self.uri()?;
        self.space()?;
        self.expects(b"HTTP/1.1")?;
        self.newline()?;
        let headers: HashMap<String, String> = self.headers()?.into_iter().collect();

        let content_length = match headers.get("content-length") {
            Some(cl_str) => match str::parse::<usize>(cl_str) {
                Ok(cl) => cl,
                Err(_) => return Err(self.error("invalid content-length")),
            },
            None => 0,
        };
        let body;
        if content_length == 0 {
            self.expect(b'\r')?;
            self.eof = true;
            self.expect(b'\n')?;
            body = vec![];
        } else {
            self.newline()?;
            body = self.body(content_length)?;
        }
        Ok(Request {
            method: method,
            uri: uri,
            headers: headers,
            body: body,
            content_length,
        })
    }
}

pub struct Response {
    status_code: u16,
    status: String,
    headers: HashMap<String, String>,
    body: Option<Vec<u8>>,
}

impl Response {
    pub fn new() -> Self {
        Self {
            status_code: 200,
            status: "OK".to_string(),
            headers: HashMap::new(),
            body: None,
        }
    }
    pub fn set_status(&mut self, status_code: u16, status: &str) {
        self.status_code = status_code;
        self.status = status.to_string();
    }
    pub fn set_header(&mut self, header: &str, value: &str) {
        self.headers.insert(header.to_string(), value.to_string());
    }
    pub fn set_body(&mut self, body: &[u8]) {
        self.body = Some(body.to_vec());
        self.set_header("Content-Length", &format!("{}", body.len()));
    }
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes: Vec<u8> = vec![];
        let status_line = format!("HTTP/1.1 {} {}\r\n", self.status_code, self.status);
        bytes.extend(status_line.into_bytes());
        for (header, value) in &self.headers {
            let header_line = format!("{}: {}\r\n", header, value);
            bytes.extend(header_line.into_bytes());
        }
        bytes.extend(b"\r\n");
        if let Some(body) = &self.body {
            bytes.extend(body);
        }
        bytes
    }
}

pub trait Handler {
    fn handle(&self, request: Request) -> Response;
}

pub trait Server {
    fn serve(&self, handler: Box<dyn Handler>);
}
