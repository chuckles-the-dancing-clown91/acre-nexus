use crate::auth::AuthUser;
use crate::state::AppState;
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};
use uuid::Uuid;

use super::helpers::resolve_tenant_ref;

/// The active tenant for an authenticated, tenant-scoped request.
#[derive(Clone, Copy, Debug)]
pub struct TenantScope {
    pub tenant_id: Uuid,
    /// True when a staff user is impersonating this tenant. Surfaced for audit
    /// logging / "viewing as" banners that consume it.
    #[allow(dead_code)]
    pub impersonated: bool,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for TenantScope {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let state = match req.rocket().state::<AppState>() {
            Some(s) => s,
            None => return Outcome::Error((Status::InternalServerError, ())),
        };
        let user = match req.guard::<AuthUser>().await {
            Outcome::Success(u) => u,
            _ => return Outcome::Error((Status::Unauthorized, ())),
        };

        // The workspace the token is scoped to (set at login or by /auth/switch)
        // always wins. For staff this is their chosen "view as" workspace.
        if let Some(id) = user.tenant_id {
            return Outcome::Success(TenantScope {
                tenant_id: id,
                impersonated: user.is_staff,
            });
        }

        // Staff with no active workspace may impersonate one via the X-Tenant
        // header (the legacy "view as client" path used before switching).
        if user.is_staff {
            if let Some(reference) = req.headers().get_one("X-Tenant") {
                if let Some(id) = resolve_tenant_ref(state, reference).await {
                    return Outcome::Success(TenantScope {
                        tenant_id: id,
                        impersonated: true,
                    });
                }
                return Outcome::Error((Status::BadRequest, ()));
            }
            return Outcome::Error((Status::BadRequest, ()));
        }

        // A non-staff user with no tenant context cannot resolve a workspace.
        Outcome::Error((Status::Unauthorized, ()))
    }
}
