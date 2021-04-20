#[derive(Debug)]
pub enum Error {
    IoError(std::io::Error),
    MalformedData(String),
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error::IoError(err)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
