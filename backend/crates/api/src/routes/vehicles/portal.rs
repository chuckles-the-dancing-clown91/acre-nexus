//! `/my/vehicles` — the signed-in person's own vehicles, self-service. These
//! are the master records the white-glove application flow snapshots onto each
//! application (properties need them for parking, garage amenities, and lease
//! verbiage). No staff permission required — everything is scoped to the
//! caller's `user_id`.

use super::dto::{CreateVehicleReq, VehicleDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::Vehicle;
use rocket::serde::json::Json;
use rocket::{delete, get, post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, ModelTrait, QueryFilter, Set};
use uuid::Uuid;

/// `GET /my/vehicles` — the signed-in user's vehicles.
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[get("/my/vehicles")]
pub async fn my_vehicles(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    _scope: TenantScope,
) -> ApiResult<Json<Vec<VehicleDto>>> {
    let rows = crate::routes::iam::self_profile::own_vehicles(&db, user.user_id).await?;
    Ok(Json(rows.into_iter().map(VehicleDto::from).collect()))
}

/// `POST /my/vehicles` — add one of my vehicles (lease/application links and
/// `user_id` in the body are ignored; the row is always mine).
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[post("/my/vehicles", data = "<body>")]
pub async fn add_my_vehicle(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<CreateVehicleReq>,
) -> ApiResult<Json<VehicleDto>> {
    let b = body.into_inner();
    if b.make.trim().is_empty() || b.model.trim().is_empty() {
        return Err(ApiError::BadRequest("make and model are required".into()));
    }
    let now = Utc::now();
    let saved = entity::vehicle::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        lease_id: Set(None),
        application_id: Set(None),
        user_id: Set(Some(user.user_id)),
        make: Set(b.make),
        model: Set(b.model),
        year: Set(b.year),
        color: Set(b.color),
        license_plate: Set(b.license_plate),
        plate_state: Set(b.plate_state),
        notes: Set(b.notes),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::VEHICLE_CREATE,
        Some("vehicle"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "make": saved.make, "model": saved.model, "self_service": true })),
    )
    .await;
    Ok(Json(VehicleDto::from(saved)))
}

/// `DELETE /my/vehicles/<id>` — remove one of my vehicles.
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[delete("/my/vehicles/<id>")]
pub async fn delete_my_vehicle(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    let vid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let v = Vehicle::find_by_id(vid)
        .filter(entity::vehicle::Column::UserId.eq(user.user_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("vehicle not found".into()))?;
    v.delete(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::VEHICLE_DELETE,
        Some("vehicle"),
        Some(vid.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "self_service": true })),
    )
    .await;
    Ok(Json(serde_json::json!({ "deleted": true })))
}
