use super::dto::VendorBillDto;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{Counterparty, Llc, VendorBill};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

/// `GET /payables/<id>` — one vendor bill.
#[rocket_okapi::openapi(tag = "Payables")]
#[get("/payables/<id>")]
pub async fn get_payable(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<VendorBillDto>> {
    user.require(Permission::PayableRead)?;
    let bid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let bill = VendorBill::find_by_id(bid)
        .filter(entity::vendor_bill::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("vendor bill not found".into()))?;
    let entity_name = Llc::find_by_id(bill.entity_id)
        .filter(entity::llc::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .map(|l| l.name);
    let vendor_name = Counterparty::find_by_id(bill.counterparty_id)
        .filter(entity::counterparty::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .map(|c| c.name);
    Ok(Json(VendorBillDto::from_model(
        bill,
        entity_name,
        vendor_name,
    )))
}
