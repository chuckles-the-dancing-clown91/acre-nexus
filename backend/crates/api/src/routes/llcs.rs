//! LLC (holding-entity) endpoints — tenant-scoped.

use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::Llc;
use rocket::serde::json::Json;
use rocket::{get, post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct LlcResp {
    pub id: Uuid,
    pub name: String,
    pub ein: String,
    pub state: String,
}

impl From<entity::llc::Model> for LlcResp {
    fn from(l: entity::llc::Model) -> Self {
        LlcResp {
            id: l.id,
            name: l.name,
            ein: l.ein,
            state: l.state,
        }
    }
}

/// `GET /llcs` — list holding entities for the active tenant.
#[rocket_okapi::openapi(tag = "LLCs")]
#[get("/llcs")]
pub async fn list(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<LlcResp>>> {
    user.require(Permission::PropertyRead)?;
    let rows = Llc::find()
        .filter(entity::llc::Column::TenantId.eq(scope.tenant_id))
        .order_by_asc(entity::llc::Column::Name)
        .all(&state.db)
        .await?;
    Ok(Json(rows.into_iter().map(LlcResp::from).collect()))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateLlcReq {
    pub name: String,
    pub ein: Option<String>,
    pub state: Option<String>,
}

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
