use super::dto::{CreateVendorBillReq, VendorBillDto};
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::payables::NewBill;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use rocket::serde::json::Json;
use rocket::{post, State};
use uuid::Uuid;

/// `POST /payables` — create a draft vendor bill. Passing
/// `maintenance_ticket_id` prefills vendor / property / amount / memo from
/// the work order, so a completed ticket becomes a bill in one call.
#[rocket_okapi::openapi(tag = "Payables")]
#[post("/payables", data = "<body>")]
pub async fn create_payable(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<CreateVendorBillReq>,
) -> ApiResult<Json<VendorBillDto>> {
    user.require(Permission::PayableManage)?;
    let b = body.into_inner();
    let bill = crate::payables::create_bill(
        &db,
        scope.tenant_id,
        NewBill {
            counterparty_id: b.counterparty_id.unwrap_or(Uuid::nil()),
            entity_id: b.entity_id,
            property_id: b.property_id,
            maintenance_ticket_id: b.maintenance_ticket_id,
            memo: b.memo,
            line_items: b.line_items,
            amount_cents: b.amount_cents,
            due_date: b.due_date,
        },
        user.user_id,
    )
    .await?;
    let entities = crate::payouts::entity_names(&db, scope.tenant_id).await?;
    let vendors = crate::payables::vendor_names(&db, scope.tenant_id).await?;
    let entity_name = entities.get(&bill.entity_id).cloned();
    let vendor_name = vendors.get(&bill.counterparty_id).cloned();
    Ok(Json(VendorBillDto::from_model(
        bill,
        entity_name,
        vendor_name,
    )))
}
