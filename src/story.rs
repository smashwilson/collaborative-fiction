//! Story routes.
//!
//! * `POST /story/:id/lock` - Acquire a lock on the story :id.

use iron::{Request, Response, IronResult, IronError, Chain};
use iron::status;
use router::{Router, Params};
use persistent::{Read, Write};
use plugin::Extensible;
use rustc_serialize::json;

use model::{Database, Story};
use auth::{AuthUser, RequireUser};
use error::FictError::{AlreadyLocked, NotFound};

#[derive(Debug, Clone, RustcEncodable)]
struct LockGranted<'a> {
    state: &'a str,
    expires: &'a str
}

#[derive(Debug, Clone, RustcEncodable)]
struct PriorSnippet<'a> {
    content: &'a str
}

#[derive(Debug, Clone, RustcEncodable)]
struct LockGrantedResponse<'a> {
    lock: LockGranted<'a>,
    snippet: PriorSnippet<'a>
}

#[derive(Debug, Clone, RustcEncodable)]
struct LockDenied<'a> {
    state: &'a str,
    owner: &'a str,
    expires: &'a str
}

#[derive(Debug, Clone, RustcEncodable)]
struct LockDeniedResponse<'a> {
    lock: LockDenied<'a>
}

pub fn acquire_lock(req: &mut Request) -> IronResult<Response> {
    let applicant = req.extensions().get::<AuthUser>().cloned()
        .expect("No authenticated user");

    let params = req.extensions().get::<Router>()
        .expect("No route parameters");
    let story_id = match params["id"].parse::<i64>() {
        Ok(i) => i,
        Err(_) => return Ok(Response::with(("id must be numeric", status::BadRequest)))
    };

    let mutex = req.extensions().get::<Write<Database>>()
        .cloned()
        .expect("No database connection available");
    let pool = mutex.lock().unwrap();
    let conn = pool.get().unwrap();

    match Story::locked_for_write(&*conn, story_id, &applicant) {
        Ok(story) => {
            let r = LockGrantedResponse {
                lock: LockGranted{
                    state: "granted",
                    expires: "TODO",
                },
                snippet: PriorSnippet{
                    content: "TODO"
                }
            };

            let encoded = json::encode(&r)
                .expect("Unable to encode response JSON");

            Ok(Response::with((status::Ok, encoded)))
        },
        Err(AlreadyLocked { username, expiration }) => {
            let r = LockDeniedResponse {
                lock: LockDenied{
                    state: "denied",
                    owner: &username,
                    expires: "TODO"
                }
            };

            let encoded = json::encode(&r)
                .expect("Unable to encode response JSON");

            Ok(Response::with((status::Conflict, encoded)))
        },
        Err(e) => {
            warn!("Unable to lock story for write: {:?}", e);
            Err(e.iron(status::InternalServerError))
        }
    }
}

/// Register `/story` routes and their required middleware.
pub fn route(router: &mut Router) {
    let mut acquire_lock_chain = Chain::new(acquire_lock);
    acquire_lock_chain.link_before(RequireUser);
    router.post("/story/:id/lock", acquire_lock_chain);
}
