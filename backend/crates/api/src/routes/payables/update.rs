use super::dto::{UpdateVendorBillReq, VendorBillDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::payables::{sum_line_items, LineItem};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::VendorBill;
use rocket::serde::json::Json;
use rocket::{patch, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `PATCH /payables/<id>` — edit a bill while it is still a draft.
#[rocket_okapi::openapi(tag = "Payables")]
#[patch("/payables/<id>", data = "<body>")]
pub async fn update_payable(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateVendorBillReq>,
) -> ApiResult<Json<VendorBillDto>> {
    user.require(Permission::PayableManage)?;
    let bid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let bill = VendorBill::find_by_id(bid)
        .filter(entity::vendor_bill::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("vendor bill not found".into()))?;
    if bill.status != "draft" {
        return Err(ApiError::BadRequest(format!(
            "only draft bills are editable (status: {})",
            bill.status
        )));
    }
    let b = body.into_inner();
    let mut am: entity::vendor_bill::ActiveModel = bill.into();
    if let Some(memo) = b.memo.filter(|m| !m.trim().is_empty()) {
        am.memo = Set(memo);
    }
    if let Some(items) = b.line_items {
        if items.is_empty() {
            return Err(ApiError::BadRequest("line_items may not be empty".into()));
        }
        let total = sum_line_items(&items).map_err(ApiError::BadRequest)?;
        am.line_items = Set(serde_json::to_value(&items).unwrap_or_default());
        am.amount_cents = Set(total);
    } else if let Some(amount) = b.amount_cents {
        if amount <= 0 {
            return Err(ApiError::BadRequest("amount_cents must be positive".into()));
        }
        // Keep the one-line representation consistent with the new total.
        let memo = match &am.memo {
            sea_orm::ActiveValue::Set(m) => m.clone(),
            _ => "Services rendered".into(),
        };
        let items = vec![LineItem {
            description: memo,
            amount_cents: amount,
        }];
        am.line_items = Set(serde_json::to_value(&items).unwrap_or_default());
        am.amount_cents = Set(amount);
    }
    if let Some(due) = b.due_date {
        am.due_date = Set(Some(due).filter(|d| !d.trim().is_empty()));
    }
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::VENDOR_BILL_UPDATE,
        Some("vendor_bill"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "amount_cents": saved.amount_cents })),
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
