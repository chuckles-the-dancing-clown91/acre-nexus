//! Small shared request guards.

use rocket::request::{FromRequest, Outcome, Request};

/// The resolved client IP (honouring proxy headers Rocket is configured to trust),
/// for audit trails such as the e-signature record. Never fails — absent IP is `None`.
pub struct ClientIp(pub Option<String>);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for ClientIp {
    type Error = ();
    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        Outcome::Success(ClientIp(req.client_ip().map(|ip| ip.to_string())))
    }
}

/// The request's `User-Agent` header (truncated to something storable), for the
/// e-signature audit trail. Never fails — absent header is `None`.
pub struct UserAgent(pub Option<String>);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for UserAgent {
    type Error = ();
    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        Outcome::Success(UserAgent(
            req.headers()
                .get_one("User-Agent")
                .map(|s| s.chars().take(512).collect()),
        ))
    }
}
