//! `GET /leases/<id>/charges` — a lease's line items + computed monthly total.

use super::dto::{ChargeDto, ChargesResp};
use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::{ApiError, ApiResult};
use crate::leasedoc::monthly_total_cents;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{Lease, LeaseCharge};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

/// `GET /leases/<id>/charges` — list the charges on a lease.
#[rocket_okapi::openapi(tag = "Lease Charges")]
#[get("/leases/<id>/charges")]
pub async fn list(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<ChargesResp>> {
    user.require(Permission::LeaseRead)?;
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let lease = Lease::find_by_id(lid)
        .filter(entity::lease::Column::TenantId.eq(scope.tenant_id))
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("lease not found".into()))?;
    let charges = LeaseCharge::find()
        .filter(entity::lease_charge::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::lease_charge::Column::LeaseId.eq(lid))
        .order_by_asc(entity::lease_charge::Column::CreatedAt)
        .all(&state.db)
        .await?;
    let total = monthly_total_cents(&lease, &charges);
    Ok(Json(ChargesResp {
        charges: charges.into_iter().map(ChargeDto::from).collect(),
        base_rent_cents: lease.rent_cents,
        base_rent_label: usd(lease.rent_cents),
        monthly_total_cents: total,
        monthly_total_label: usd(total),
    }))
}
