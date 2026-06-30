//! `GET /fees` — the active tenant's fee/discount/amenity schedule.

use super::dto::FeeDto;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::FeeSchedule;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

/// `GET /fees` — list the fee schedule.
#[rocket_okapi::openapi(tag = "Fee Schedule")]
#[get("/fees")]
pub async fn list(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<FeeDto>>> {
    user.require(Permission::FeeRead)?;
    let rows = FeeSchedule::find()
        .filter(entity::fee_schedule::Column::TenantId.eq(scope.tenant_id))
        .order_by_asc(entity::fee_schedule::Column::Label)
        .all(&state.db)
        .await?;
    Ok(Json(rows.into_iter().map(FeeDto::from).collect()))
}
