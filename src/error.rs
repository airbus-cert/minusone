use std::str::Utf8Error;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum MinusOneErrorKind {
    Parsing,
    InvalidChildIndex,
    Unknown
}

#[derive(Debug)]
pub struct MinusOneError {
    /// Kind of error
    kind: MinusOneErrorKind,
    /// Associated message of the context
    message: String
}

impl MinusOneError {
    /// create a new MinusOne error
    /// # Example
    /// ```
    /// extern crate minusone;
    ///
    /// use minusone::error::{MinusOneError, MinusOneErrorKind};
    /// let error = MinusOneError::new(MinusOneErrorKind::Unknown, "Unknown");
    /// ```
    pub fn new (kind: MinusOneErrorKind, message: &str) -> Self {
        MinusOneError {
            kind,
            message: String::from(message)
        }
    }

    /// Return the kind of error
    ///
    /// # Example
    /// ```
    /// extern crate minusone;
    ///
    /// use minusone::error::{MinusOneError, MinusOneErrorKind};
    /// let error = MinusOneError::new(MinusOneErrorKind::Unknown, "unknown");
    /// assert_eq!(error.kind(), MinusOneErrorKind::Unknown)
    /// ```
    pub fn kind(&self) -> MinusOneErrorKind {
        self.kind
    }
}

#[derive(Debug)]
pub enum Error {
    /// MinusOne error
    MinusOneError(MinusOneError),
    Utf8Error(Utf8Error)
}

impl Error {
    pub fn new(kind: MinusOneErrorKind, message: &str) -> Self {
        Error::MinusOneError(MinusOneError::new(kind, message))
    }

    pub fn invalid_child() -> Self {
        Error::MinusOneError(MinusOneError::new(MinusOneErrorKind::InvalidChildIndex, "A child was expected at this index"))
    }
}

impl From<Utf8Error> for Error {
    fn from(e: Utf8Error) -> Error {
        Error::Utf8Error(e)
    }
}

pub type MinusOneResult<T> = Result<T, Error>;