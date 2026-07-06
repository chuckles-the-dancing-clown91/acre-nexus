use super::dto::VendorBillDto;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::VendorBill;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

/// `POST /payables/<id>/pay` — execute an approved bill's payment as an ACH
/// transfer through the payments provider (sandbox by default). Settlement
/// clears the liability on the ledger and updates the linked work order.
#[rocket_okapi::openapi(tag = "Payables")]
#[post("/payables/<id>/pay")]
pub async fn pay_payable(
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
    let saved = crate::payables::pay_bill(&db, scope.tenant_id, bill, user.user_id).await?;

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
