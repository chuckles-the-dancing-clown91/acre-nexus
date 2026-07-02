//! `GET /vehicles?lease_id=&application_id=&user_id=` — resident vehicles,
//! optionally filtered to a lease, an application, or a person (the profile
//! vehicles staff manage on a renter's behalf).

use super::dto::VehicleDto;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Vehicle;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

/// `GET /vehicles` — list vehicles for the tenant (optionally scoped).
#[rocket_okapi::openapi(tag = "Vehicles")]
#[get("/vehicles?<lease_id>&<application_id>&<user_id>")]
pub async fn list(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    lease_id: Option<String>,
    application_id: Option<String>,
    user_id: Option<String>,
) -> ApiResult<Json<Vec<VehicleDto>>> {
    user.require(Permission::VehicleRead)?;
    let mut q = Vehicle::find().filter(entity::vehicle::Column::TenantId.eq(scope.tenant_id));
    if let Some(l) = lease_id.as_deref().and_then(|s| Uuid::parse_str(s).ok()) {
        q = q.filter(entity::vehicle::Column::LeaseId.eq(l));
    }
    if let Some(a) = application_id
        .as_deref()
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        q = q.filter(entity::vehicle::Column::ApplicationId.eq(a));
    }
    if let Some(u) = user_id.as_deref().and_then(|s| Uuid::parse_str(s).ok()) {
        q = q.filter(entity::vehicle::Column::UserId.eq(u));
    }
    let rows = q
        .order_by_desc(entity::vehicle::Column::CreatedAt)
        .all(&db)
        .await?;
    Ok(Json(rows.into_iter().map(VehicleDto::from).collect()))
}
