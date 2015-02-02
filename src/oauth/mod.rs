//! OAuth2 authentication providers.

extern crate router;

use router::Router;

pub trait OAuthProvider {

    /// Register the routes necessary to support this provider. Usually, this will involve a
    /// *redirect route*, which will redirect to an external authorization page, and a *callback
    /// route*, to which the provider is expected to return control with a redirect back.
    fn register <'a> (&self, root: &str, router: &'a mut Router) -> &'a Router;

}

mod github;
