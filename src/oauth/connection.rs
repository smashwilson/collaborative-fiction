use std::io::Read;
use std::borrow::ToOwned;

use hyper::Client;
use hyper::header::{Accept, Authorization, UserAgent, qitem};
use hyper::mime::{Mime, TopLevel, SubLevel};
use rustc_serialize::json::Json;

use error::FictResult;

/// Custom user agent to use for outgoing requests.
const USER_AGENT: &'static str = "collabfict/0.0.1 hyper/0.3.13 rust/1.0.0-beta.2";

/// Manage a connection to an HTTPS API that accepts and produces JSON documents.
pub struct JsonConnection {
    client: Client,
    auth: Authorization<String>,
}

/// A re-usable HTTP connection that sends and accepts JSON payloads.
impl JsonConnection {

    pub fn new(auth_method: &str, token: &str) -> JsonConnection {
        let auth_body = format!("{} {}", auth_method, token);

        JsonConnection{
            client: Client::new(),
            auth: Authorization(auth_body)
        }
    }

    pub fn get(&mut self, url: &str) -> FictResult<Json> {
        let mut req = self.client.get(url);
        req = req.header(self.auth.clone());
        req = req.header(Accept(vec![qitem(Mime(TopLevel::Application, SubLevel::Json, vec![]))]));
        req = req.header(UserAgent(USER_AGENT.to_owned()));

        let mut resp = try!(req.send());

        let mut resp_body = String::new();
        try!(resp.read_to_string(&mut resp_body));

        Json::from_str(&resp_body)
            .map_err(|e| From::from(e) )
    }

}
