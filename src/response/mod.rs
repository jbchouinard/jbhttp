//! HTTP response and status codes.
use std::collections::HashMap;

pub mod status;

/// An HTTP response.
///
/// # Example
/// ```
/// # use jbhttp::response::RawResponse;
///
/// let response = RawResponse::new(200)
///     .with_header("Content-Type", "text/plain")
///     .with_payload(b"Hello!".to_vec());
///
/// # assert_eq!(response.content_length(), 6);
/// ```
#[derive(Debug)]
pub struct Response<T> {
    pub status_code: u16,
    pub status: String,
    headers: Vec<(String, String)>,
    pub payload: Option<T>,
}

pub type RawResponse = Response<Vec<u8>>;

impl<T> Response<T> {
    /// Create a new Response. Status is automatically set to the default
    /// status for the given code (200 -> "OK", etc.)
    pub fn new(status_code: u16) -> Self {
        Self {
            status_code,
            status: status::default(status_code),
            headers: vec![],
            payload: None,
        }
    }
    pub fn headers(&self) -> HashMap<String, String> {
        self.headers.iter().cloned().collect()
    }
    /// Change status code (does not update status).
    pub fn with_status_code(mut self, status_code: u16) -> Self {
        self.status_code = status_code;
        self
    }
    /// Change status.
    pub fn with_status(mut self, status: &str) -> Self {
        self.status = status.to_string();
        self
    }
    /// Add header.
    pub fn with_header(mut self, header: &str, value: &str) -> Self {
        self.headers.push((header.to_string(), value.to_string()));
        self
    }
    pub fn into_type<S>(self) -> Response<S> {
        Response {
            status_code: self.status_code,
            status: self.status,
            headers: self.headers,
            payload: None,
        }
    }
    pub fn into_raw(self) -> RawResponse {
        self.into_type::<Vec<u8>>()
    }
    /// Sets response payload.
    pub fn with_payload(mut self, payload: T) -> Self {
        self.payload = Some(payload);
        self
    }
}

impl Response<Vec<u8>> {
    /// Get content length.
    pub fn content_length(&self) -> usize {
        match &self.payload {
            Some(body) => body.len(),
            None => 0,
        }
    }
    /// Write HTTP response bytes.
    pub fn into_bytes(mut self) -> Vec<u8> {
        let mut bytes: Vec<u8> = vec![];

        let status_line = format!("HTTP/1.1 {} {}\r\n", self.status_code, self.status);
        bytes.extend(status_line.into_bytes());

        let content_length = self.content_length();
        if content_length > 0 {
            self = self.with_header("Content-Length", &content_length.to_string());
        }

        for (header, value) in &self.headers {
            let header_line = format!("{}: {}\r\n", header, value);
            bytes.extend(header_line.into_bytes());
        }

        bytes.extend(b"\r\n");
        if let Some(body) = &self.payload {
            bytes.extend(body);
        }
        bytes
    }
}

impl<T> Default for Response<T> {
    fn default() -> Self {
        Self::new(200)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_response_bytes() {
        let response = RawResponse::new(500)
            .with_header("Connection", "closed")
            .with_payload(b"foobar!".to_vec());

        let actual = response.into_bytes();
        let expected = b"HTTP/1.1 500 Internal Server Error\r\nConnection: closed\r\nContent-Length: 7\r\n\r\nfoobar!";
        assert_eq!(expected[..], actual[..]);
    }
}
