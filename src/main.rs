#[macro_use] extern crate log;
extern crate env_logger;
extern crate iron;
extern crate router;

use iron::prelude::*;
use iron::status;
use router::Router;

fn health_check(_: &mut Request) -> IronResult<Response> {
    info!("Health check request.");

    Ok(Response::with((status::Ok, "Up and running.")))
}

fn main() {
    env_logger::init().unwrap();

    let mut router = Router::new();

    router.get("/", health_check);

    info!("Launching collaborative fiction API server on localhost:3000.");
    Iron::new(router).listen("localhost:3000").unwrap();
}
