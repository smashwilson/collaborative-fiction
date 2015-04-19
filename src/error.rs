//! Error structures.

use std::error::Error;
use std::fmt::{Display, Formatter};

use std::fmt::Error as FmtError;
use iron::error::IronError;
use postgres::Error as PgError;
use hyper::HttpError;
use rustc_serialize::json::DecoderError as JSONDecoderError;
use rustc_serialize::json::EncoderError as JSONEncoderError;

use error::FictError::{Message, Database, Iron, Hyper, JSONDecode, JSONEncode};

/// An Error type that can be used throughout the application. It can provide its own error message,
/// wrap an underlying error, or both.
///
#[derive(Debug)]
pub enum FictError {
    Message(String),
    Database(PgError),
    Iron(IronError),
    Hyper(HttpError),
    JSONDecode(JSONDecoderError),
    JSONEncode(JSONEncoderError),
}

impl Error for FictError {
    fn description(&self) -> &str {
        match *self {
            Message(ref s) => s,
            Database(ref e) => e.description(),
            Iron(ref e) => e.description(),
            Hyper(ref e) => e.description(),
            JSONDecode(ref e) => e.description(),
            JSONEncode(ref e) => e.description(),
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            Message(..) => None,
            Database(ref e) => Some(e),
            Iron(ref e) => Some(e),
            Hyper(ref e) => Some(e),
            JSONDecode(ref e) => Some(e),
            JSONEncode(ref e) => Some(e),
        }
    }
}

impl Display for FictError {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        match *self {
            Message(ref s) => f.write_str(s),
            Database(ref e) => Display::fmt(e, f),
            Iron(ref e) => Display::fmt(e, f),
            Hyper(ref e) => Display::fmt(e, f),
            JSONDecode(ref e) => Display::fmt(e, f),
            JSONEncode(ref e) => Display::fmt(e, f),
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

impl From<HttpError> for FictError {
    fn from(err: HttpError) -> FictError {
        FictError::Hyper(err)
    }
}

impl From<JSONDecoderError> for FictError {
    fn from(err: JSONDecoderError) -> FictError {
        FictError::JSONDecode(err)
    }
}

impl From<JSONEncoderError> for FictError {
    fn from(err: JSONEncoderError) -> FictError {
        FictError::JSONEncode(err)
    }
}

pub type FictResult<T> = Result<T, FictError>;

/// Create a new FictError with the provided message.
pub fn fict_err<S: Into<String>>(msg: S) -> FictError {
    FictError::Message(msg.into())
}

/// Wrap any supported inner error type within a FictError.
pub fn as_fict_err<E: Into<FictError>>(err: E) -> FictError {
    err.into()
}
