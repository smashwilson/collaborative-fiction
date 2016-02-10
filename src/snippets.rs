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
use error::IntoIronResult;

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

    debug!("POST /snippets [{}]", u.name);

    let mutex = req.extensions().get::<Write<Database>>()
        .cloned()
        .expect("No database connection available");
    let pool = mutex.lock().unwrap();
    let ref conn = *pool.get().unwrap();

    match body.snippet.story_id {
        Some(id) => {
            debug!(".. Into existing story id {}", id);

            // Ensure that the current user holds an active lock on an existing Story.
            let mut story = try!(Story::locked_for_write(conn, id, &u, false).iron());

            try!(Snippet::contribute(conn, &story, &u, body.snippet.content).iron());

            story.contribution_count += 1;

            try!(story.save(conn).iron());
            try!(story.unlock(conn).iron());

            Ok(Response::with(status::Created))
        },
        None => {
            // Begin a new Story belonging to the authenticated User and containing the newly
            // created Snippet.
            debug!(".. Creating a new Story");

            try!(Snippet::begin(conn, &u, body.snippet.content).iron());

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
