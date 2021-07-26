use std::error::Error as StdError;
use std::fmt::Display;
use std::marker::Sync;
use std::os::raw::c_int;
use std::result;

#[derive(Debug, PartialEq)]
pub enum ErrorKind {
    BadOperation = 1,
    InconsistentState = 2,
    IoError = 3,
    JsError = 4,
    LuaError = 5,
    MalformedData = 6,
    UnsupportedFeature = 7,
    WebGlError = 8,
    UnknownError = 9,
}

#[derive(Debug)]
pub struct Error {
    pub kind: ErrorKind,
    pub description: String,
    pub source: Option<Box<dyn StdError>>,
}

impl Error {
    pub fn new(kind: ErrorKind, description: String) -> Error {
        Error {
            kind,
            description,
            source: None,
        }
    }

    pub fn with_source<E: std::error::Error + 'static>(
        kind: ErrorKind,
        description: String,
        source: E,
    ) -> Error {
        Error {
            kind,
            description,
            source: Some(Box::new(source)),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match &self.source {
            Some(err) => write!(f, "{}: {}", self.description, err),
            None => write!(f, "{}", self.description),
        }
    }
}

impl PartialEq for Error {
    fn eq(&self, other: &Error) -> bool {
        self.kind == other.kind && self.description == other.description
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source.as_deref()
    }
}

unsafe impl Send for Error {}
unsafe impl Sync for Error {}

pub type Result<T> = result::Result<T, Error>;

pub trait IntoResult<T> {
    fn into_result<F: FnOnce() -> String>(self, desc_fn: F) -> Result<T>;
}

impl<T> IntoResult<T> for result::Result<T, std::io::Error> {
    fn into_result<F: FnOnce() -> String>(self, desc_fn: F) -> Result<T> {
        self.map_err(|e| Error::with_source(ErrorKind::IoError, desc_fn(), e))
    }
}

impl<T> IntoResult<T> for result::Result<T, prost::DecodeError> {
    fn into_result<F: FnOnce() -> String>(self, desc_fn: F) -> Result<T> {
        self.map_err(|e| {
            Error::with_source(ErrorKind::MalformedData, desc_fn(), e)
        })
    }
}

impl IntoResult<()> for c_int {
    fn into_result<F: FnOnce() -> String>(self, desc_fn: F) -> Result<()> {
        if self == 0 {
            return Ok(());
        }

        use ErrorKind::*;
        let kind = match self {
            1 => BadOperation,
            2 => InconsistentState,
            3 => IoError,
            4 => JsError,
            5 => LuaError,
            6 => MalformedData,
            7 => UnsupportedFeature,
            8 => WebGlError,
            _ => UnknownError,
        };

        Err(Error::new(kind, desc_fn()))
    }
}
