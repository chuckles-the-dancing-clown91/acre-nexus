//! `PATCH /vehicles/<id>` — edit a vehicle (e.g. attach to a lease at signing).

use super::dto::{UpdateVehicleReq, VehicleDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::Vehicle;
use rocket::serde::json::Json;
use rocket::{patch, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `PATCH /vehicles/<id>` — update a vehicle.
#[rocket_okapi::openapi(tag = "Vehicles")]
#[patch("/vehicles/<id>", data = "<body>")]
pub async fn update(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateVehicleReq>,
) -> ApiResult<Json<VehicleDto>> {
    user.require(Permission::VehicleManage)?;
    let vid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let existing = Vehicle::find_by_id(vid)
        .filter(entity::vehicle::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("vehicle not found".into()))?;
    let b = body.into_inner();
    // A re-pointed lease must belong to this tenant.
    super::assert_links_in_tenant(&db, scope.tenant_id, b.lease_id, None).await?;
    let mut am: entity::vehicle::ActiveModel = existing.into();
    if let Some(v) = b.lease_id {
        am.lease_id = Set(Some(v));
    }
    if let Some(v) = b.make {
        am.make = Set(v);
    }
    if let Some(v) = b.model {
        am.model = Set(v);
    }
    if let Some(v) = b.year {
        am.year = Set(Some(v));
    }
    if let Some(v) = b.color {
        am.color = Set(Some(v));
    }
    if let Some(v) = b.license_plate {
        am.license_plate = Set(Some(v));
    }
    if let Some(v) = b.plate_state {
        am.plate_state = Set(Some(v));
    }
    if let Some(v) = b.notes {
        am.notes = Set(Some(v));
    }
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::VEHICLE_UPDATE,
        Some("vehicle"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        None,
    )
    .await;
    Ok(Json(VehicleDto::from(saved)))
}
