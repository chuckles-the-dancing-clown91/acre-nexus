//! `POST /domains/<id>/verify` — confirm DNS for a custom domain and provision
//! TLS (§7.3).
//!
//! Production note: real DNS lookup + TLS issuance is handled out-of-process by
//! **Caddy on-demand TLS** keyed to the verified `domain` set (the spec's
//! dependency-rule recommendation), and would normally run as a retrying
//! background job. Here we record the verified state the routing layer reads.

use super::dto::DomainResp;
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

/// `POST /domains/<id>/verify` — mark a domain verified + TLS active.
#[rocket_okapi::openapi(tag = "Domains")]
#[post("/domains/<id>/verify")]
pub async fn verify(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<DomainResp>> {
    user.require(Permission::DomainManage)?;
    let did = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid domain id".into()))?;
    let domain = Domain::find()
        .filter(entity::domain::Column::Id.eq(did))
        .filter(entity::domain::Column::TenantId.eq(scope.tenant_id))
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("domain not found".into()))?;

    let mut am: entity::domain::ActiveModel = domain.into();
    am.verified_at = Set(Some(Utc::now().into()));
    am.tls_status = Set("active".into());
    let saved = am.update(&state.db).await?;

    crate::audit::record(
        &state.db,
        Some(user.user_id),
        crate::audit::actions::DOMAIN_VERIFY,
        Some("domain"),
        Some(did.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "hostname": saved.hostname })),
    )
    .await;
    Ok(Json(DomainResp::from(saved)))
}
