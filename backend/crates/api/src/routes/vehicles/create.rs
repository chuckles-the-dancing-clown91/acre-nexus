//! `POST /vehicles` — add a resident vehicle profile.

use super::dto::{CreateVehicleReq, VehicleDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, Set};
use uuid::Uuid;

/// `POST /vehicles` — create a vehicle.
#[rocket_okapi::openapi(tag = "Vehicles")]
#[post("/vehicles", data = "<body>")]
pub async fn create(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    body: Json<CreateVehicleReq>,
) -> ApiResult<Json<VehicleDto>> {
    user.require(Permission::VehicleManage)?;
    let b = body.into_inner();
    if b.make.trim().is_empty() || b.model.trim().is_empty() {
        return Err(ApiError::BadRequest("make and model are required".into()));
    }
    let now = Utc::now();
    let saved = entity::vehicle::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        lease_id: Set(b.lease_id),
        application_id: Set(b.application_id),
        user_id: Set(b.user_id),
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
    .insert(&state.db)
    .await?;
    crate::audit::record(
        &state.db,
        Some(user.user_id),
        crate::audit::actions::VEHICLE_CREATE,
        Some("vehicle"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "make": saved.make, "model": saved.model })),
    )
    .await;
    Ok(Json(VehicleDto::from(saved)))
}
