use crate::state::AppState;
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};
use uuid::Uuid;

use super::helpers::resolve_tenant_ref;

/// Tenant resolved for an **unauthenticated** public-website request.
#[derive(Clone, Copy, Debug)]
pub struct PublicTenant {
    pub tenant_id: Uuid,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for PublicTenant {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let state = match req.rocket().state::<AppState>() {
            Some(s) => s,
            None => return Outcome::Error((Status::InternalServerError, ())),
        };
        let reference = req
            .headers()
            .get_one("X-Tenant")
            .map(|s| s.to_string())
            .or_else(|| {
                req.uri().query().and_then(|q| {
                    q.segments()
                        .find(|(k, _)| *k == "tenant")
                        .map(|(_, v)| v.to_string())
                })
            });
        match reference {
            Some(r) => match resolve_tenant_ref(state, &r).await {
                Some(id) => Outcome::Success(PublicTenant { tenant_id: id }),
                None => Outcome::Error((Status::NotFound, ())),
            },
            None => Outcome::Error((Status::BadRequest, ())),
        }
    }
}
