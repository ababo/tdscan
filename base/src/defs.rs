use std::error::Error as StdError;

#[derive(Debug)]
pub enum ErrorKind {
    IoError,
    MalformedData,
    FeatureNotSupported,
    JsError,
    WebGlError,
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

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match &self.source {
            Some(err) => write!(f, "{}: {}", self.description, err),
            None => write!(f, "{}", self.description),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source.as_deref()
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub trait IntoResult<T> {
    fn res<F: FnOnce() -> String>(self, desc_fn: F) -> Result<T>;
}

impl<T> IntoResult<T> for std::result::Result<T, std::io::Error> {
    fn res<F: FnOnce() -> String>(self, desc_fn: F) -> Result<T> {
        self.map_err(|e| Error::with_source(ErrorKind::IoError, desc_fn(), e))
    }
}

impl<T> IntoResult<T> for std::result::Result<T, prost::DecodeError> {
    fn res<F: FnOnce() -> String>(self, desc_fn: F) -> Result<T> {
        self.map_err(|e| {
            Error::with_source(ErrorKind::MalformedData, desc_fn(), e)
        })
    }
}
