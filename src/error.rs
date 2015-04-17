//! Error structures.

use std::error::Error;
use std::fmt::{Display, Formatter};

use iron::error::IronError;
use postgres::error::Error as PgError;

/// An Error type that can be used throughout the application. It can provide its own error message,
/// wrap an underlying error, or both.
///
#[derive(Debug)]
pub struct FictError {

    /// Brief description of what went wrong.
    message: Option<String>,

    /// Underlying Error that caused this one to arise.
    cause: Option<Box<Error>>,
}

impl FictError {
    /// Create a new FictError that contains a custom message.
    pub fn with_message(message: String) -> FictError {
        FictError{
            message: Some(message),
            cause: None,
        }
    }

    /// Create a new FictError that wraps an underlying error.
    pub fn caused_by(cause: Error) -> FictError {
        FictError{
            message: None,
            cause: Some(Box::new(cause)),
        }
    }

    /// Create a new FictError that wraps an underlying error, but provides a new explanation of
    /// what happened.
    pub fn new(message: String, cause: Error) -> FictError {
        FictError{
            message: Some(message),
            cause: Some(Box::new(cause)),
        }
    }
}

impl Error for FictError {
    fn description(&self) -> &str {
        self.message.unwrap_or_else(|| {
            match self.cause {
                Some(c) => c.description(),
                None => "Something went wrong.",
            }
        })
    }

    fn cause(&self) -> Option<&Error> {
        self.cause
    }
}

impl Display for FictError {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        let mut started = match self.message {
            Some(m) => {
                try!(f.write_str(m));
                false
            },
            None => false,
        };

        match self.cause {
            Some(c) => {
                try!(Display::fmt(c, f));
                Ok(())
            },
            None => Ok(()),
        }
    }
}

impl From<PgError> for FictError {
    fn from(err: PgError) -> FictError {
        FictError::caused_by(err)
    }
}

impl From<IronError> for FictError {
    fn from(err: IronError) -> FictError {
        FictError::caused_by(err)
    }
}
