//! `PATCH /llcs/<id>` — update an LLC's onboarding profile.

use super::dto::{LlcResp, UpdateLlcReq};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::Llc;
use rocket::serde::json::Json;
use rocket::{patch, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `PATCH /llcs/<id>` — update onboarding profile fields. Moving `status` to
/// `active` stamps `onboarded_at` the first time.
#[rocket_okapi::openapi(tag = "LLCs")]
#[patch("/llcs/<id>", data = "<body>")]
pub async fn update(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateLlcReq>,
) -> ApiResult<Json<LlcResp>> {
    user.require(Permission::LlcManage)?;
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let existing = Llc::find_by_id(lid)
        .filter(entity::llc::Column::TenantId.eq(scope.tenant_id))
        .one(&state.property_db)
        .await?
        .ok_or_else(|| ApiError::NotFound("llc not found".into()))?;
    let was_onboarded = existing.onboarded_at.is_some();
    let b = body.into_inner();
    let now = Utc::now();
    let mut am: entity::llc::ActiveModel = existing.into();
    if let Some(v) = b.name {
        am.name = Set(v);
    }
    if let Some(v) = b.ein {
        am.ein = Set(v);
    }
    if let Some(v) = b.state {
        am.state = Set(v);
    }
    if let Some(v) = b.entity_type {
        am.entity_type = Set(v);
    }
    if let Some(v) = b.formation_date {
        am.formation_date = Set(Some(v));
    }
    if let Some(v) = b.registered_agent {
        am.registered_agent = Set(Some(v));
    }
    if let Some(v) = b.principal_address {
        am.principal_address = Set(Some(v));
    }
    if let Some(v) = b.mailing_address {
        am.mailing_address = Set(Some(v));
    }
    if let Some(v) = b.contact_name {
        am.contact_name = Set(Some(v));
    }
    if let Some(v) = b.contact_email {
        am.contact_email = Set(Some(v));
    }
    if let Some(v) = b.contact_phone {
        am.contact_phone = Set(Some(v));
    }
    if let Some(v) = b.website {
        am.website = Set(Some(v));
    }
    if let Some(v) = b.status {
        if v == "active" && !was_onboarded {
            am.onboarded_at = Set(Some(now.into()));
        }
        am.status = Set(v);
    }
    am.updated_at = Set(now.into());
    let saved = am.update(&state.property_db).await?;
    crate::audit::record(
        &state.user_db,
        Some(user.user_id),
        crate::audit::actions::LLC_UPDATE,
        Some("llc"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "status": saved.status })),
    )
    .await;
    Ok(Json(LlcResp::from(saved)))
}
