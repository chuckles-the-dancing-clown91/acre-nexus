use super::dto::VendorBillDto;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::VendorBill;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

/// `GET /payables?status=&counterparty=` — vendor bills, newest first.
#[rocket_okapi::openapi(tag = "Payables")]
#[get("/payables?<status>&<counterparty>")]
pub async fn list_payables(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    status: Option<&str>,
    counterparty: Option<&str>,
) -> ApiResult<Json<Vec<VendorBillDto>>> {
    user.require(Permission::PayableRead)?;
    let mut q =
        VendorBill::find().filter(entity::vendor_bill::Column::TenantId.eq(scope.tenant_id));
    if let Some(s) = status.filter(|s| !s.is_empty()) {
        if !crate::payables::STATUSES.contains(&s) {
            return Err(crate::error::ApiError::BadRequest(format!(
                "invalid status '{s}' (expected one of {})",
                crate::payables::STATUSES.join(", ")
            )));
        }
        q = q.filter(entity::vendor_bill::Column::Status.eq(s));
    }
    if let Some(c) = counterparty.and_then(|c| Uuid::parse_str(c).ok()) {
        q = q.filter(entity::vendor_bill::Column::CounterpartyId.eq(c));
    }
    let rows = q
        .order_by_desc(entity::vendor_bill::Column::CreatedAt)
        .limit(200)
        .all(&db)
        .await?;
    let entities = crate::payouts::entity_names(&db, scope.tenant_id).await?;
    let vendors = crate::payables::vendor_names(&db, scope.tenant_id).await?;
    Ok(Json(
        rows.into_iter()
            .map(|b| {
                let entity_name = entities.get(&b.entity_id).cloned();
                let vendor_name = vendors.get(&b.counterparty_id).cloned();
                VendorBillDto::from_model(b, entity_name, vendor_name)
            })
            .collect(),
    ))
}
