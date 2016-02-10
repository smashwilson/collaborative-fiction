//! Story routes.
//!
//! * `POST /stories/:id/lock` - Acquire a lock on the story :id.

use iron::{Request, Response, IronResult, Chain};
use iron::status;
use router::Router;
use persistent::Write;
use plugin::Extensible;
use rustc_serialize::json;

use model::{Database, Story, ContributionAttempt, Snippet};
use auth::{AuthUser, RequireUser};
use error::IntoIronResult;
use error::FictError::{Cooldown, AlreadyLocked};

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
struct LockConflict<'a> {
    state: &'a str,
    reason: &'a str,
    owner: &'a str,
    expires: &'a str
}

#[derive(Debug, Clone, RustcEncodable)]
struct LockConflictResponse<'a> {
    lock: LockConflict<'a>
}

#[derive(Debug, Clone, RustcEncodable)]
struct LockCooldown<'a> {
    state: &'a str,
    reason: &'a str
}

#[derive(Debug, Clone, RustcEncodable)]
struct LockCooldownResponse<'a> {
    lock: LockCooldown<'a>
}

/// Consistent DateTime format to be used throughout the API: `Fri, 10 May 2015 17:58:28 +0000`
const TIMESTAMP_FORMAT: &'static str = "%a, %d %b %Y %T %z";

/// `POST /stories/:id/lock` to acquire a lock on an existing story and retrieve the most recent
/// contributed Snippet.
pub fn acquire_lock(req: &mut Request) -> IronResult<Response> {
    let applicant = req.extensions().get::<AuthUser>().cloned()
        .expect("No authenticated user");

    let params = req.extensions().get::<Router>()
        .expect("No route parameters");
    let story_id = match params["id"].parse::<i64>() {
        Ok(i) => i,
        Err(_) => return Ok(Response::with(("id must be numeric", status::BadRequest)))
    };

    debug!("POST /stories/{}/lock [{}]", story_id, applicant.name);

    let mutex = req.extensions().get::<Write<Database>>()
        .cloned()
        .expect("No database connection available");
    let pool = mutex.lock().unwrap();
    let ref conn = *pool.get().unwrap();

    match Story::locked_for_write(conn, story_id, &applicant, true) {
        Ok(story) => {
            debug!(".. Lock granted until {:?}.", story.lock_expiration);

            try!(ContributionAttempt::record(conn, &story, &applicant).iron());

            let formatted_expiration = story.lock_expiration.map(|exp| {
                format!("{}", exp.format(TIMESTAMP_FORMAT))
            }).expect("Story missing expiration date");

            let snippet = try!(Snippet::most_recent(conn, &story).iron());

            let r = LockGrantedResponse {
                lock: LockGranted{
                    state: "granted",
                    expires: &formatted_expiration
                },
                snippet: PriorSnippet{
                    content: &snippet.content
                }
            };

            let encoded = json::encode(&r)
                .expect("Unable to encode response JSON");

            Ok(Response::with((status::Ok, encoded)))
        },
        Err(AlreadyLocked { username, expiration }) => {
            debug!(".. Lock denied: already held by {}.", username);

            let r = LockConflictResponse {
                lock: LockConflict{
                    state: "denied",
                    reason: "conflict",
                    owner: &username,
                    expires: &format!("{}", expiration.format(TIMESTAMP_FORMAT))
                }
            };

            let encoded = json::encode(&r)
                .expect("Unable to encode response JSON");

            Ok(Response::with((status::Conflict, encoded)))
        },
        Err(Cooldown) => {
            debug!(".. Lock denied: last contribution too recent.");

            let r = LockCooldownResponse {
                lock: LockCooldown{
                    state: "denied",
                    reason: "cooldown"
                }
            };

            let encoded = json::encode(&r)
                .expect("Unable to encode response JSON");

            Ok(Response::with((status::Conflict, encoded)))
        },
        Err(e) => {
            error!("Unable to lock story for write: {:?}", e);
            Err(e.to_iron_error(status::InternalServerError))
        }
    }
}

/// `DELETE /stories/:id/lock` to revoke a lock on a story that you currently hold.
pub fn revoke_lock(req: &mut Request) -> IronResult<Response> {
    let user = req.extensions().get::<AuthUser>().cloned()
        .expect("No authenticated user");

    let params = req.extensions().get::<Router>()
        .expect("No route parameters");
    let story_id = match params["id"].parse::<i64>() {
        Ok(i) => i,
        Err(_) => return Ok(Response::with(("id must be numeric", status::BadRequest)))
    };

    debug!("DELETE /stories/{}/lock [{}]", story_id, user.name);

    let mutex = req.extensions().get::<Write<Database>>()
        .cloned()
        .expect("No database connection available");
    let pool = mutex.lock().unwrap();
    let ref conn = *pool.get().unwrap();

    let story = try!(Story::locked_for_write(conn, story_id, &user, false).iron());
    try!(story.unlock(conn).iron());

    debug!(".. Lock revoked succesfully.");

    Ok(Response::with(status::NoContent))
}

/// Register `/stories` routes and their required middleware.
pub fn route(router: &mut Router) {
    let mut acquire_lock_chain = Chain::new(acquire_lock);
    acquire_lock_chain.link_before(RequireUser);
    router.post("/stories/:id/lock", acquire_lock_chain);

    let mut revoke_lock_chain = Chain::new(revoke_lock);
    revoke_lock_chain.link_before(RequireUser);
    router.delete("/stories/:id/lock", revoke_lock_chain);
}
