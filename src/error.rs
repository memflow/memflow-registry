//! Error definitions

/// Library result type
pub type Result<T> = std::result::Result<T, Error>;
pub type ResponseResult<T> = std::result::Result<T, (axum::http::StatusCode, String)>;

/// Library errors
#[derive(thiserror::Error, Debug, Clone, PartialEq)]
#[allow(unused)]
pub enum Error {
    // Basic errors
    #[error("Unknown error: {0}")]
    Unknown(String),
    #[error("IO error: {0}")]
    IO(String),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Already exists: {0}")]
    AlreadyExists(String),
    #[error("Not implemented: {0}")]
    NotImplemented(String),

    // External crate error forwards
    #[error("Memflow error: {0}")]
    Memflow(String),
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("Signature error: {0}")]
    Signature(String),

    #[error("{0} {1}")]
    Wrapped(String, Box<Error>),
}

impl Error {
    #[inline]
    pub fn context(self, context: &str) -> Self {
        Self::Wrapped(context.to_string(), Box::new(self))
    }
}

pub trait ResultExt<T> {
    fn context(self, context: &str) -> Result<T>;
}

impl<T, E> ResultExt<T> for std::result::Result<T, E>
where
    E: Into<Error>,
{
    #[inline]
    fn context(self, context: &str) -> Result<T> {
        self.map_err(|e| e.into().context(context))
    }
}

impl From<&str> for Error {
    fn from(err: &str) -> Self {
        Error::Unknown(err.to_owned())
    }
}

impl From<std::convert::Infallible> for Error {
    fn from(err: std::convert::Infallible) -> Self {
        Error::IO(err.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::IO(err.to_string())
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(value: std::str::Utf8Error) -> Self {
        Error::Parse(format!("Unable to parse utf8: {}", value))
    }
}

impl From<memflow::error::Error> for Error {
    fn from(err: memflow::error::Error) -> Self {
        Error::Memflow(err.to_string())
    }
}

impl From<k256::ecdsa::Error> for Error {
    fn from(err: k256::ecdsa::Error) -> Self {
        Error::Signature(err.to_string())
    }
}

impl From<std::num::ParseIntError> for Error {
    fn from(err: std::num::ParseIntError) -> Self {
        Error::Parse(err.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Parse(err.to_string())
    }
}
