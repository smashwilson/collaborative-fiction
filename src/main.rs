extern crate iron;

use iron::prelude::*;
use iron::status;

fn health_check(_: &mut Request) -> IronResult<Response> {
    Ok(Response::with((status::Ok, "Up and running.")))
}

fn main() {
    Iron::new(health_check).listen("localhost:3000").unwrap();

    println!("Collaborative fiction API server: alive and running.");
}
