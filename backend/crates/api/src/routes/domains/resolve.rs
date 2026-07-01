//! `GET /public/resolve` — the unauthenticated routing entrypoint (§7.2).
//!
//! Maps an inbound host (the `Host` header, or an explicit `?host=` override) to
//! its tenant, audience, and branding so the frontend can pick the right surface
//! (admin app vs owner portal vs renter portal) and theme before login. An
//! unknown / unverified host returns `404` — the caller falls back to marketing.

use super::dto::ResolveResp;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use crate::tenancy::helpers::resolve_host;
use entity::prelude::{Tenant, Theme};
use rocket::request::{FromRequest, Outcome, Request};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

/// The inbound `Host` header, extracted for the routing layer.
pub struct HostHeader(pub Option<String>);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for HostHeader {
    type Error = ();
    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        Outcome::Success(HostHeader(
            req.headers().get_one("Host").map(|s| s.to_string()),
        ))
    }
}

/// `GET /public/resolve?host=<host>` — resolve a host to tenant + audience + theme.
#[rocket_okapi::openapi(tag = "Public Website")]
#[get("/public/resolve?<host>")]
pub async fn resolve(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    host: Option<String>,
    header: HostHeader,
) -> ApiResult<Json<ResolveResp>> {
    let hostname = host
        .or(header.0)
        .ok_or_else(|| ApiError::BadRequest("no host provided".into()))?;
    let resolved = resolve_host(state, &hostname)
        .await
        .ok_or_else(|| ApiError::NotFound("host not mapped".into()))?;

    let tenant = Tenant::find_by_id(resolved.tenant_id)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("tenant not found".into()))?;
    let theme = Theme::find()
        .filter(entity::theme::Column::TenantId.eq(resolved.tenant_id))
        .one(&db)
        .await?;
    let (company_name, primary_color, accent_color) = match theme {
        Some(t) => (t.company_name, t.primary_color, t.accent_color),
        None => (tenant.name.clone(), "#F5451F".into(), "#F5451F".into()),
    };

    Ok(Json(ResolveResp {
        tenant_id: tenant.id,
        tenant_slug: tenant.slug,
        audience: resolved.audience,
        company_name,
        primary_color,
        accent_color,
    }))
}
