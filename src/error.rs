//! Error definitions

/// Library result type
pub type Result<T> = std::result::Result<T, Error>;
pub type ResponseResult<T> = std::result::Result<T, (axum::http::StatusCode, String)>;

/// Library errors
#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum Error {
    #[error("IO error: {0}")]
    IO(String),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Not implemented: {0}")]
    NotImplemented(String),
    #[error("Goblin error: {0}")]
    Goblin(String),
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

impl From<goblin::error::Error> for Error {
    fn from(err: goblin::error::Error) -> Self {
        Error::Goblin(err.to_string())
    }
}
