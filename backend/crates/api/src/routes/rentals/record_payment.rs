use super::dto::{LeasePaymentDto, RecordPaymentReq};
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

/// `POST /leases/<id>/payments` — record a payment against a lease and update
/// the lease's outstanding balance and payment standing.
#[rocket_okapi::openapi(tag = "Rentals")]
#[post("/leases/<id>/payments", data = "<body>")]
pub async fn record_payment(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<RecordPaymentReq>,
) -> ApiResult<Json<LeasePaymentDto>> {
    user.require(Permission::LeaseManage)?;
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let lease = Lease::find_by_id(lid)
        .filter(entity::lease::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("lease not found".into()))?;
    let b = body.into_inner();
    let now = Utc::now();
    let status = match b.status {
        Some(s) if !s.trim().is_empty() => s,
        _ => "paid".to_string(),
    };
    let model = entity::lease_payment::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        lease_id: Set(lid),
        due_date: Set(b.due_date),
        amount_cents: Set(b.amount_cents),
        paid_date: Set(b.paid_date),
        status: Set(status.clone()),
        method: Set(b.method),
        created_at: Set(now.into()),
    };
    let saved = model.insert(&db).await?;

    // A settled payment draws down the lease's outstanding balance and may
    // restore the resident to current standing.
    if status == "paid" {
        let new_balance = (lease.balance_cents - saved.amount_cents).max(0);
        let payment_status = if new_balance <= 0 {
            "current"
        } else {
            "partial"
        };
        let mut am: entity::lease::ActiveModel = lease.into();
        am.balance_cents = Set(new_balance);
        am.payment_status = Set(payment_status.to_string());
        am.updated_at = Set(now.into());
        am.update(&db).await?;
    }

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::LEASE_PAYMENT_RECORD,
        Some("lease_payment"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "lease_id": saved.lease_id, "amount_cents": saved.amount_cents, "status": saved.status })),
    )
    .await;
    Ok(Json(LeasePaymentDto::from(saved)))
}
