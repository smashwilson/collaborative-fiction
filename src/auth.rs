//! Authentication middleware.

use std::error::Error;
use std::fmt;

use iron::{Request, IronResult, IronError, BeforeMiddleware};
use iron::status;
use iron::typemap::Key;
use hyper::header::{Authorization, Basic};
use persistent::Write;
use plugin::Extensible;

use model::{Database, Session, User};

#[derive(Debug)]
struct AuthError;

impl AuthError {

    fn iron() -> IronError {
        IronError::new(AuthError, status::Unauthorized)
    }

}

impl Error for AuthError {

    fn description(&self) -> &str {
        "Authentication is required to access this endpoint."
    }
}

impl fmt::Display for AuthError {

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("AuthError")
    }

}

/// An authenticated user.
pub struct AuthUser;

impl Key for AuthUser { type Value = User; }

/// Link this middleware before a handler to ensure that an incoming request is accompanied by
/// a valid API key. If so, the Session will be added to the request. Otherwise, a 401 response
/// will be returned.
pub struct RequireUser;

impl BeforeMiddleware for RequireUser {

    fn before(&self, req: &mut Request) -> IronResult<()> {
        let auth_opt = req.headers.get::<Authorization<Basic>>().cloned();

        match auth_opt {
            Some(auth) => {
                let mutex = req.extensions().get::<Write<Database>>()
                    .cloned()
                    .expect("No database connection available");
                let pool = mutex.lock().unwrap();
                let conn = pool.get().unwrap();

                let password = try!(auth.password.clone().ok_or_else(|| {
                    warn!("No password present in Authorization header.");
                    AuthError::iron()
                }));

                let token = try!(password.parse::<i64>().map_err(|e| {
                    warn!("Unable to parse token id from a request: [{}]", e);
                    AuthError::iron()
                }));

                let session_opt = try!(Session::validate(&*conn, token).map_err(|e| {
                    error!("Unable to query the database for a session: [{}]", e);
                    AuthError::iron()
                }));

                match session_opt {
                    Some(session) => {
                        let user = try!(session.user(&*conn).map_err(|e| {
                            error!("Unable to query the database for a user: [{}]", e);
                            AuthError::iron()
                        }));

                        req.extensions_mut().insert::<AuthUser>(user);

                        Ok(())
                    },
                    None => Err(AuthError::iron()),
                }
            },
            None => Err(AuthError::iron()),
        }
    }

}
