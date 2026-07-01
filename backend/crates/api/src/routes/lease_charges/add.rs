//! `POST /leases/<id>/charges` — add a manual line item (fee/discount/amenity).

use super::dto::{AddChargeReq, ChargeDto};
use super::signed_amount;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::Lease;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

const KINDS: &[&str] = &["fee", "discount", "rebate", "amenity"];

/// `POST /leases/<id>/charges` — add a charge to a lease.
#[rocket_okapi::openapi(tag = "Lease Charges")]
#[post("/leases/<id>/charges", data = "<body>")]
pub async fn add(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<AddChargeReq>,
) -> ApiResult<Json<ChargeDto>> {
    user.require(Permission::LeaseManage)?;
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    Lease::find_by_id(lid)
        .filter(entity::lease::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("lease not found".into()))?;
    let b = body.into_inner();
    if !KINDS.contains(&b.kind.as_str()) {
        return Err(ApiError::BadRequest(format!("invalid kind: {}", b.kind)));
    }
    let saved = entity::lease_charge::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        lease_id: Set(lid),
        kind: Set(b.kind.clone()),
        code: Set(b.code),
        label: Set(b.label),
        amount_cents: Set(signed_amount(&b.kind, b.amount_cents)),
        recurring: Set(b.recurring.unwrap_or(true)),
        source: Set("manual".into()),
        verbiage: Set(b.verbiage),
        created_at: Set(Utc::now().into()),
    }
    .insert(&db)
    .await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::LEASE_CHARGE_ADD,
        Some("lease_charge"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "lease_id": lid, "kind": saved.kind, "amount_cents": saved.amount_cents })),
    )
    .await;
    Ok(Json(ChargeDto::from(saved)))
}
