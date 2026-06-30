//! `POST /llcs` — create a holding entity.

use super::dto::{CreateLlcReq, LlcResp};
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, Set};
use uuid::Uuid;

/// `POST /llcs` — create a holding entity.
#[rocket_okapi::openapi(tag = "LLCs")]
#[post("/llcs", data = "<body>")]
pub async fn create(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    body: Json<CreateLlcReq>,
) -> ApiResult<Json<LlcResp>> {
    user.require(Permission::TenantManage)?;
    let b = body.into_inner();
    let model = entity::llc::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        name: Set(b.name),
        ein: Set(b.ein.unwrap_or_default()),
        state: Set(b.state.unwrap_or_default()),
        entity_type: Set(b.entity_type.unwrap_or_else(|| "llc".into())),
        registered_agent: Set(b.registered_agent),
        status: Set("active".into()),
        created_at: Set(Utc::now().into()),
    };
    let saved = model.insert(&state.db).await?;
    crate::audit::record(
        &state.db,
        Some(user.user_id),
        crate::audit::actions::LLC_CREATE,
        Some("llc"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "name": saved.name })),
    )
    .await;
    Ok(Json(LlcResp::from(saved)))
}
