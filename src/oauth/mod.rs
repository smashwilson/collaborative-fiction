//! OAuth2 authentication providers.

extern crate router;

use std::collections::HashSet;
use std::rand::{Rng, OsRng};
use std::old_io::IoError;
use std::sync::{Arc, Mutex};

use iron::prelude::*;
use iron::status;
use iron::Url;
use iron::modifiers::Redirect;
use router::Router;

pub struct OAuthProvider {
    name: String,
    request_uri: Url,
    token_uri: Url,

    client_id: String,
    client_secret: String,

    valid_states: Mutex<HashSet<String>>
}

impl OAuthProvider {
    pub fn new(request_uri: String, token_uri: String, id: String, secret: String) -> Provider {
        Provider{
            request_uri: request_uri,
            token_uri: token_uri,
            client_id: id,
            client_secret: secret,
            valid_states: Mutex::new(HashSet::with_capacity(100)),
        }
    }

    /// Generate an unguessable random string for use as a `state` parameter. Remember it as valid
    /// within the provider.
    pub fn generate_state <R: Rng> (&self, rng: R) -> String {
        let state = rng.gen_ascii_chars().take(20).collect();
        let states = self.valid_states.lock().unwrap();

        states.insert(state);

        state
    }

    /// Verify that a given state is valid. Discard it from the provider's store if it is.
    pub fn validate_state(&self, state: &str) -> bool {
        let states = self.valid_states.lock().unwrap();

        states.remove(state)
    }

    /// Register the routes necessary to support this provider. Usually, this will involve a
    /// *redirect route*, which will redirect to an external authorization page, and a *callback
    /// route*, to which the provider is expected to return control with a redirect back.
    pub fn register <'a> (&self, root: &str, router: &'a mut Router) -> &'a Router {
        let shared_provider = Arc::new(self);

        let request_glob = format!("{}/{}", root, &self.name);
        let callback_glob = format!("{}/{}/callback", root, &self.name);

        let request_handler = RequestHandler{
            provider: shared_provider.clone(),
            rng: OsRng::new().unwrap(),
            callback_uri: callback_glob,
        };

        let callback_handler = CallbackHandler{
            provider: shared_provider.clone(),
        };

        router.get(request_glob, request_handler);
        router.get(callback_glob, callback_handler);

        router.get(format!("{}/github/callback", root), |&: _: &mut Request| {

        });

        router
    }
}

/// A Handler that redirects to the provider for authorization.
struct RequestHandler {
    provider: Arc<OAuthProvider>,
    rng: OsRng,
    callback_uri: String
}

impl Handler for RequestHandler {

    fn call(&self, _: &mut Request) -> IronResult<Response> {
        let state = self.provider.generate_state(self.rng);

        debug!("Redirecting to {} with state [{}].", self.provider.name, state);

        let mut u = self.provider.request_uri.clone()

        u.query = Some(!format!(
            "client_id={}&redirect_uri={}&scope=user:email&state={}",
            &self.provider.client_id, &self.callback_uri, state
        ))

        Ok(Response::new().set(Redirect(u)))
    }

}

/// A Handler that accepts the redirection back from the provider after authentication has succeeded
/// or failed. It performs a POST back to the provider to acquire a token based on the temporary `code`
/// and `state`.
struct CallbackHandler {
    provider: Arc<OAuthProvider>,
}

impl Handler for CallbackHandler {

    fn call(&self, req: &mut Request) -> IronResult<Response> {
        debug!("GitHub callback reached. :sparkles: query = <{}>", req.url.query);

        Ok(Response::with((status::Ok, "Callback reached!")))
    }

}
