//! OAuth2 authentication providers.

extern crate router;
extern crate persistent;

use std::collections::HashSet;
use std::rand::{Rng, OsRng};
use std::old_io::IoError;
use std::sync::{Arc, Mutex};

use iron::prelude::*;
use iron::status;
use iron::{Url, Handler};
use iron::modifiers::Redirect;
use iron::typemap::Key;
use router::Router;
use persistent::Write;

/// Initial size of the "valid state parameter" pool.
const INIT_STATE_CAPACITY: usize = 100;

/// Length of the "state" parameter used to defeat XSS hijacking.
const STATE_LEN: usize = 20;

pub struct Provider {
    name: &'static str,
    request_uri: Url,
    token_uri: Url,

    client_id: String,
    client_secret: String
}

impl Key for Provider { type Value = Shared; }

impl Provider {

    fn new(name: &'static str, request_uri: Url, token_uri: Url, id: String, secret: String) -> Provider {
        Provider{
            name: name,
            request_uri: request_uri,
            token_uri: token_uri,
            client_id: id,
            client_secret: secret,
        }
    }

    /// Allocate a Provider configured to authenticate against GitHub's OAuth2 API.
    pub fn github(id: String, secret: String) -> Provider {
        let request = Url::parse("https://github.com/login/oauth/authorize").unwrap();
        let token = Url::parse("https://github.com/login/oauth/access_token").unwrap();

        Provider::new("github", request, token, id, secret)
    }

    /// Register the routes necessary to support this provider. Usually, this will involve a
    /// *redirect route*, which will redirect to an external authorization page, and a *callback
    /// route*, to which the provider is expected to return control with a redirect back.
    pub fn route(&self, root: &str, router: &mut Router) {
        let request_glob = format!("{}/{}", root, &self.name);
        let callback_glob = format!("{}/{}/callback", root, &self.name);

        let mut callback_uri = Url::parse(&format!(
            "http://localhost:3000/{}", &callback_glob
        )).unwrap();

        let request_handler = RequestHandler{
            client_id: self.client_id.clone(),
            request_uri: self.request_uri.clone(),
            callback_uri: callback_uri,
        };

        let callback_handler = CallbackHandler;

        router.get(request_glob, request_handler);
        router.get(callback_glob, callback_handler);
    }

}

struct Shared {
    rng: OsRng,
    valid_states: HashSet<String>,
}

impl Shared {

    /// Initialize the shared state to a reasonable starting point.
    fn new() -> Shared {
        Shared{
            rng: OsRng::new().unwrap(),
            valid_states: HashSet::with_capacity(INIT_STATE_CAPACITY),
        }
    }

    /// Generate an unguessable random string for use as a `state` parameter. Remember it as valid
    fn generate_state(&mut self) -> String {
        let state: String = self.rng.gen_ascii_chars().take(STATE_LEN).collect();
        self.valid_states.insert(state.clone());
        state
    }

    /// Verify that a given state is valid. Discard it from the provider's store if it is.
    fn validate_state(&mut self, state: &str) -> bool {
        self.valid_states.remove(state)
    }

}

/// Link supporting middleware into the chain to supply common shared state for all OAuth
/// providers.
pub fn link(chain: &mut Chain) {
    chain.link_before(Write::<Provider>::one(Shared::new()));
}

/// A Handler that redirects to the provider for authorization.
struct RequestHandler {
    client_id: String,
    request_uri: Url,
    callback_uri: Url,
}

impl Handler for RequestHandler {

    fn handle(&self, req: &mut Request) -> IronResult<Response> {
        let mutex = req.get::<Write<Provider>>().unwrap();
        let mut shared = mutex.lock().unwrap();
        let state = shared.generate_state();

        debug!("Redirecting to {} with state [{}].", self.callback_uri, state);

        let mut u = self.request_uri.clone();

        u.query = Some(format!(
            "client_id={}&redirect_uri={}&scope=user:email&state={}",
            &self.client_id, self.callback_uri, &state
        ));

        Ok(Response::new().set(Redirect(u)))
    }

}

/// A Handler that accepts the redirection back from the provider after authentication has succeeded
/// or failed. It performs a POST back to the provider to acquire a token based on the temporary `code`
/// and `state`.
struct CallbackHandler;

impl Handler for CallbackHandler {

    fn handle(&self, req: &mut Request) -> IronResult<Response> {
        debug!("GitHub callback reached. :sparkles: query = <{:?}>", req.url.query);

        Ok(Response::with((status::Ok, "Callback reached!")))
    }

}
