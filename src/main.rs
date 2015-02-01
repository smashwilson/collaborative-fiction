extern crate iron;
extern crate router;

use iron::prelude::*;
use iron::status;
use router::Router;

fn health_check(_: &mut Request) -> IronResult<Response> {
    Ok(Response::with((status::Ok, "Up and running.")))
}

fn main() {
    let mut router = Router::new();

    router.get("/", health_check);

    Iron::new(router).listen("localhost:3000").unwrap();

    println!("Collaborative fiction API server: alive and running.");
}
