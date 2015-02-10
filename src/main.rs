#[macro_use] extern crate log;
extern crate env_logger;
extern crate iron;
extern crate router;
extern crate persistent;

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

    let gh_client_id = os::getenv("FICTION_GITHUBKEY").unwrap();
    let gh_client_key = os::getenv("FICTION_GITHUBSECRET").unwrap();
    let provider = oauth::Provider::github(gh_client_id, gh_client_key);

    let mut router = Router::new();
    router.get("/", health_check);
    provider.route("auth", &mut router);

    let mut chain = Chain::new(router);
    oauth::link(&mut chain);

    info!("Launching collaborative fiction API server on localhost:3000.");
    Iron::new(chain).listen("localhost:3000").unwrap();
}
