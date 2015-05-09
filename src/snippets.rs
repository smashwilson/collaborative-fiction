//! Snippet creation endpoints.

use iron::{Request, Response, IronResult, IronError, Chain};
use iron::status;
use router::Router;
use persistent::{Read, Write};
use bodyparser;
use plugin::Pluggable;
use plugin::Extensible;

use model::{Database, Snippet, Story, AccessLevel};
use auth::{AuthUser, RequireUser};
use error::FictError;

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
            // Append a new Snippet to an existing Story.
            let story_opt = try!(Story::with_id(&*conn, id)
                .map_err(|err| err.iron(status::InternalServerError)));

            let story = match story_opt {
                Some(s) => s,
                None => return Ok(Response::with(("No such story", status::NotFound))),
            };

            let access: AccessLevel = try!(story.access_for(&*conn, &u)
                .map_err(|err: FictError| err.iron(status::InternalServerError)));

            if ! access.grants_write() {
                return Ok(Response::with(("No such story", status::NotFound)));
            }

            try!(Snippet::contribute(&*conn, &story, &u, body.snippet.content)
                .map_err(|err: FictError| err.iron(status::InternalServerError)));

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
