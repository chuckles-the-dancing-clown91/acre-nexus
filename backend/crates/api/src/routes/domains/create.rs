//! `POST /domains` — add a custom white-label domain (§7.3). Returns the
//! verification token + DNS instructions; the domain stays unverified (and does
//! not route) until DNS is confirmed via `POST /domains/<id>/verify`.

use super::dto::{CreateDomainReq, DomainResp};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::Domain;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

const VALID_AUDIENCES: &[&str] = &["admin", "owner", "renter"];

/// `POST /domains` — register a custom domain for verification.
#[rocket_okapi::openapi(tag = "Domains")]
#[post("/domains", data = "<body>")]
pub async fn create(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    body: Json<CreateDomainReq>,
) -> ApiResult<Json<DomainResp>> {
    user.require(Permission::DomainManage)?;
    let b = body.into_inner();
    let hostname = b.hostname.trim().to_lowercase();
    if hostname.is_empty() || !hostname.contains('.') {
        return Err(ApiError::BadRequest("a valid hostname is required".into()));
    }
    let audience = b.audience.unwrap_or_else(|| "admin".into());
    if !VALID_AUDIENCES.contains(&audience.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "invalid audience: {audience}"
        )));
    }

    if Domain::find()
        .filter(entity::domain::Column::Hostname.eq(hostname.clone()))
        .one(&state.db)
        .await?
        .is_some()
    {
        return Err(ApiError::Conflict("hostname already registered".into()));
    }

    let id = Uuid::new_v4();
    let token = format!("acre-verify={}", Uuid::new_v4().simple());
    let saved = entity::domain::ActiveModel {
        id: Set(id),
        tenant_id: Set(scope.tenant_id),
        hostname: Set(hostname.clone()),
        kind: Set("custom".into()),
        audience: Set(audience.clone()),
        verification_token: Set(Some(token)),
        verified_at: Set(None),
        tls_status: Set("pending".into()),
        created_at: Set(Utc::now().into()),
    }
    .insert(&state.db)
    .await?;

    crate::audit::record(
        &state.db,
        Some(user.user_id),
        crate::audit::actions::DOMAIN_CREATE,
        Some("domain"),
        Some(id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "hostname": hostname, "audience": audience })),
    )
    .await;

    Ok(Json(DomainResp::from(saved)))
}
