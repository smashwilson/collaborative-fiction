#![deny(deprecated,stable_features,unused_mut)]

#[macro_use]
extern crate log;

extern crate env_logger;
extern crate iron;
extern crate router;
extern crate persistent;
extern crate bodyparser;
extern crate rand;
extern crate hyper;
extern crate rustc_serialize;
extern crate url;
extern crate postgres;
extern crate r2d2;
extern crate r2d2_postgres;
extern crate plugin;
extern crate time;

use std::env;
use std::process;

use iron::prelude::*;
use iron::status;
use router::Router;

use oauth::Provider;
use model::Database;
use error::FictResult;

mod error;
mod oauth;
mod model;

mod auth;

mod whoami;
mod snippets;

/// Respond with a simple string on `/` to be able to quickly check if it's up.
fn health_check(_: &mut Request) -> IronResult<Response> {
    info!("Health check request.");

    Ok(Response::with((status::Ok, "Up and running.")))
}

fn main() {
    let status = match launch() {
        Ok(..) => 0,
        Err(e) => { error!("Oops: {}", e); 1 },
    };
    process::exit(status);
}

fn launch() -> FictResult<()> {
    try!(env_logger::init());

    let gh_client_id = try!(env::var("FICTION_GITHUBID"));
    let gh_client_key = try!(env::var("FICTION_GITHUBSECRET"));
    let github = oauth::GitHub::new("auth", gh_client_id, gh_client_key);

    let mut router = Router::new();
    router.get("/", health_check);
    github.route(&mut router);
    whoami::route(&mut router);

    let mut chain = Chain::new(router);
    try!(Database::link(&mut chain));
    github.link(&mut chain);

    info!("Launching collaborative fiction API server on localhost:3000.");
    try!(Iron::new(chain).http("localhost:3000"));

    Ok(())
}
