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

/// `POST /payables/<id>/void` — cancel a bill that hasn't been approved yet.
/// Approved bills are already on the books and can only proceed to payment.
#[rocket_okapi::openapi(tag = "Payables")]
#[post("/payables/<id>/void")]
pub async fn void_payable(
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
    if !crate::payables::is_valid_transition(&bill.status, "void") {
        return Err(ApiError::BadRequest(format!(
            "bill cannot be voided (status: {})",
            bill.status
        )));
    }
    let mut am: entity::vendor_bill::ActiveModel = bill.into();
    am.status = Set("void".into());
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::VENDOR_BILL_VOID,
        Some("vendor_bill"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "bill_number": saved.bill_number })),
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
