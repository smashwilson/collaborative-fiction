//! The GitHub OAuth provider.

use std::sync::{Arc, Mutex};
use std::borrow::ToOwned;

use iron::{Chain, Request};
use iron::typemap::Key;
use iron::Url as IronUrl;
use hyper::Url as HyperUrl;
use persistent::Write;
use rustc_serialize::json::Json;
use plugin::Pluggable;

use error::{FictResult, fict_err};
use oauth::{Provider, Options, Shared};
use oauth::connection::JsonConnection;

/// Implement OAuth for GitHub.
#[derive(Clone)]
pub struct GitHub {
    options: Options,
}

impl GitHub {

    pub fn new(root: &'static str, id: String, secret: String) -> GitHub {
        GitHub{
            options: Options{
                name: "github",
                root: root,
                client_id: id,
                client_secret: secret,
                request_uri: IronUrl::parse("https://github.com/login/oauth/authorize").unwrap(),
                token_uri: HyperUrl::parse("https://github.com/login/oauth/access_token").unwrap(),
            }
        }
    }

}

impl Key for GitHub {

    type Value = Shared;

}

impl Provider for GitHub {

    fn options(&self) -> &Options {
        &self.options
    }

    fn shared_mutex(&self, req: &mut Request) -> Arc<Mutex<Shared>> {
        req.get::<Write<GitHub>>().unwrap_or_else(|_| {
            panic!("Shared GitHub content not found.");
        })
    }

    fn scopes(&self) -> &'static str {
        "user:email"
    }

    fn get_user_data(&self, token: &str) -> FictResult<(String, String)> {
        debug!("Acquiring user profile from GitHub.");

        let mut conn = JsonConnection::new("token", token);

        let profile_doc = try!(conn.get("https://api.github.com/user"));

        let username = try!(profile_doc.find("login")
            .and_then(|login| login.as_string())
            .ok_or(fict_err("GitHub profile element 'login' was not a string")));

        match profile_doc.find("email") {
            Some(&Json::String(ref public_email)) => {
                debug!("Discovered public email {} in GitHub profile.", public_email);
                return Ok((public_email.to_owned(), username.to_owned()));
            },
            Some(&Json::Null) => (),
            _ => return Err(fict_err("GitHub profile element 'email' was not a string or null")),
        }

        debug!("Profile email is not public. Requesting email address resource.");

        let email_doc = try!(conn.get("https://api.github.com/user/emails"));

        let emails = try!(email_doc.as_array()
            .ok_or(fict_err("GitHub email document root was not an array")));

        let primary_email = try!(emails.iter()
            .find(|doc| doc.find("primary").and_then(|n| n.as_boolean()).unwrap_or(false))
            .and_then(|doc| doc.find("email").and_then(|n| n.as_string()))
            .ok_or(fict_err("No primary email specified")));

        return Ok((primary_email.to_owned(), username.to_owned()));
    }

    fn link(&self, chain: &mut Chain) {
        chain.link_before(Write::<GitHub>::one(Shared::new()));
    }
}
