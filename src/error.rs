//! Error structures.

use std::error::Error;
use std::fmt::{Display, Formatter};

use std;
use std::fmt::Error as FmtError;
use log;
use postgres;
use r2d2;
use hyper;
use iron;
use rustc_serialize;

use error::FictError::{Message, Cause};

/// An Error type that can be used throughout the application. It can provide its own error message
/// or wrap an underlying error of a different type.
///
#[derive(Debug)]
pub enum FictError {
    Message(String),
    Cause(Box<Error + Send>),
}

impl FictError {

    /// Consume a FictError to produce an IronError that wraps it and produces the appropriate HTTP
    /// status code.
    pub fn iron(self, status: iron::status::Status) -> iron::IronError {
        iron::IronError::new(self, status)
    }

}

impl Error for FictError {
    fn description(&self) -> &str {
        match *self {
            Message(ref s) => s,
            Cause(ref e) => e.description(),
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            Message(..) => None,
            Cause(ref e) => Some(&**e),
        }
    }
}

impl Display for FictError {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        match *self {
            Message(ref s) => f.write_str(s),
            Cause(ref e) => Display::fmt(e, f),
        }
    }
}

impl From<FmtError> for FictError {
    fn from(err: FmtError) -> FictError {
        FictError::Message(format!("{}", err))
    }
}

trait NonFictError: Error {}
impl NonFictError for std::io::Error {}
impl NonFictError for std::env::VarError {}
impl NonFictError for log::SetLoggerError {}
impl NonFictError for iron::IronError {}
impl NonFictError for postgres::error::Error {}
impl NonFictError for postgres::error::ConnectError {}
impl NonFictError for r2d2::InitializationError {}
impl NonFictError for r2d2::GetTimeout {}
impl NonFictError for hyper::Error {}
impl NonFictError for rustc_serialize::json::DecoderError {}
impl NonFictError for rustc_serialize::json::EncoderError {}
impl NonFictError for rustc_serialize::json::ParserError {}

impl<E: NonFictError + Send + 'static> From<E> for FictError {
    fn from(err: E) -> FictError {
        FictError::Cause(Box::new(err))
    }
}

/// Convenient type alias for a Result that uses FictError as its error type.
pub type FictResult<T> = Result<T, FictError>;

/// Create a new FictError with the provided message.
pub fn fict_err<S: Into<String>>(msg: S) -> FictError {
    FictError::Message(msg.into())
}

/// Wrap any supported inner error type within a FictError.
pub fn as_fict_err<E: Into<FictError>>(err: E) -> FictError {
    err.into()
}
