#![deny(deprecated,stable_features,unused_mut)]
#![feature(env,core,io)]

#[macro_use]
extern crate log;

extern crate env_logger;
extern crate iron;
extern crate router;
extern crate persistent;
extern crate rand;
extern crate hyper;
extern crate "rustc-serialize" as rustc_serialize;
extern crate url;
extern crate postgres;

use std::env;

use iron::prelude::*;
use iron::status;
use router::Router;
use oauth::Provider;

mod oauth;

/// Respond with a simple string on `/` to be able to quickly check if it's up.
fn health_check(_: &mut Request) -> IronResult<Response> {
    info!("Health check request.");

    Ok(Response::with((status::Ok, "Up and running.")))
}

fn main() {
    env_logger::init().unwrap();

    let gh_client_id = env::var("FICTION_GITHUBID").unwrap();
    let gh_client_key = env::var("FICTION_GITHUBSECRET").unwrap();
    let github = oauth::GitHub::new("auth", gh_client_id, gh_client_key);

    let mut router = Router::new();
    router.get("/", health_check);
    github.route(&mut router);

    let mut chain = Chain::new(router);
    github.link(&mut chain);

    info!("Launching collaborative fiction API server on localhost:3000.");
    Iron::new(chain).listen("localhost:3000").unwrap();
}
