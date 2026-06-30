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
