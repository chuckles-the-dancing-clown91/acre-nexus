use super::dto::DealDto;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Deal;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

/// `GET /modules/flips/deals?<stage>&<strategy>` — the tenant's deals, newest
/// first, each with its computed underwriting. Optionally filter by stage or
/// strategy.
#[rocket_okapi::openapi(tag = "Flips")]
#[get("/modules/flips/deals?<stage>&<strategy>")]
pub async fn list(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    stage: Option<String>,
    strategy: Option<String>,
) -> ApiResult<Json<Vec<DealDto>>> {
    user.require(Permission::DealRead)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "flips").await?;

    let mut q = Deal::find().filter(entity::deal::Column::TenantId.eq(scope.tenant_id));
    if let Some(s) = stage.filter(|s| !s.is_empty()) {
        q = q.filter(entity::deal::Column::Stage.eq(s));
    }
    if let Some(s) = strategy.filter(|s| !s.is_empty()) {
        q = q.filter(entity::deal::Column::Strategy.eq(s));
    }
    let rows = q
        .order_by_desc(entity::deal::Column::CreatedAt)
        .all(&db)
        .await?;
    Ok(Json(rows.iter().map(DealDto::build).collect()))
}
