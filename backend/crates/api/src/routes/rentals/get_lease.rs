use super::dto::{LeaseDetailDto, LeaseDto, LeasePaymentDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{Lease, LeasePayment};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

/// `GET /leases/<id>` — a lease with its full payment ledger.
#[rocket_okapi::openapi(tag = "Rentals")]
#[get("/leases/<id>")]
pub async fn get_lease(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<LeaseDetailDto>> {
    user.require(Permission::LeaseRead)?;
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let lease = Lease::find_by_id(lid)
        .filter(entity::lease::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("lease not found".into()))?;
    let payments = LeasePayment::find()
        .filter(entity::lease_payment::Column::LeaseId.eq(lid))
        .order_by_desc(entity::lease_payment::Column::DueDate)
        .all(&db)
        .await?
        .into_iter()
        .map(LeasePaymentDto::from)
        .collect();
    Ok(Json(LeaseDetailDto {
        lease: LeaseDto::from(lease),
        payments,
    }))
}
