use crate::state::AppState;
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};
use uuid::Uuid;

use super::helpers::{resolve_host, resolve_tenant_ref};

/// Tenant resolved for an **unauthenticated** public-website request.
#[derive(Clone, Debug)]
pub struct PublicTenant {
    pub tenant_id: Uuid,
    /// The app surface this host serves (`admin` / `owner` / `renter`), when the
    /// tenant was resolved from a configured domain. Defaults to `admin`.
    /// Consumed by audience-gated public routes (e.g. owner vs renter portal).
    #[allow(dead_code)]
    pub audience: String,
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
        // Explicit X-Tenant / ?tenant wins (used by the dev/public site).
        if let Some(r) = reference {
            return match resolve_tenant_ref(state, &r).await {
                Some(id) => Outcome::Success(PublicTenant {
                    tenant_id: id,
                    audience: "admin".into(),
                }),
                None => Outcome::Error((Status::NotFound, ())),
            };
        }

        // Otherwise resolve the inbound Host against the domain table (§7.2).
        if let Some(host) = req.headers().get_one("Host") {
            if let Some(resolved) = resolve_host(state, host).await {
                return Outcome::Success(PublicTenant {
                    tenant_id: resolved.tenant_id,
                    audience: resolved.audience,
                });
            }
        }
        Outcome::Error((Status::BadRequest, ()))
    }
}
