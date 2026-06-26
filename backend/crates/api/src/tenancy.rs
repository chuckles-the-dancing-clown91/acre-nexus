//! Tenant-resolution request guards.
//!
//! Every tenant-scoped query must be filtered by the active tenant. These guards
//! produce that id from the right source depending on the caller:
//!
//! * **Authenticated users** — their own `tenant_id` from the JWT. Platform staff
//!   (no tenant) may *impersonate* a tenant by passing an `X-Tenant` header
//!   (slug or uuid) — useful for the HQ "view as client" flow.
//! * **Public website visitors** — resolved from the `X-Tenant` header or
//!   `?tenant=<slug>` query param (no auth).

use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::state::AppState;
use entity::prelude::Tenant;
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

/// The active tenant for an authenticated, tenant-scoped request.
#[derive(Clone, Copy, Debug)]
pub struct TenantScope {
    pub tenant_id: Uuid,
    /// True when a staff user is impersonating this tenant. Surfaced for audit
    /// logging / "viewing as" banners that consume it.
    #[allow(dead_code)]
    pub impersonated: bool,
}

async fn resolve_tenant_ref(state: &AppState, reference: &str) -> Option<Uuid> {
    // Accept either a raw uuid or a slug.
    if let Ok(id) = Uuid::parse_str(reference) {
        return Some(id);
    }
    Tenant::find()
        .filter(entity::tenant::Column::Slug.eq(reference))
        .one(&state.db)
        .await
        .ok()
        .flatten()
        .map(|t| t.id)
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

        let header_tenant = req.headers().get_one("X-Tenant");

        // Staff may impersonate a tenant via header; clients are pinned to theirs.
        if user.is_staff {
            if let Some(reference) = header_tenant {
                if let Some(id) = resolve_tenant_ref(state, reference).await {
                    return Outcome::Success(TenantScope {
                        tenant_id: id,
                        impersonated: true,
                    });
                }
                return Outcome::Error((Status::BadRequest, ()));
            }
            // Staff without an X-Tenant header have no single tenant scope.
            return Outcome::Error((Status::BadRequest, ()));
        }

        match user.tenant_id {
            Some(id) => Outcome::Success(TenantScope {
                tenant_id: id,
                impersonated: false,
            }),
            None => Outcome::Error((Status::Unauthorized, ())),
        }
    }
}

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

/// Helper for handlers that need to turn a guard failure into a JSON error.
#[allow(dead_code)]
pub fn tenant_required() -> ApiError {
    ApiError::BadRequest("tenant context required — pass X-Tenant header or ?tenant=<slug>".into())
}
