//! Content-Type negotiation and de/serialization.
#![allow(clippy::borrowed_box)]
use std::fmt;
use std::marker::PhantomData;

use crate::handler::{Handler, Res};
use crate::request::{Accept, ContentType, HeaderParseError, Request};
use crate::response::Response;

#[cfg(feature = "json")]
pub mod json;
pub mod mediatypes;

#[derive(Debug)]
pub enum Error {
    Serialization(SerializationError),
    UnsupportedMediaType(Option<String>),
    HeaderParse(HeaderParseError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            Self::Serialization(e) => {
                write!(f, "{}", e)
            }
            Self::UnsupportedMediaType(s) => {
                write!(f, "unsupported content type: {:?}", s)
            }
            Self::HeaderParse(e) => {
                write!(f, "{}", e)
            }
        }
    }
}

impl From<HeaderParseError> for Error {
    fn from(err: HeaderParseError) -> Self {
        Self::HeaderParse(err)
    }
}

/// Add new content-types by implementing this trait.
///
/// # Example
///
/// Using the `media_type!` macro:
/// ```
/// use jbhttp::media_type;
///
/// media_type!(ApplicationMsWord, "application", "msword");
// ```
pub trait MediaType {
    fn mime_type() -> String;
    fn mime_subtype() -> String;
    fn media_type() -> String {
        format!("{}/{}", Self::mime_type(), Self::mime_subtype())
    }
}

pub trait MediaTypeMatch {
    fn matches(&self, mime_type: &str, mime_subtype: &str) -> bool;
}

fn match_media_type<M: MediaTypeMatch, T>(
    media_type: M,
    choices: &[(String, String, T)],
) -> Option<&T> {
    for (mime_type, mime_subtype, item) in choices.iter() {
        if media_type.matches(mime_type, mime_subtype) {
            return Some(item);
        }
    }
    None
}

/// Implement this trait to enable Content-Type based serialization on
/// your types, like `impl Serialize<ApplicationJson> for MyType {..}`
pub trait Serialize<M: MediaType> {
    fn serialize(self) -> Result<Vec<u8>, SerializationError>;
}

/// Implement this trait to enable Content-Type based deserialization on
/// your types, like `impl Deserialize<MyType> for ApplicationJson {..}`
pub trait Deserialize<T> {
    fn deserialize(bytes: Vec<u8>) -> Result<T, SerializationError>;
}

/// De/serialize response payloads based on *Content-Type*/*Accept* headers.
///
/// # Example
/// ```
/// use jbhttp::prelude::*;
/// use jbhttp::content::MediaTypeSerde;
///
/// struct Person {
///     name: String,
/// }
///
/// impl Person {
///     pub fn new(name: &str) -> Self {
///         Self {
///             name: name.to_string(),
///         }
///     }
/// }
///
/// impl Serialize<TextPlain> for Person {
///     fn serialize(self) -> Result<Vec<u8>, SerializationError> {
///         Ok(format!("{}", self.name).into_bytes())
///     }
/// }
///
/// impl Deserialize<Person> for TextPlain {
///     fn deserialize(bytes: Vec<u8>) -> Result<Person, SerializationError> {
///         Ok(Person::new(std::str::from_utf8(&bytes).unwrap()))
///     }
/// }
///
/// fn get_person(req: Request<Person>, _: &mut ()) -> Res<Person, Vec<u8>> {
///     Ok(Response::new(200).with_body(Person::new("John Smith")))
/// }
///
/// let handler = MediaTypeSerde::new(Box::new(get_person))
///     .with_media_type::<TextPlain>(true);
///
/// let mut request = Request::default().with_header("accept", "text/plain");
/// let response = handler.handle(request, &mut ()).unwrap();
/// # assert_eq!(response.status_code, 200);
/// # assert_eq!(
/// #     response.headers().get("Content-Type"),
/// #     Some(&"text/plain".to_string())
/// # );
/// # assert_eq!(response.body, Some(b"John Smith".to_vec()));
/// ```
pub struct MediaTypeSerde<H, I, O>
where
    O: 'static,
    I: 'static,
{
    handler: H,
    serializer: MediaTypeSerializer<H, I, O>,
    deserializer: MediaTypeDeserializer<H, I, O>,
}

impl<H, I, O> MediaTypeSerde<H, I, O>
where
    I: 'static + Sync,
    O: 'static + Sync,
{
    pub fn new(handler: H) -> Self {
        Self {
            handler,
            serializer: MediaTypeSerializer {
                handler: None,
                default_serializer: None,
                serializers: Vec::new(),
                phantom_i: PhantomData,
            },
            deserializer: MediaTypeDeserializer {
                handler: None,
                default_deserializer: None,
                deserializers: Vec::new(),
                phantom_o: PhantomData,
            },
        }
    }
    pub fn with_media_type_serial<M>(mut self, default: bool) -> Self
    where
        M: 'static + MediaType + Send + Sync,
        O: Serialize<M>,
    {
        self.serializer = self.serializer.with_media_type::<M>(default);
        self
    }
    pub fn with_media_type_deserial<M>(mut self, default: bool) -> Self
    where
        M: 'static + MediaType + Send + Sync + Deserialize<I>,
    {
        self.deserializer = self.deserializer.with_media_type::<M>(default);
        self
    }
    pub fn with_media_type<M>(mut self, default: bool) -> Self
    where
        M: 'static + MediaType + Send + Sync + Deserialize<I>,
        O: Serialize<M>,
    {
        self.serializer = self.serializer.with_media_type::<M>(default);
        self.deserializer = self.deserializer.with_media_type::<M>(default);
        self
    }
}

impl<H, I, O, E, C> Handler<Vec<u8>, Vec<u8>, E, C> for MediaTypeSerde<H, I, O>
where
    H: Handler<I, O, E, C>,
    I: 'static + Sync,
    O: 'static + Sync,
    E: 'static + Sync,
{
    fn handle(&self, request: Request<Vec<u8>>, context: &mut C) -> Res<Vec<u8>, E> {
        let accept = match request.accept() {
            Ok(accept) => accept,
            _ => return Err(Response::new(406)),
        };
        // Check if we can provide requested type form Accept *first* to avoid side effects on
        // a request that would ultimately return 406
        if self.serializer.get_serializer(&accept).is_none() {
            return Err(Response::new(406));
        }
        let request = match self.deserializer.deserialize(request) {
            Ok(request) => request,
            Err(Error::Serialization(_)) => return Err(Response::new(400)),
            Err(Error::UnsupportedMediaType(_)) => return Err(Response::new(415)),
            Err(Error::HeaderParse(_)) => return Err(Response::new(400)),
        };
        match self.handler.handle(request, context) {
            Ok(response) => match self.serializer.serialize(&accept, response) {
                Ok(response) => Ok(response),
                Err(Error::Serialization(_)) => Err(Response::new(500)),
                Err(Error::UnsupportedMediaType(_)) => Err(Response::new(406)),
                Err(Error::HeaderParse(_)) => Err(Response::new(400)),
            },
            Err(response) => Err(response),
        }
    }
}

/// Serialize response payloads based on the *Accept* header of requests.
///
/// Converts `Response<T>` to `Response<Vec<u8>>` for types that implementing
/// [`Serialize`](crate::content::Serialize)`<MediaType> for T`.
pub struct MediaTypeSerializer<H, I, O>
where
    I: 'static,
{
    handler: Option<H>,
    default_serializer: Option<Box<dyn ResponseSerializer<O>>>,
    // These are all SingleMediaTypeSerializer's, but since they have different
    // types for M, I still need boxdyns
    serializers: Vec<(String, String, Box<dyn ResponseSerializer<O>>)>,
    phantom_i: PhantomData<&'static I>,
}

impl<H, I, O> MediaTypeSerializer<H, I, O>
where
    I: 'static,
    O: 'static + Sync,
{
    pub fn new(handler: H) -> Self {
        Self {
            handler: Some(handler),
            serializers: Vec::new(),
            default_serializer: None,
            phantom_i: PhantomData,
        }
    }
    pub fn with_media_type<M>(mut self, default: bool) -> Self
    where
        M: 'static + MediaType + Send + Sync,
        O: Serialize<M>,
    {
        let serializer: SingleMediaTypeSerializer<M, O> = SingleMediaTypeSerializer::new();
        self.serializers
            .push((M::mime_type(), M::mime_subtype(), Box::new(serializer)));
        if default {
            let serializer: SingleMediaTypeSerializer<M, O> = SingleMediaTypeSerializer::new();
            self.default_serializer = Some(Box::new(serializer));
        }
        self
    }
    fn get_serializer<'a>(
        &'a self,
        accept: &Option<Accept>,
    ) -> Option<&Box<dyn ResponseSerializer<O>>> {
        match accept {
            Some(accept) => {
                for pref in accept.iter() {
                    if let Some(serializer) = match_media_type(pref, &self.serializers) {
                        return Some(serializer);
                    }
                }
                None
            }
            None => self.default_serializer.as_ref(),
        }
    }
    fn serialize(
        &self,
        accept: &Option<Accept>,
        response: Response<O>,
    ) -> Result<Response<Vec<u8>>, Error> {
        match self.get_serializer(accept) {
            Some(serializer) => match serializer.serialize(response) {
                Ok(response) => Ok(response),
                Err(e) => Err(Error::Serialization(e)),
            },
            None => Err(Error::UnsupportedMediaType(None)),
        }
    }
}

impl<H, I, O, E, C> Handler<I, Vec<u8>, E, C> for MediaTypeSerializer<H, I, O>
where
    H: Handler<I, O, E, C>,
    I: 'static + Sync,
    O: 'static + Sync,
    E: 'static + Sync,
{
    fn handle(&self, request: Request<I>, context: &mut C) -> Res<Vec<u8>, E> {
        let accept = match request.accept() {
            Ok(accept) => accept,
            _ => return Err(Response::new(406)),
        };
        if self.get_serializer(&accept).is_none() {
            return Err(Response::new(406));
        }
        match self.handler.as_ref().unwrap().handle(request, context) {
            Ok(response) => match self.serialize(&accept, response) {
                Ok(response) => Ok(response),
                Err(Error::Serialization(_)) => Err(Response::new(500)),
                Err(Error::UnsupportedMediaType(_)) => Err(Response::new(406)),
                Err(Error::HeaderParse(_)) => Err(Response::new(400)),
            },
            Err(response) => Err(response),
        }
    }
}

/// Serialize error response payloads based on the *Accept* header of requests.
///
/// Converts `Response<T>` to `Response<Vec<u8>>` for types that implementing
/// [`Serialize`](crate::content::Serialize)`<MediaType> for T`.
pub struct MediaTypeErrorSerializer<H, I, E>
where
    I: 'static,
{
    handler: Option<H>,
    default_serializer: Option<Box<dyn ResponseSerializer<E>>>,
    // These are all SingleMediaTypeSerializer's, but since they have different
    // types for M, I still need boxdyns
    serializers: Vec<(String, String, Box<dyn ResponseSerializer<E>>)>,
    phantom_i: PhantomData<&'static I>,
}

impl<H, I, E> MediaTypeErrorSerializer<H, I, E>
where
    I: 'static,
    E: 'static + Sync,
{
    pub fn new(handler: H) -> Self {
        Self {
            handler: Some(handler),
            serializers: Vec::new(),
            default_serializer: None,
            phantom_i: PhantomData,
        }
    }
    pub fn with_media_type<M>(mut self, default: bool) -> Self
    where
        M: 'static + MediaType + Send + Sync,
        E: Serialize<M>,
    {
        let serializer: SingleMediaTypeSerializer<M, E> = SingleMediaTypeSerializer::new();
        self.serializers
            .push((M::mime_type(), M::mime_subtype(), Box::new(serializer)));
        if default {
            let serializer: SingleMediaTypeSerializer<M, E> = SingleMediaTypeSerializer::new();
            self.default_serializer = Some(Box::new(serializer));
        }
        self
    }
    fn get_serializer<'a>(
        &'a self,
        accept: &Option<Accept>,
    ) -> Option<&Box<dyn ResponseSerializer<E>>> {
        match accept {
            Some(accept) => {
                for pref in accept.iter() {
                    if let Some(serializer) = match_media_type(pref, &self.serializers) {
                        return Some(serializer);
                    }
                }
                None
            }
            None => self.default_serializer.as_ref(),
        }
    }
    fn serialize(
        &self,
        accept: &Option<Accept>,
        response: Response<E>,
    ) -> Result<Response<Vec<u8>>, Error> {
        match self.get_serializer(accept) {
            Some(serializer) => match serializer.serialize(response) {
                Ok(response) => Ok(response),
                Err(e) => Err(Error::Serialization(e)),
            },
            None => Err(Error::UnsupportedMediaType(None)),
        }
    }
}

impl<H, I, O, E, C> Handler<I, O, Vec<u8>, C> for MediaTypeErrorSerializer<H, I, E>
where
    H: Handler<I, O, E, C>,
    I: 'static + Sync,
    O: 'static + Sync,
    E: 'static + Sync,
{
    fn handle(&self, request: Request<I>, context: &mut C) -> Res<O, Vec<u8>> {
        let accept = match request.accept() {
            Ok(accept) => accept,
            _ => return Err(Response::new(406)),
        };
        if self.get_serializer(&accept).is_none() {
            return Err(Response::new(406));
        }
        match self.handler.as_ref().unwrap().handle(request, context) {
            Err(response) => match self.serialize(&accept, response) {
                Ok(response) => Err(response),
                Err(Error::Serialization(_)) => Err(Response::new(500)),
                Err(Error::UnsupportedMediaType(_)) => Err(Response::new(406)),
                Err(Error::HeaderParse(_)) => Err(Response::new(400)),
            },
            Ok(response) => Ok(response),
        }
    }
}

trait ResponseSerializer<O>: Send + Sync {
    fn serialize(&self, r: Response<O>) -> Result<Response<Vec<u8>>, SerializationError>;
}

// this is a hack to attach a carry around the MediaType type parameter
struct SingleMediaTypeSerializer<M, O>
where
    M: 'static + MediaType + Sync + Send,
    O: 'static + Serialize<M>,
{
    phantom_m: PhantomData<&'static M>,
    phantom_o: PhantomData<&'static O>,
}

impl<M, O> SingleMediaTypeSerializer<M, O>
where
    M: MediaType + Send + Sync,
    O: Serialize<M>,
{
    fn new() -> Self {
        Self {
            phantom_m: PhantomData,
            phantom_o: PhantomData,
        }
    }
}

impl<M, O> ResponseSerializer<O> for SingleMediaTypeSerializer<M, O>
where
    M: MediaType + Send + Sync,
    O: Serialize<M> + Sync,
{
    fn serialize(
        &self,
        mut response: Response<O>,
    ) -> Result<Response<Vec<u8>>, SerializationError> {
        let body = std::mem::replace(&mut response.body, None);
        if let Some(body) = body {
            Ok(response
                .into_raw()
                .with_body(body.serialize()?)
                .with_header("Content-Type", &M::media_type()))
        } else {
            Ok(response.into_raw())
        }
    }
}

/// Deserialize request payloads based on their  *Content-Type* headers.
///
/// Converts `Request<Vec<u8>>` to `Request<T>` for types T implementing
/// [`Deserialize`](crate::content::Serialize)`<T> for MediaType`.
pub struct MediaTypeDeserializer<H, I, O>
where
    O: 'static,
{
    handler: Option<H>,
    default_deserializer: Option<Box<dyn RequestDeserializer<I>>>,
    // These are all SingleMediaTypeDeserializer's, but since they have different
    // types for M, I still need boxdyns
    deserializers: Vec<(String, String, Box<dyn RequestDeserializer<I>>)>,
    phantom_o: PhantomData<&'static O>,
}

impl<H, I, O> MediaTypeDeserializer<H, I, O>
where
    I: 'static + Sync,
    O: 'static,
{
    pub fn new(handler: H) -> Self {
        Self {
            handler: Some(handler),
            default_deserializer: None,
            deserializers: Vec::new(),
            phantom_o: PhantomData,
        }
    }

    pub fn with_media_type<M>(mut self, default: bool) -> Self
    where
        M: 'static + MediaType + Send + Sync + Deserialize<I>,
    {
        let deserializer: SingleMediaTypeDeserializer<M, I> = SingleMediaTypeDeserializer::new();
        self.deserializers
            .push((M::mime_type(), M::mime_subtype(), Box::new(deserializer)));
        if default {
            let deserializer: SingleMediaTypeDeserializer<M, I> =
                SingleMediaTypeDeserializer::new();
            self.default_deserializer = Some(Box::new(deserializer));
        }
        self
    }
    fn get_deserializer<'a>(
        &'a self,
        content_type: &Option<ContentType>,
    ) -> Option<&Box<dyn RequestDeserializer<I>>> {
        match content_type {
            Some(content_type) => match_media_type(content_type, &self.deserializers),
            None => self.default_deserializer.as_ref(),
        }
    }
    fn deserialize(&self, request: Request<Vec<u8>>) -> Result<Request<I>, Error> {
        let content_type = request.content_type()?;
        match self.get_deserializer(&content_type) {
            Some(deserializer) => match deserializer.deserialize(request) {
                Ok(request) => Ok(request),
                Err(e) => Err(Error::Serialization(e)),
            },
            None => Err(Error::UnsupportedMediaType(
                content_type.map(|c| format!("{}/{}", c.mime_type, c.mime_subtype)),
            )),
        }
    }
}

impl<H, I, O, E, C> Handler<Vec<u8>, O, E, C> for MediaTypeDeserializer<H, I, O>
where
    H: Handler<I, O, E, C>,
    I: 'static + Sync,
    O: 'static + Sync,
    E: 'static + Sync,
{
    fn handle(&self, request: Request<Vec<u8>>, context: &mut C) -> Res<O, E> {
        let request = match self.deserialize(request) {
            Ok(request) => request,
            Err(Error::Serialization(_)) => return Err(Response::new(400)),
            Err(Error::UnsupportedMediaType(_)) => return Err(Response::new(415)),
            Err(Error::HeaderParse(_)) => return Err(Response::new(400)),
        };
        self.handler.as_ref().unwrap().handle(request, context)
    }
}

trait RequestDeserializer<I>: Send + Sync {
    fn deserialize(&self, r: Request<Vec<u8>>) -> Result<Request<I>, SerializationError>;
}

// this is a hack to attach a MediaType parameter to a Handler
struct SingleMediaTypeDeserializer<M, I>
where
    I: 'static,
    M: 'static + MediaType + Send + Sync + Deserialize<I>,
{
    phantom_m: PhantomData<&'static M>,
    phantom_i: PhantomData<&'static I>,
}

impl<M, I> SingleMediaTypeDeserializer<M, I>
where
    M: 'static + MediaType + Send + Sync + Deserialize<I>,
{
    fn new() -> Self {
        Self {
            phantom_m: PhantomData,
            phantom_i: PhantomData,
        }
    }
}

impl<M, I> RequestDeserializer<I> for SingleMediaTypeDeserializer<M, I>
where
    M: 'static + MediaType + Send + Sync + Deserialize<I>,
    I: Sync,
{
    fn deserialize(&self, mut request: Request<Vec<u8>>) -> Result<Request<I>, SerializationError> {
        let body = std::mem::replace(&mut request.body, None);
        match body {
            Some(body) => {
                let body = M::deserialize(body)?;
                let mut request = request.into_type();
                request.body = Some(body);
                Ok(request)
            }
            None => Ok(request.into_type()),
        }
    }
}

#[derive(Debug)]
pub struct SerializationError {
    reason: String,
}

impl SerializationError {
    pub fn new(reason: &str) -> Self {
        Self {
            reason: reason.to_string(),
        }
    }
}

impl fmt::Display for SerializationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "serialization error: {}", self.reason)
    }
}

/// Define a new unit struct implementing MediaType.
#[macro_export]
macro_rules! media_type {
    ( $i:ident, $t:literal, $s:literal ) => {
        pub struct $i;

        impl $crate::content::MediaType for $i {
            fn mime_type() -> String {
                $t.to_string()
            }
            fn mime_subtype() -> String {
                $s.to_string()
            }
        }
    };
}
