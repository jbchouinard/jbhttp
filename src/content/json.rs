//! Automatic implementation of JSON de/serialization for types that implement
//! [`serde::Serialize`](serde::Serialize) and [`serde::Deserialize`](serde::Deserialize).
//!
//! If for some reason you want to implement custom JSON serialization
//! for some types while using `serde_json` for others, you can define
//! a different media type and implement [`Serialize`](crate::content::Serialize)
//! and [`Deserialize`](crate::content::Deserialize) for it.
//! ```
//! use jbhttp::media_type;
//! media_type!(CustomApplicationJson, "application", "json");
//! ```
use crate::content::mediatypes::ApplicationJson;
use crate::content::{Deserialize, SerializationError, Serialize};

impl<T> Serialize<ApplicationJson> for T
where
    T: serde::Serialize,
{
    fn serialize(self) -> Result<Vec<u8>, SerializationError> {
        match serde_json::to_vec(&self) {
            Ok(bytes) => Ok(bytes),
            Err(e) => Err(SerializationError::new(&e.to_string())),
        }
    }
}

impl<T> Deserialize<T> for ApplicationJson
where
    T: serde::de::DeserializeOwned,
{
    fn deserialize(bytes: Vec<u8>) -> Result<T, SerializationError> {
        match serde_json::from_slice(&bytes[..]) {
            Ok(p) => Ok(p),
            Err(e) => Err(SerializationError::new(&e.to_string())),
        }
    }
}
