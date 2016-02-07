//! The `/whoami` endpoint.

use iron::{Request, Response, IronResult, Chain};
use iron::status;
use router::Router;
use rustc_serialize::json;

use auth::{AuthUser, RequireUser};

#[derive(RustcEncodable)]
struct Payload<'a> {
    name: &'a str,
    email: &'a str
}

/// Generate a JSON document containing information about the current user.
fn get(req: &mut Request) -> IronResult<Response> {
    let u = req.extensions.get::<AuthUser>().unwrap();

    let p = Payload{
        name: &u.name,
        email: &u.email
    };

    let encoded = json::encode(&p).unwrap();

    Ok(Response::with((status::Ok, encoded)))
}

/// Add the `/whoami` route and its required middleware to a borrowed Router.
pub fn route(router: &mut Router) {
    let mut chain = Chain::new(get);

    chain.link_before(RequireUser);

    router.get("/whoami", chain);
}
