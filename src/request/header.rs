use std::fmt;
use std::hash;
use std::str::FromStr;

use crate::content::MediaTypeMatch;

#[derive(Debug, Clone)]
pub struct Header(String);

impl Header {
    pub fn new(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl PartialEq for Header {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_lowercase() == other.0.to_lowercase()
    }
}

impl Eq for Header {}

impl hash::Hash for Header {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.0.to_lowercase().hash(state);
    }
}

impl From<String> for Header {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<Header> for String {
    fn from(s: Header) -> Self {
        s.0
    }
}

pub struct MediaTypePreference {
    pub mime_type: String,
    pub mime_subtype: String,
    pub quality: f32,
}

impl MediaTypeMatch for &MediaTypePreference {
    fn matches(&self, mime_type: &str, mime_subtype: &str) -> bool {
        if &self.mime_type[..] == "*" {
            true
        } else if self.mime_type == mime_type {
            if &self.mime_subtype[..] == "*" {
                true
            } else {
                self.mime_subtype == mime_subtype
            }
        } else {
            false
        }
    }
}

impl MediaTypePreference {
    pub fn quality(&self) -> f32 {
        self.quality
    }
}

#[derive(Debug)]
pub struct HeaderParseError {
    header: String,
    reason: String,
}

impl HeaderParseError {
    pub fn new(header: &str, reason: &str) -> Self {
        Self {
            header: header.to_string(),
            reason: reason.to_string(),
        }
    }
}

impl fmt::Display for HeaderParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::result::Result<(), fmt::Error> {
        write!(f, "error parsing header '{}': {}", self.header, self.reason)
    }
}

type Result<T> = std::result::Result<T, HeaderParseError>;

// Accept: <MIME_type>/<MIME_subtype>
// Accept: <MIME_type>/*
// Accept: */*
// Multiple types, weighted with the quality value syntax:
// Accept: text/html, application/xhtml+xml, application/xml;q=0.9, image/webp, */*;q=0.8
impl FromStr for MediaTypePreference {
    type Err = HeaderParseError;
    fn from_str(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split(';').collect();
        let (content_type, q) = match &parts[..] {
            [content_type] => (content_type.to_string(), 1.0),
            [content_type, q] => match str::parse::<f32>(q) {
                Ok(q) => (content_type.to_string(), q),
                Err(_) => return Err(HeaderParseError::new("Accept", "invalid q value")),
            },
            _ => return Err(HeaderParseError::new("Accept", "invalid mimetype format")),
        };
        let parts: Vec<&str> = content_type.split('/').collect();
        let (mime_type, mime_subtype) = match &parts[..] {
            [mime_type, mime_subtype] => (mime_type.to_string(), mime_subtype.to_string()),
            _ => return Err(HeaderParseError::new("Accept", "invalid mimetype format")),
        };
        Ok(MediaTypePreference {
            mime_type,
            mime_subtype,
            quality: q,
        })
    }
}

pub struct Accept {
    prefs: Vec<MediaTypePreference>,
}

impl Accept {
    pub fn iter(&self) -> std::slice::Iter<MediaTypePreference> {
        self.prefs.iter()
    }
}

impl FromStr for Accept {
    type Err = HeaderParseError;
    fn from_str(s: &str) -> Result<Self> {
        let mut vec = vec![];
        let parts = s.split(',');
        for part in parts {
            if let Ok(ctp) = str::parse::<MediaTypePreference>(part.trim()) {
                vec.push(ctp);
            }
        }
        vec.sort_by(|a, b| a.quality.partial_cmp(&b.quality).unwrap());
        Ok(Self { prefs: vec })
    }
}

pub struct ContentType {
    pub mime_type: String,
    pub mime_subtype: String,
    pub charset: Option<String>,
    pub boundary: Option<String>,
}

impl MediaTypeMatch for &ContentType {
    fn matches(&self, mime_type: &str, mime_subtype: &str) -> bool {
        self.mime_type == mime_type && self.mime_subtype == mime_subtype
    }
}

// Content-Type: text/html; charset=UTF-8
// Content-Type: multipart/form-data; boundary=something
impl FromStr for ContentType {
    type Err = HeaderParseError;
    fn from_str(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split(';').collect();
        if parts.is_empty() {
            return Err(HeaderParseError::new("Content-Type", "empty"));
        }
        let media_type_parts: Vec<&str> = parts[0].trim().split('/').collect();
        let (mime_type, mime_subtype) = match &media_type_parts[..] {
            [mime_type, mime_subtype] => (mime_type.to_string(), mime_subtype.to_string()),
            _ => {
                return Err(HeaderParseError::new(
                    "Content-Type",
                    "invalid mimetype format",
                ))
            }
        };
        let mut charset = None;
        let mut boundary = None;
        for part in &parts[1..] {
            let parts: Vec<&str> = part.trim().split('=').collect();
            match parts[..] {
                [key, value] => match key {
                    "charset" => charset = Some(value.to_string()),
                    "boundary" => boundary = Some(value.to_string()),
                    _ => (),
                },
                _ => return Err(HeaderParseError::new("Content-Type", "invalid key-value")),
            }
        }
        Ok(ContentType {
            mime_type,
            mime_subtype,
            charset,
            boundary,
        })
    }
}
