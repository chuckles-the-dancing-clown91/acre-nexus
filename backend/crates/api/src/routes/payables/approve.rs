use super::dto::VendorBillDto;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::VendorBill;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `POST /payables/<id>/approve` — approve a submitted bill. Approval accrues
/// the expense to the owning entity's books
/// (`Dr Property Expenses / Cr Accounts Payable`), so the cost is recognized
/// the moment the obligation is committed — payment later just clears the
/// liability.
#[rocket_okapi::openapi(tag = "Payables")]
#[post("/payables/<id>/approve")]
pub async fn approve_payable(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<VendorBillDto>> {
    user.require(Permission::PayableApprove)?;
    let bid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let bill = VendorBill::find_by_id(bid)
        .filter(entity::vendor_bill::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("vendor bill not found".into()))?;
    if !crate::payables::is_valid_transition(&bill.status, "approved") {
        return Err(ApiError::BadRequest(format!(
            "bill is not approvable (status: {})",
            bill.status
        )));
    }

    // Accrue first: if the posting is rejected (unbalanced books, missing
    // entity), the approval fails with it — the request transaction rolls
    // both back together.
    let now = Utc::now();
    let today = now.date_naive().to_string();
    let txn = crate::accounting::post_vendor_bill_approved(
        &db,
        scope.tenant_id,
        bill.entity_id,
        bill.property_id,
        &today,
        bill.amount_cents,
        bill.id,
        Some(user.user_id),
    )
    .await?;

    let mut am: entity::vendor_bill::ActiveModel = bill.into();
    am.status = Set("approved".into());
    am.approved_by = Set(Some(user.user_id));
    am.approved_at = Set(Some(now.into()));
    am.accrual_txn_id = Set(Some(txn.id));
    am.updated_at = Set(now.into());
    let saved = am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::VENDOR_BILL_APPROVE,
        Some("vendor_bill"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({
            "bill_number": saved.bill_number,
            "amount_cents": saved.amount_cents,
            "accrual_txn_id": txn.id,
        })),
    )
    .await;

    let entities = crate::payouts::entity_names(&db, scope.tenant_id).await?;
    let vendors = crate::payables::vendor_names(&db, scope.tenant_id).await?;
    let entity_name = entities.get(&saved.entity_id).cloned();
    let vendor_name = vendors.get(&saved.counterparty_id).cloned();
    Ok(Json(VendorBillDto::from_model(
        saved,
        entity_name,
        vendor_name,
    )))
}
