use super::dto::{LeaseDto, UpdateLeaseReq};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::Lease;
use rocket::serde::json::Json;
use rocket::{patch, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `PATCH /leases/<id>` — update fields on a lease.
#[rocket_okapi::openapi(tag = "Rentals")]
#[patch("/leases/<id>", data = "<body>")]
pub async fn update_lease(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateLeaseReq>,
) -> ApiResult<Json<LeaseDto>> {
    user.require(Permission::LeaseManage)?;
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let existing = Lease::find_by_id(lid)
        .filter(entity::lease::Column::TenantId.eq(scope.tenant_id))
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("lease not found".into()))?;
    let b = body.into_inner();
    let mut am: entity::lease::ActiveModel = existing.into();
    if let Some(v) = b.unit_id {
        am.unit_id = Set(Some(v));
    }
    if let Some(v) = b.tenant_name {
        am.tenant_name = Set(v);
    }
    if let Some(v) = b.tenant_email {
        am.tenant_email = Set(Some(v));
    }
    if let Some(v) = b.tenant_phone {
        am.tenant_phone = Set(Some(v));
    }
    if let Some(v) = b.rent_cents {
        am.rent_cents = Set(v);
    }
    if let Some(v) = b.deposit_cents {
        am.deposit_cents = Set(Some(v));
    }
    if let Some(v) = b.start_date {
        am.start_date = Set(v);
    }
    if let Some(v) = b.end_date {
        am.end_date = Set(Some(v));
    }
    if let Some(v) = b.status {
        am.status = Set(v);
    }
    if let Some(v) = b.payment_status {
        am.payment_status = Set(v);
    }
    if let Some(v) = b.balance_cents {
        am.balance_cents = Set(v);
    }
    if let Some(v) = b.notes {
        am.notes = Set(Some(v));
    }
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&state.db).await?;
    crate::audit::record(
        &state.db,
        Some(user.user_id),
        crate::audit::actions::LEASE_UPDATE,
        Some("lease"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "status": saved.status, "payment_status": saved.payment_status })),
    )
    .await;
    Ok(Json(LeaseDto::from(saved)))
}
