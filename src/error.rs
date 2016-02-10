//! Error structures.

use std::error::Error;
use std::fmt::{Display, Formatter};

use std;
use std::fmt::Error as FmtError;
use log;
use postgres;
use r2d2;
use hyper;
use iron::status::{self, Status};
use iron::{IronError, IronResult};
use rustc_serialize;
use chrono::{DateTime, UTC};

use error::FictError::{Message, Cause, NotFound, Unlocked, Cooldown, AlreadyLocked};

/// An Error type that can be used throughout the application. It can provide its own error message
/// or wrap an underlying error of a different type.
///
#[derive(Debug)]
pub enum FictError {
    Message(String),
    Cause(Box<Error + Send>),
    NotFound,
    Unlocked,
    Cooldown,
    AlreadyLocked { username: String, expiration: DateTime<UTC> }
}

impl FictError {
    /// HTTP status code that this error will generally result in.
    pub fn preferred_status(&self) -> Status {
        match *self {
            NotFound => status::NotFound,
            Unlocked | Cooldown | AlreadyLocked {..} => status::Unauthorized,
            _ => status::InternalServerError
        }
    }

    /// Consume the error to produce an IronError with a custom HTTP status code.
    pub fn to_iron_error(self, status: Status) -> IronError {
        IronError::new(self, status)
    }
}

impl Error for FictError {
    fn description(&self) -> &str {
        match *self {
            Message(ref s) => s,
            Cause(ref e) => e.description(),
            NotFound => "Resource not found",
            Unlocked => "Resource not locked",
            Cooldown => "Cooldown",
            AlreadyLocked {..} => "Unable to acquire a lock"
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            Cause(ref e) => Some(&**e),
            _ => None
        }
    }
}

impl Display for FictError {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        match *self {
            Cause(ref e) => Display::fmt(e, f),
            _ => f.write_str(self.description()),
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
impl NonFictError for IronError {}
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

pub trait IntoIronResult<T> {
    fn iron_with_status(self, status: Status) -> IronResult<T>;

    fn iron(self) -> IronResult<T>;
}

impl <T> IntoIronResult<T> for FictResult<T> {
    fn iron_with_status(self, status: Status) -> IronResult<T> {
        self.map_err(|err| {
            if status.is_server_error() {
                error!("{} server error: {:?}", status, err);
            } else {
                info!("{} client error: {:?}", status, err);
            }

            err.to_iron_error(status)
        })
    }

    fn iron(self) -> IronResult<T> {
        self.map_err(|err| {
            let st = err.preferred_status();

            if st.is_server_error() {
                error!("{} server error: {:?}", st, err);
            } else {
                info!("{} client error: {:?}", st, err);
            }

            err.to_iron_error(st)
        })
    }
}

/// Create a new FictError with the provided message.
pub fn fict_err<S: Into<String>>(msg: S) -> FictError {
    FictError::Message(msg.into())
}

/// Wrap any supported inner error type within a FictError.
pub fn as_fict_err<E: Into<FictError>>(err: E) -> FictError {
    err.into()
}
