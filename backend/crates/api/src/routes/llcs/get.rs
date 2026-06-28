//! `GET /llcs/<id>` — fetch one LLC's full onboarding profile.

use super::dto::LlcResp;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Llc;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

/// `GET /llcs/<id>` — fetch one holding entity.
#[rocket_okapi::openapi(tag = "LLCs")]
#[get("/llcs/<id>")]
pub async fn get(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<LlcResp>> {
    user.require(Permission::LlcRead)?;
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let llc = Llc::find_by_id(lid)
        .filter(entity::llc::Column::TenantId.eq(scope.tenant_id))
        .one(&state.property_db)
        .await?
        .ok_or_else(|| ApiError::NotFound("llc not found".into()))?;
    Ok(Json(LlcResp::from(llc)))
}
