//! OAuth2 authentication providers.

extern crate router;
extern crate persistent;

use std::collections::HashSet;

use iron::prelude::*;
use iron::status;
use iron::{Url, Handler};
use iron::modifiers::Redirect;
use iron::typemap::Key;
use router::Router;
use persistent::Write;
use rand::{OsRng, Rng};
use hyper::Client;
use hyper::header::{Accept, qitem};
use hyper::mime::{Mime, TopLevel, SubLevel};

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

        let callback_uri = Url::parse(&format!(
            "http://localhost:3000/{}", &callback_glob
        )).unwrap();

        let request_handler = RequestHandler{
            client_id: self.client_id.clone(),
            request_uri: self.request_uri.clone(),
            callback_uri: callback_uri,
        };

        let callback_handler = CallbackHandler{
            client_id: self.client_id.clone(),
            client_secret: self.client_secret.clone(),
            token_uri: self.token_uri.clone(),
        };

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

        let mut u = self.request_uri.clone();
        u.query = Some(format!(
            "client_id={}&redirect_uri={}&scope=user:email&state={}",
            &self.client_id, self.callback_uri, &state
        ));

        debug!("Redirecting to [{}].", u);

        Ok(Response::with((status::Found, Redirect(u))))
    }

}

/// A Handler that accepts the redirection back from the provider after authentication has succeeded
/// or failed. It performs a POST back to the provider to acquire a token based on the temporary `code`
/// and `state`.
struct CallbackHandler {
    client_id: String,
    client_secret: String,
    token_uri: Url,
}

impl Handler for CallbackHandler {

    fn handle(&self, req: &mut Request) -> IronResult<Response> {
        let mutex = req.get::<Write<Provider>>().unwrap();
        let mut shared = mutex.lock().unwrap();

        let result = extract_callback_params(req).and_then(|(code, state)| {
            validate_state(&mut *shared, &state).map(|_| { code })
        }).and_then(|code| {
            generate_token(&self, code)
        });

        match result {
            Ok(token) => {
                debug!("OAuth flow completed. Acquired token: [{}]", token);

                let output = format!("Callback reached! Your token is [{}].", token);
                Ok(Response::with((status::Ok, output)))
            },
            Err(message) => {
                warn!("OAuth flow problem: {}", message);

                Ok(Response::with((status::BadRequest, message)))
            },
        }
    }

}

/// Extract the "code" and "state" query parameters from the callback request. Fail if either are
/// not present.
fn extract_callback_params(req: &Request) -> Result<(String, String), &'static str> {
    let u = req.url.clone().into_generic_url();

    let mut code_op: Option<String> = None;
    let mut state_op: Option<String> = None;

    match u.query_pairs() {
        Some(pairs) => {
            for pair in pairs.iter() {
                let (ref key, ref value) = *pair;
                let key_str: &str = key;

                match key_str {
                    "code" => code_op = Some(value.clone()),
                    "state" => state_op = Some(value.clone()),
                    _ => warn!("Unrecognized callback parameter: [{}]", &key),
                }
            }
        },
        None => {
            warn!("Callback request missing any query parameters: [{}]", u);
            return Err("Callback missing query parameters");
        },
    };

    match (code_op, state_op) {
        (Some(code), Some(state)) => Ok((code, state)),
        _ => Err("Callback request missing required query parameters"),
    }
}

fn validate_state(shared: &mut Shared, state: &str) -> Result<(), &'static str> {
    if shared.validate_state(state) {
        Ok(())
    } else {
        Err("Unfamiliar state encountered. Danger: this could be an XSS attack!")
    }
}

fn generate_token(handler: &CallbackHandler, code: String) -> Result<String, &'static str> {
    let mut client = Client::new();
    let u: &str = &format!("{}", handler.token_uri);
    let b: &str = &format!("client_id={}&client_secret={}&code={}",
        &handler.client_id, &handler.client_secret, &code
    );

    debug!("Attempting to acquire token from: [{}]", u);

    let mut req = client.post(u).body(b);
    req = req.header(Accept(vec![qitem(Mime(TopLevel::Application, SubLevel::Json, vec![]))]));

    match req.send() {
        Ok(mut response) => response.read_to_string().map_err(|_| "Unable to read response"),
        Err(_) => Err("Unable to acquire a token for you."),
    }
}
