#[macro_use] extern crate log;
extern crate env_logger;
extern crate iron;
extern crate router;

use oauth::OAuthProvider;

use std::os;
use std::rand::{Rng, OsRng};

use iron::prelude::*;
use iron::status;
use router::Router;

mod oauth;

fn health_check(_: &mut Request) -> IronResult<Response> {
    info!("Health check request.");

    Ok(Response::with((status::Ok, "Up and running.")))
}

fn main() {
    env_logger::init().unwrap();

    let rng = OsRng::new().unwrap();

    let gh_client_id = os::getenv("FICTION_GITHUBKEY").unwrap();
    let gh_client_key = os::getenv("FICTION_GITHUBSECRET").unwrap();
    let provider = oauth::github::Provider::new(gh_client_id, gh_client_key, rng);

    let mut router = Router::new();

    router.get("/", health_check);

    router = *provider.register("auth", &mut router);

    info!("Launching collaborative fiction API server on localhost:3000.");
    Iron::new(router).listen("localhost:3000").unwrap();
}
