use super::dto::LeasePaymentDto;
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

/// `GET /leases/<id>/payments` — a lease's rent payment ledger.
#[rocket_okapi::openapi(tag = "Rentals")]
#[get("/leases/<id>/payments")]
pub async fn list_payments(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Vec<LeasePaymentDto>>> {
    user.require(Permission::LeaseRead)?;
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    Lease::find_by_id(lid)
        .filter(entity::lease::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("lease not found".into()))?;
    let rows = LeasePayment::find()
        .filter(entity::lease_payment::Column::LeaseId.eq(lid))
        .order_by_desc(entity::lease_payment::Column::DueDate)
        .all(&db)
        .await?;
    Ok(Json(rows.into_iter().map(LeasePaymentDto::from).collect()))
}
