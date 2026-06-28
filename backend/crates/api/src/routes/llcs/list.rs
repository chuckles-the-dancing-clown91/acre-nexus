//! `GET /llcs` — list holding entities for the active tenant.

use super::dto::LlcResp;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Llc;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

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
        .all(&state.property_db)
        .await?;
    Ok(Json(rows.into_iter().map(LlcResp::from).collect()))
}
