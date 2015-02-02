//! OAuth2 support for GitHub authentication.
//!
//! For reference see the documentation in the [GitHub API](https://developer.github.com/v3/oauth/).

extern crate log;
extern crate iron;
extern crate router;

use super::OAuthProvider;

use std::collections::HashSet;
use std::rand::Rng;
use std::old_io::IoError;

use iron::prelude::*;
use iron::status;
use iron::Url;
use iron::modifiers::Redirect;

use router::Router;

struct Provider <'p, R: Rng> {
    client_id: String,
    client_secret: String,

    valid_states: HashSet<String>,

    rng: R,
}

impl<'p, R: Rng> Provider<'p, R> {
    fn new(id: String, secret: String, rng: R) -> Provider<'p, R> {
        Provider{
            client_id: id,
            client_secret: secret,
            valid_states: HashSet::with_capacity(100),
            rng: rng,
        }
    }
}

impl<'p, R: Rng> OAuthProvider for Provider <'p, R> {
    fn register <'a> (&self, root: &str, router: &'a mut Router) -> &'a Router {
        router.get(format!("{}/github", root), |&: _: &mut Request| {
            let state: String = self.rng.gen_iter().take(20).collect();
            debug!("Redirecting to GitHub with state [{}].", state);

            let u = Url::parse(&format!(
                "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&scope={}&state={}",
                &self.client_id, "http://localhost:3000/auth/github/callback", "user:email", &state)
            ).unwrap();

            Ok(Response::new().set(Redirect(u)))
        });

        router.get(format!("{}/github/callback", root), |&: _: &mut Request| {
            debug!("GitHub callback reached. :sparkles:");

            Ok(Response::with((status::Ok, "Callback reached!")))
        });

        router
    }
}
