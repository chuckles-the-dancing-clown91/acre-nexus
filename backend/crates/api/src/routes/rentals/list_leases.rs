use super::dto::LeaseDto;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Lease;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

/// `GET /leases?status=&property_id=` — list leases in the active tenant.
#[rocket_okapi::openapi(tag = "Rentals")]
#[get("/leases?<status>&<property_id>")]
pub async fn list_leases(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    status: Option<String>,
    property_id: Option<String>,
) -> ApiResult<Json<Vec<LeaseDto>>> {
    user.require(Permission::LeaseRead)?;
    let mut query = Lease::find().filter(entity::lease::Column::TenantId.eq(scope.tenant_id));
    if let Some(s) = status.filter(|s| !s.is_empty()) {
        query = query.filter(entity::lease::Column::Status.eq(s));
    }
    if let Some(pid) = property_id.as_deref().and_then(|s| Uuid::parse_str(s).ok()) {
        query = query.filter(entity::lease::Column::PropertyId.eq(pid));
    }
    let rows = query
        .order_by_desc(entity::lease::Column::CreatedAt)
        .all(&state.db)
        .await?;
    Ok(Json(rows.into_iter().map(LeaseDto::from).collect()))
}
