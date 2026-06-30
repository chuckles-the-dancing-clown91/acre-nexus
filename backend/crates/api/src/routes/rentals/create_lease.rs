use super::dto::{CreateLeaseReq, LeaseDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::Property;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `POST /properties/<id>/leases` — create a lease on a property.
#[rocket_okapi::openapi(tag = "Rentals")]
#[post("/properties/<id>/leases", data = "<body>")]
pub async fn create_lease(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<CreateLeaseReq>,
) -> ApiResult<Json<LeaseDto>> {
    user.require(Permission::LeaseManage)?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    let b = body.into_inner();
    let now = Utc::now();
    let status = match b.status {
        Some(s) if !s.trim().is_empty() => s,
        _ => "active".to_string(),
    };
    let payment_status = match b.payment_status {
        Some(s) if !s.trim().is_empty() => s,
        _ => "current".to_string(),
    };
    let model = entity::lease::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        property_id: Set(pid),
        unit_id: Set(b.unit_id),
        application_id: Set(None),
        tenant_name: Set(b.tenant_name),
        tenant_email: Set(b.tenant_email),
        tenant_phone: Set(b.tenant_phone),
        rent_cents: Set(b.rent_cents),
        deposit_cents: Set(b.deposit_cents),
        start_date: Set(b.start_date),
        end_date: Set(b.end_date),
        status: Set(status),
        payment_status: Set(payment_status),
        balance_cents: Set(0),
        has_pet: Set(b.has_pet.unwrap_or(false)),
        pet_details: Set(b.pet_details.clone()),
        is_military: Set(b.is_military.unwrap_or(false)),
        notes: Set(b.notes),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    };
    let saved = model.insert(&state.db).await?;
    // Reflect the new tenancy on the property + unit immediately.
    crate::rentals_occupancy::sync_property_occupancy(&state.db, pid).await;
    crate::audit::record(
        &state.db,
        Some(user.user_id),
        crate::audit::actions::LEASE_CREATE,
        Some("lease"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "property_id": saved.property_id, "tenant_name": saved.tenant_name, "status": saved.status })),
    )
    .await;
    Ok(Json(LeaseDto::from(saved)))
}
