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

/// `POST /payables/<id>/submit` — hand a draft bill to the approvers.
#[rocket_okapi::openapi(tag = "Payables")]
#[post("/payables/<id>/submit")]
pub async fn submit_payable(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<VendorBillDto>> {
    user.require(Permission::PayableManage)?;
    let bid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let bill = VendorBill::find_by_id(bid)
        .filter(entity::vendor_bill::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("vendor bill not found".into()))?;
    if !crate::payables::is_valid_transition(&bill.status, "submitted") {
        return Err(ApiError::BadRequest(format!(
            "bill is not submittable (status: {})",
            bill.status
        )));
    }
    if bill.amount_cents <= 0 {
        return Err(ApiError::BadRequest(
            "bill amount must be positive to submit".into(),
        ));
    }
    let now = Utc::now();
    let mut am: entity::vendor_bill::ActiveModel = bill.into();
    am.status = Set("submitted".into());
    am.submitted_by = Set(Some(user.user_id));
    am.submitted_at = Set(Some(now.into()));
    am.rejected_reason = Set(None);
    am.updated_at = Set(now.into());
    let saved = am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::VENDOR_BILL_SUBMIT,
        Some("vendor_bill"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({
            "bill_number": saved.bill_number,
            "amount_cents": saved.amount_cents,
        })),
    )
    .await;

    let vendors = crate::payables::vendor_names(&db, scope.tenant_id).await?;
    let vendor_name = vendors.get(&saved.counterparty_id).cloned();

    // Everyone who can approve hears about it (the submitter is excluded —
    // they know).
    crate::notify::notify_staff(
        &db,
        scope.tenant_id,
        "payable:approve",
        "vendor_bill_submitted",
        serde_json::json!({
            "bill_number": saved.bill_number,
            "vendor": vendor_name.clone().unwrap_or_else(|| "a vendor".into()),
            "amount": crate::dto::usd(saved.amount_cents),
        }),
        Some(("vendor_bill", saved.id)),
        "submitted",
        Some(user.user_id),
    )
    .await;

    let entities = crate::payouts::entity_names(&db, scope.tenant_id).await?;
    let entity_name = entities.get(&saved.entity_id).cloned();
    Ok(Json(VendorBillDto::from_model(
        saved,
        entity_name,
        vendor_name,
    )))
}
