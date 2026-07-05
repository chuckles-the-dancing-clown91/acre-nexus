use super::dto::PaymentDto;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::LeasePayment;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

const MAX_ROWS: u64 = 200;

/// `GET /payments?status=&lease=&limit=` — tenant-wide payment activity for
/// the back office, newest first.
#[rocket_okapi::openapi(tag = "Payments")]
#[get("/payments?<status>&<lease>&<limit>")]
pub async fn list_payments(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    status: Option<String>,
    lease: Option<String>,
    limit: Option<u64>,
) -> ApiResult<Json<Vec<PaymentDto>>> {
    user.require(Permission::PaymentRead)?;
    let mut q =
        LeasePayment::find().filter(entity::lease_payment::Column::TenantId.eq(scope.tenant_id));
    if let Some(status) = status.filter(|s| !s.trim().is_empty()) {
        q = q.filter(entity::lease_payment::Column::Status.eq(status));
    }
    if let Some(lease) = lease.and_then(|l| uuid::Uuid::parse_str(&l).ok()) {
        q = q.filter(entity::lease_payment::Column::LeaseId.eq(lease));
    }
    let rows = q
        .order_by_desc(entity::lease_payment::Column::CreatedAt)
        .limit(limit.unwrap_or(100).min(MAX_ROWS))
        .all(&db)
        .await?;
    Ok(Json(rows.into_iter().map(PaymentDto::from).collect()))
}
