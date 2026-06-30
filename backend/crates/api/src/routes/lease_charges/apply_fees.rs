//! `POST /leases/<id>/apply-fees` — evaluate the tenant's fee schedule against a
//! lease's attributes (pets, military, vehicles) and auto-create the matching
//! fees, discounts, and amenities. Idempotent per fee `code` (won't double-apply).

use super::dto::{ApplyFeesResp, ChargeDto};
use super::signed_amount;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{FeeSchedule, Lease, LeaseCharge, Vehicle};
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use std::collections::HashSet;
use uuid::Uuid;

/// Does `condition_type` match this lease's attributes?
fn condition_matches(condition: &str, lease: &entity::lease::Model, has_vehicle: bool) -> bool {
    match condition {
        "always" => true,
        "has_pet" => lease.has_pet,
        "is_military" => lease.is_military,
        "has_vehicle" => has_vehicle,
        _ => false, // `manual` and unknown conditions never auto-apply
    }
}

/// Apply the active fee schedule to a lease, creating auto charges for every
/// matching condition whose `code` is not already present. Returns the new rows.
/// Shared by this endpoint and the application→lease conversion.
pub async fn apply_to_lease<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    lease: &entity::lease::Model,
) -> Result<Vec<entity::lease_charge::Model>, ApiError> {
    let fees = FeeSchedule::find()
        .filter(entity::fee_schedule::Column::TenantId.eq(tenant_id))
        .filter(entity::fee_schedule::Column::Active.eq(true))
        .all(db)
        .await?;
    let existing: HashSet<String> = LeaseCharge::find()
        .filter(entity::lease_charge::Column::TenantId.eq(tenant_id))
        .filter(entity::lease_charge::Column::LeaseId.eq(lease.id))
        .all(db)
        .await?
        .into_iter()
        .filter_map(|c| c.code)
        .collect();
    let has_vehicle = Vehicle::find()
        .filter(entity::vehicle::Column::TenantId.eq(tenant_id))
        .filter(entity::vehicle::Column::LeaseId.eq(lease.id))
        .one(db)
        .await?
        .is_some();

    let mut created = Vec::new();
    let now = Utc::now();
    for fee in fees {
        if !condition_matches(&fee.condition_type, lease, has_vehicle) {
            continue;
        }
        if existing.contains(&fee.code) {
            continue;
        }
        let saved = entity::lease_charge::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            lease_id: Set(lease.id),
            kind: Set(fee.kind.clone()),
            code: Set(Some(fee.code.clone())),
            label: Set(fee.label.clone()),
            amount_cents: Set(signed_amount(&fee.kind, fee.amount_cents)),
            recurring: Set(fee.recurring),
            source: Set("auto".into()),
            verbiage: Set(fee.verbiage.clone()),
            created_at: Set(now.into()),
        }
        .insert(db)
        .await?;
        created.push(saved);
    }
    Ok(created)
}

/// `POST /leases/<id>/apply-fees` — auto-apply the conditional fee schedule.
#[rocket_okapi::openapi(tag = "Lease Charges")]
#[post("/leases/<id>/apply-fees")]
pub async fn apply_fees(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<ApplyFeesResp>> {
    user.require(Permission::LeaseManage)?;
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let lease = Lease::find_by_id(lid)
        .filter(entity::lease::Column::TenantId.eq(scope.tenant_id))
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("lease not found".into()))?;
    let created = apply_to_lease(&state.db, scope.tenant_id, &lease).await?;
    crate::audit::record(
        &state.db,
        Some(user.user_id),
        crate::audit::actions::LEASE_FEES_APPLY,
        Some("lease"),
        Some(lid.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "applied": created.len() })),
    )
    .await;
    Ok(Json(ApplyFeesResp {
        applied: created.len(),
        charges: created.into_iter().map(ChargeDto::from).collect(),
    }))
}
