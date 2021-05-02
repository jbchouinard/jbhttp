use std::collections::HashMap;
use std::fmt;
use std::io::prelude::*;
use std::str::FromStr;
use std::str::Utf8Error;

use crate::request::{Header, Method, Param, Params, Request};

impl FromStr for Method {
    type Err = RequestParserError;
    fn from_str(s: &str) -> Result<Method> {
        match s {
            "GET" => Ok(Method::GET),
            "HEAD" => Ok(Method::HEAD),
            "POST" => Ok(Method::POST),
            "PUT" => Ok(Method::PUT),
            "PATCH" => Ok(Method::PATCH),
            "DELETE" => Ok(Method::DELETE),
            "CONNECT" => Ok(Method::CONNECT),
            "OPTIONS" => Ok(Method::OPTIONS),
            "TRACE" => Ok(Method::TRACE),
            _ => Err(RequestParserError::new(0, "invalid HTTP method")),
        }
    }
}

const REQUEST_PARSER_BUFFER_SIZE: usize = 1024;

/// A not very good HTTP/1.x request parser.
pub struct RequestParser<T: Read> {
    buffer: [u8; REQUEST_PARSER_BUFFER_SIZE],
    buffer_position: usize,
    buffer_read_size: usize,
    peek: Option<u8>,
    stream_position: usize,
    eof: bool,
    stream: T,
}

const WHITESPACE: [u8; 2] = *b" \t";
const PATH: [u8; 67] = *b"/ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
const QUERY: [u8; 77] =
    *b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~!$&'()*+,;=";
const FRAGMENT: [u8; 81] =
    *b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~!$&'()*+,;=?/:@";

fn one_of(chars: &'static [u8]) -> impl Fn(u8) -> bool {
    move |c: u8| chars.contains(&c)
}

fn whitespace() -> impl Fn(u8) -> bool {
    one_of(&WHITESPACE[..])
}

fn in_range(min: u8, max: u8) -> impl Fn(u8) -> bool {
    move |c: u8| c >= min && c <= max
}

impl<R: Read> RequestParser<R> {
    pub fn new(stream: R) -> Self {
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
    /// Read next chunk from the input stream.
    fn read(&mut self) -> Result<()> {
        self.buffer_read_size = self.stream.read(&mut self.buffer)?;
        self.buffer_position = 0;
        Ok(())
    }
    /// Get next byte from the stream and advance peek. Calls `read` as
    /// needed when end of buffer is reached. Caller is responsible
    /// for setting `eof` to true before calling `next` if the end of stream
    /// is expected, otherwise it will hang on `read`.
    fn next(&mut self) -> Result<Option<u8>> {
        let curr = self.peek;
        if self.eof {
            self.peek = None;
            return Ok(curr);
        }
        if self.buffer_position == self.buffer_read_size {
            self.read()?;
        }
        if self.buffer_position == self.buffer_read_size {
            self.peek = None;
        } else {
            self.peek = Some(self.buffer[self.buffer_position]);
            self.buffer_position += 1;
            self.stream_position += 1;
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
    fn one<F>(&mut self, predicate: &F) -> Result<Vec<u8>>
    where
        F: Fn(u8) -> bool,
    {
        match self.peek {
            Some(peek) => {
                if predicate(peek) {
                    Ok(vec![self.next()?.unwrap()])
                } else {
                    Err(self.error("unexpected character"))
                }
            }
            None => Err(self.error("unexpected character")),
        }
    }
    fn star<F>(&mut self, predicate: &F) -> Result<Vec<u8>>
    where
        F: Fn(u8) -> bool,
    {
        let mut out = vec![];
        loop {
            match self.peek {
                Some(peek) => {
                    if predicate(peek) {
                        out.push(self.next()?.unwrap());
                    } else {
                        return Ok(out);
                    }
                }
                None => return Ok(out),
            }
        }
    }
    fn plus<F>(&mut self, predicate: &F) -> Result<Vec<u8>>
    where
        F: Fn(u8) -> bool,
    {
        let mut out = self.one(predicate)?;
        out.append(&mut self.star(predicate)?);
        Ok(out)
    }
    fn crlf(&mut self) -> Result<()> {
        self.expects(b"\r\n")
    }
    fn until(&mut self, b: u8) -> Result<Vec<u8>> {
        let mut word: Vec<u8> = vec![];
        while self.peek != Some(b) {
            word.push(
                self.next()?
                    .ok_or_else(|| self.error("unexpected end of input"))?,
            )
        }
        Ok(word)
    }
    fn method(&mut self) -> Result<Method> {
        let method = self.plus(&in_range(b'A', b'Z'))?;
        let method = std::str::from_utf8(&method)?;
        Ok(Method::from_str(method)?)
    }
    fn path(&mut self) -> Result<String> {
        if self.peek != Some(b'/') {
            return Err(self.error("expected path starting with /"));
        }
        let path = self.plus(&one_of(&PATH[..]))?;
        Ok(std::str::from_utf8(&path)?.to_string())
    }
    fn query(&mut self) -> Result<String> {
        if self.peek == Some(b'?') {
            self.expect(b'?')?;
            let query = self.plus(&one_of(&QUERY[..]))?;
            Ok(std::str::from_utf8(&query)?.to_string())
        } else {
            Ok("".to_string())
        }
    }
    fn fragment(&mut self) -> Result<String> {
        if self.peek == Some(b'#') {
            self.expect(b'#')?;
            let fragment = self.plus(&one_of(&FRAGMENT[..]))?;
            Ok(std::str::from_utf8(&fragment)?.to_string())
        } else {
            Ok("".to_string())
        }
    }
    fn uri(&mut self) -> Result<(String, String, String)> {
        Ok((self.path()?, self.query()?, self.fragment()?))
    }
    fn header(&mut self) -> Result<(Header, String)> {
        // TODO: only get allowed characters instead, don't just check delimiters
        let header = self.until(b':')?;
        self.expects(b":")?;
        self.star(&whitespace())?;
        let value = self.until(b'\r')?;
        self.crlf()?;
        Ok((
            Header::new(std::str::from_utf8(&header)?),
            std::str::from_utf8(&value)?.to_string(),
        ))
    }
    fn headers(&mut self) -> Result<Vec<(Header, String)>> {
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
                return Err(self.error(&format!("expected {} more bytes", content_length - i)));
            }
        }
        Ok(buf)
    }
    /// Parse next HTTP request in stream.
    pub fn parse(&mut self) -> Result<Request<Vec<u8>>> {
        self.next()?;
        let method = self.method()?;
        self.plus(&whitespace())?;
        let (path, query, fragment) = self.uri()?;
        self.plus(&whitespace())?;
        self.expects(b"HTTP/1.")?;
        self.one(&one_of(&b"01"[..]))?;
        self.crlf()?;
        let headers: HashMap<Header, String> = self.headers()?.into_iter().collect();

        let content_length = match headers.get(&Header::new("content-length")) {
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
            body = None;
        } else {
            self.crlf()?;
            body = Some(self.body(content_length)?);
        }
        let mut request = Request {
            method,
            path,
            query,
            fragment,
            headers,
            body,
            content_length,
            params: Params::new(),
        };
        parse_query_params(&mut request);
        parse_body_params(&mut request);
        Ok(request)
    }
}

fn parse_params(params_str: &str) -> Vec<(String, String)> {
    let mut params = vec![];
    let pairs = params_str.split('&');
    for pair in pairs {
        let parts: Vec<&str> = pair.splitn(2, '=').collect();
        if parts.len() == 2 {
            let name = parts[0].to_string();
            let value = parts[1].to_string();
            params.push((name, value));
        }
    }
    params
}

fn parse_body_params(req: &mut Request<Vec<u8>>) {
    if let Some(body) = &req.body {
        if let Some(content_type) = req.headers.get(&Header::new("content-type")) {
            if content_type == "application/www-form-urlencoded" {
                if let Ok(body) = std::str::from_utf8(body) {
                    for (name, val) in parse_params(body) {
                        req.params.add(Param::Body(name), val);
                    }
                }
            }
        }
    }
}

fn parse_query_params<T>(req: &mut Request<T>) {
    for (name, val) in parse_params(&req.query) {
        req.params.add(Param::Query(name), val);
    }
}

#[derive(Debug, Clone, PartialEq)]
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
            "Error parsing request at position {}: {}",
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

pub type Result<T> = std::result::Result<T, RequestParserError>;

#[cfg(test)]
mod test {
    use super::*;
    use std::str::FromStr;

    pub fn make_request(
        method: &str,
        path: &str,
        query: &str,
        fragment: &str,
        headers: &[(&str, &str)],
        body: Option<&[u8]>,
    ) -> Request<Vec<u8>> {
        let mut req = Request {
            method: Method::from_str(method).unwrap(),
            path: path.to_string(),
            query: query.to_string(),
            fragment: fragment.to_string(),
            headers: headers
                .iter()
                .map(|(h, v)| (Header::new(h), v.to_string()))
                .collect(),
            content_length: body.map_or(0, |b| b.len()),
            body: body.map(|b| b.to_vec()),
            params: Params::new(),
        };
        parse_body_params(&mut req);
        parse_query_params(&mut req);
        req
    }

    fn test_parser(bytes: &[u8], expected: &Request<Vec<u8>>) {
        let mut parser = RequestParser::new(bytes);
        let actual = parser.parse().unwrap();
        assert_eq!(&actual, expected);
    }

    fn test_parser_error(bytes: &[u8], expected: &RequestParserError) {
        let mut parser = RequestParser::new(bytes);
        match parser.parse() {
            Ok(_) => panic!("should have errored"),
            Err(actual) => assert_eq!(&actual, expected),
        }
    }

    #[test]
    fn test_parser_get() {
        test_parser(
            b"GET /path?p1=v1&p2=v2#fragment HTTP/1.1\r\nHost: localhost\r\n\r\n",
            &make_request(
                "GET",
                "/path",
                "p1=v1&p2=v2",
                "fragment",
                &[("host", "localhost")],
                None,
            ),
        )
    }

    #[test]
    fn test_parser_post() {
        test_parser(
            b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 3\r\n\r\nfoo",
            &make_request(
                "POST",
                "/",
                "",
                "",
                &[("host", "localhost"), ("content-length", "3")],
                Some(&b"foo"[..]),
            ),
        )
    }

    #[test]
    fn test_parser_post_body_params() {
        test_parser(
            b"POST / HTTP/1.1\r\nHost:localhost\r\nContent-Length:15\r\nContent-Type:application/www-form-urlencoded\r\n\r\nfoo=bar&foo=baz",
            &make_request(
                "POST",
                "/",
                "",
                "",
                &[("host", "localhost"), ("content-length", "15"), ("content-type", "application/www-form-urlencoded")],
                Some(&b"foo=bar&foo=baz"[..]),
            ),
        )
    }

    #[test]
    fn test_parser_nonsense() {
        test_parser_error(b"FOO", &RequestParserError::new(0, "invalid HTTP method"));
    }

    #[test]
    fn test_parser_content_length_too_long() {
        test_parser_error(
            b"GET / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 10\r\n\r\nfoo",
            &RequestParserError::new(58, "expected 7 more bytes"),
        );
    }
}
