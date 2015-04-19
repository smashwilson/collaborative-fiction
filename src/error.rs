//! Error structures.

use std::error::Error;
use std::fmt::{Display, Formatter};

use std::fmt::Error as FmtError;
use iron::error::IronError;
use postgres::Error as PgError;

use error::FictError::{Message, Database, Iron};

/// An Error type that can be used throughout the application. It can provide its own error message,
/// wrap an underlying error, or both.
///
#[derive(Debug)]
pub enum FictError {
    Message(String),
    Database(PgError),
    Iron(IronError)
}

impl Error for FictError {
    fn description(&self) -> &str {
        match *self {
            Message(ref s) => s,
            Database(ref e) => e.description(),
            Iron(ref e) => e.description(),
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            Message(_) => None,
            Database(ref e) => Some(e),
            Iron(ref e) => Some(e),
        }
    }
}

impl Display for FictError {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        match *self {
            Message(ref s) => f.write_str(s),
            Database(ref e) => Display::fmt(e, f),
            Iron(ref e) => Display::fmt(e, f),
        }
    }
}

impl From<PgError> for FictError {
    fn from(err: PgError) -> FictError {
        FictError::Database(err)
    }
}

impl From<IronError> for FictError {
    fn from(err: IronError) -> FictError {
        FictError::Iron(err)
    }
}

/// Create a new FictError with the provided message.
pub fn fict_err<S: Into<String>>(msg: S) -> FictError {
    FictError::Message(msg.into())
}
