//! Snippet creation endpoints.

use iron::{Request, Response, IronResult, Chain};
use iron::status;
use router::Router;
use persistent::{Read, Write};
use bodyparser;
use plugin::Pluggable;
use plugin::Extensible;

use model::{Database, Snippet, Story};
use auth::{AuthUser, RequireUser};
use error::FictError::{NotFound, Unlocked, AlreadyLocked};

#[derive(Debug, Clone, RustcEncodable, RustcDecodable)]
struct CreationBody {
    snippet: SnippetBody
}

#[derive(Debug, Clone, RustcEncodable, RustcDecodable)]
struct SnippetBody {
    content: String,
    story_id: Option<i64>
}

pub fn post(req: &mut Request) -> IronResult<Response> {
    let u = req.extensions().get::<AuthUser>().cloned()
        .expect("No authenticated user");

    let body = match req.get::<bodyparser::Struct<CreationBody>>() {
        Ok(Some(b)) => b,
        Ok(None) => {
            return Ok(Response::with(("Expected a request body", status::BadRequest)))
        },
        Err(err) => {
            warn!("Unable to parse request body: {:?}", err);
            return Ok(Response::with(("Unable to parse request body", status::BadRequest)))
        }
    };

    let mutex = req.extensions().get::<Write<Database>>()
        .cloned()
        .expect("No database connection available");
    let pool = mutex.lock().unwrap();
    let conn = pool.get().unwrap();

    match body.snippet.story_id {
        Some(id) => {
            // Ensure that the current user holds an active lock on an existing Story.
            let story = match Story::locked_for_write(&*conn, id, &u, false) {
                Ok(s) => s,
                Err(e @ NotFound) => return Err(e.iron(status::NotFound)),
                Err(e @ Unlocked) | Err(e @ AlreadyLocked {..}) => return Err(e.iron(status::Forbidden)),
                Err(e) => {
                    error!("Unable to lock story for snippet addition: {:?}", e);
                    return Err(e.iron(status::InternalServerError))
                }
            };

            try!(Snippet::contribute(&*conn, &story, &u, body.snippet.content)
                .map_err(|err| err.iron(status::InternalServerError)));

            try!(story.unlock(&*conn)
                .map_err(|err| err.iron(status::InternalServerError)));

            Ok(Response::with(status::Created))
        },
        None => {
            // Begin a new Story belonging to the authenticated User and containing the newly
            // created Snippet.

            try!(Snippet::begin(&*conn, &u, body.snippet.content)
                .map_err(|err| err.iron(status::InternalServerError)));

            Ok(Response::with(status::Created))
        }
    }
}

const MAX_BODY_LENGTH: usize = 1024 * 1024 * 10;

/// Add the `/snippets` route and its required middleware to a borrowed Router.
pub fn route(router: &mut Router) {
    let mut chain = Chain::new(post);

    chain.link_before(RequireUser);
    chain.link_before(Read::<bodyparser::MaxBodyLength>::one(MAX_BODY_LENGTH));

    router.post("/snippets", chain);
}
