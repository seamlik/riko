//! Wrappers for objects being returned to the target side.

use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize)]
pub struct Returned<T> {
    pub error: Option<Error>,
    pub value: Option<T>,
}

impl<T, E: std::error::Error> From<Result<Option<T>, E>> for Returned<T> {
    fn from(src: Result<Option<T>, E>) -> Self {
        match src {
            Ok(value) => Returned { error: None, value },
            Err(err) => Returned {
                error: Some(err.into()),
                value: None,
            },
        }
    }
}

impl<T, E: std::error::Error> From<Result<T, E>> for Returned<T> {
    fn from(src: Result<T, E>) -> Self {
        match src {
            Ok(value) => Returned {
                error: None,
                value: Some(value),
            },
            Err(err) => Returned {
                error: Some(err.into()),
                value: None,
            },
        }
    }
}

impl<T> From<Option<T>> for Returned<T> {
    fn from(value: Option<T>) -> Self {
        Returned { error: None, value }
    }
}

impl<T> From<T> for Returned<T> {
    fn from(src: T) -> Self {
        Returned {
            error: None,
            value: Some(src),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Error {
    pub debug: String,
    pub message: String,
}

impl<T: std::error::Error> From<T> for Error {
    fn from(src: T) -> Self {
        Error {
            debug: format!("{:?}", src),
            message: src.to_string(),
        }
    }
}
