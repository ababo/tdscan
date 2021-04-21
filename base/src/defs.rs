pub type InnerError = Box<dyn std::error::Error>;

pub enum ErrorKind {
    IoError,
    MalformedData,
    FeatureNotSupported,
}

pub struct Error {
    pub kind: ErrorKind,
    pub description: String,
    pub inner_error: Option<InnerError>,
}

impl Error {
    pub fn new(kind: ErrorKind, description: String) -> Error {
        Error {
            kind,
            description,
            inner_error: None,
        }
    }

    pub fn with_error(
        kind: ErrorKind,
        description: String,
        inner_error: InnerError,
    ) -> Error {
        Error {
            kind,
            description,
            inner_error: Some(inner_error),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match &self.inner_error {
            Some(err) => write!(f, "{}: {}", self.description, err),
            None => write!(f, "{}", self.description),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub trait IntoResult<T> {
    fn res(self, description: String) -> Result<T>;
}

impl<T> IntoResult<T> for std::result::Result<T, std::io::Error> {
    fn res(self, description: String) -> Result<T> {
        self.map_err(|e| {
            Error::with_error(ErrorKind::IoError, description, Box::new(e))
        })
    }
}
