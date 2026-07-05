use super::dto::PaymentMethodDto;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{Lease, PaymentMethod};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

/// `GET /leases/<id>/payment-methods` — the lease's saved methods (staff
/// view: tokens are never returned, only display metadata).
#[rocket_okapi::openapi(tag = "Payments")]
#[get("/leases/<id>/payment-methods")]
pub async fn lease_methods(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Vec<PaymentMethodDto>>> {
    user.require(Permission::PaymentRead)?;
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    Lease::find_by_id(lid)
        .filter(entity::lease::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("lease not found".into()))?;
    let rows = PaymentMethod::find()
        .filter(entity::payment_method::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::payment_method::Column::LeaseId.eq(lid))
        .filter(entity::payment_method::Column::Status.eq("active"))
        .order_by_asc(entity::payment_method::Column::CreatedAt)
        .all(&db)
        .await?;
    Ok(Json(rows.into_iter().map(PaymentMethodDto::from).collect()))
}
