use super::dto::{RejectVendorBillReq, VendorBillDto};
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

/// `POST /payables/<id>/reject` — send a submitted bill back to draft with a
/// reason for the submitter to address.
#[rocket_okapi::openapi(tag = "Payables")]
#[post("/payables/<id>/reject", data = "<body>")]
pub async fn reject_payable(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<RejectVendorBillReq>,
) -> ApiResult<Json<VendorBillDto>> {
    user.require(Permission::PayableApprove)?;
    let bid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let bill = VendorBill::find_by_id(bid)
        .filter(entity::vendor_bill::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("vendor bill not found".into()))?;
    if bill.status != "submitted" {
        return Err(ApiError::BadRequest(format!(
            "only submitted bills can be rejected (status: {})",
            bill.status
        )));
    }
    let reason = body
        .into_inner()
        .reason
        .filter(|r| !r.trim().is_empty())
        .unwrap_or_else(|| "returned for changes".into());
    let mut am: entity::vendor_bill::ActiveModel = bill.into();
    am.status = Set("draft".into());
    am.rejected_reason = Set(Some(reason.clone()));
    am.submitted_by = Set(None);
    am.submitted_at = Set(None);
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::VENDOR_BILL_REJECT,
        Some("vendor_bill"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "reason": reason })),
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
