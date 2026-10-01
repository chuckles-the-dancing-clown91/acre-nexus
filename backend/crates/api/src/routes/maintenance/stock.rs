//! **Stock** (Vantedge phase 2C): scan-in and the movements behind every
//! quantity — Alpha's inventory, translated.
//!
//! * `GET /inventory/lookup?code=` — a barcode / UPC / SKU → the item (404 when
//!   unknown, so the receive flow can offer "new item").
//! * `POST /inventory/receive` — a delivery: lines of existing or new items
//!   with quantity and unit cost; tax and shipping spread across the lines by
//!   value (the *landed* cost); each item's **unit cost becomes the weighted
//!   average** of what's on the shelf and what came in; one expense records
//!   the purchase.
//! * `POST /inventory/<id>/count` — a physical count writes the difference.
//! * `GET /inventory/movements` — the ledger of receive / use / count.
//! * `GET /inventory/reorder` — what's at or under its reorder level, by vendor
//!   (the Friday email uses it too).

use super::dto::InventoryItemDto;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::routes::team::parse_id;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{InventoryItem, InventoryMovement, MaintenanceTicket, User};
use rocket::serde::json::Json;
use rocket::{get, post};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use uuid::Uuid;

/// Write one movement row.
#[allow(clippy::too_many_arguments)]
pub async fn record_movement(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    item_id: Uuid,
    kind: &str,
    quantity: i32,
    unit_cost_cents: i64,
    ticket_id: Option<Uuid>,
    expense_id: Option<Uuid>,
    note: Option<&str>,
    recorded_by: Option<Uuid>,
) -> ApiResult<entity::inventory_movement::Model> {
    Ok(entity::inventory_movement::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        inventory_item_id: Set(item_id),
        kind: Set(kind.into()),
        quantity: Set(quantity),
        unit_cost_cents: Set(unit_cost_cents),
        ticket_id: Set(ticket_id),
        expense_id: Set(expense_id),
        note: Set(note.map(str::to_string)),
        recorded_by: Set(recorded_by),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await?)
}

/// The new weighted-average unit cost after `qty` arrive at `landed` each.
pub fn weighted_average(on_hand: i32, avg_cents: i64, qty: i32, landed_cents: i64) -> i64 {
    let base = on_hand.max(0) as i64;
    let total = base + qty as i64;
    if total <= 0 {
        return landed_cents;
    }
    crate::workforce::overtime::div_round(base * avg_cents + qty as i64 * landed_cents, total)
}

/// Spread `extra` (tax + shipping) across lines in proportion to their value,
/// returning each line's landed unit cost. The last line takes the rounding.
pub fn landed_costs(lines: &[(i32, i64)], extra_cents: i64) -> Vec<i64> {
    let subtotal: i64 = lines.iter().map(|(q, c)| *q as i64 * c).sum();
    if subtotal <= 0 || extra_cents <= 0 {
        return lines.iter().map(|(_, c)| *c).collect();
    }
    let mut out = Vec::with_capacity(lines.len());
    let mut spread = 0;
    for (i, (q, c)) in lines.iter().enumerate() {
        let value = *q as i64 * c;
        let share = if i + 1 == lines.len() {
            extra_cents - spread
        } else {
            crate::workforce::overtime::div_round(extra_cents * value, subtotal)
        };
        spread += share;
        out.push(c + crate::workforce::overtime::div_round(share, *q as i64));
    }
    out
}

/// `GET /inventory/lookup?code=` — find a stock item by barcode or SKU.
#[rocket_okapi::openapi(tag = "Stock")]
#[get("/inventory/lookup?<code>")]
pub async fn lookup(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    code: String,
) -> ApiResult<Json<InventoryItemDto>> {
    user.require(Permission::MaintenanceRead)?;
    let code = code.trim().to_string();
    if code.is_empty() {
        return Err(ApiError::BadRequest("scan or type a code".into()));
    }
    let item = InventoryItem::find()
        .filter(entity::inventory_item::Column::TenantId.eq(scope.tenant_id))
        .filter(
            sea_orm::Condition::any()
                .add(entity::inventory_item::Column::Barcode.eq(code.clone()))
                .add(entity::inventory_item::Column::Sku.eq(code.clone())),
        )
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("no stock item has the code {code}")))?;
    Ok(Json(InventoryItemDto::from(item)))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct NewItem {
    pub name: String,
    pub barcode: Option<String>,
    pub sku: Option<String>,
    pub category: Option<String>,
    pub unit: Option<String>,
    pub reorder_level: Option<i32>,
    pub storage_location: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ReceiveLine {
    pub inventory_item_id: Option<Uuid>,
    /// Instead of an id: create the item as part of the delivery.
    pub new_item: Option<NewItem>,
    pub quantity: i32,
    /// What you paid each, before tax and shipping.
    pub unit_cost_cents: i64,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ReceiveReq {
    pub lines: Vec<ReceiveLine>,
    pub vendor: Option<String>,
    /// `YYYY-MM-DD`, default today.
    pub incurred_on: Option<String>,
    /// The receipt total, when it includes tax and shipping; the difference
    /// from the line subtotal is spread across the lines.
    pub total_cents: Option<i64>,
    pub note: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ReceiveResp {
    pub expense_id: Uuid,
    pub items: Vec<InventoryItemDto>,
    pub total_cents: i64,
}

/// `POST /inventory/receive` — a delivery arrives.
#[rocket_okapi::openapi(tag = "Stock")]
#[post("/inventory/receive", data = "<body>")]
pub async fn receive(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<ReceiveReq>,
) -> ApiResult<Json<ReceiveResp>> {
    user.require(Permission::MaintenanceManage)?;
    let b = body.into_inner();
    if b.lines.is_empty() {
        return Err(ApiError::BadRequest("add at least one line".into()));
    }
    let clean = |s: Option<String>| s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
    let now = Utc::now();
    let day = match clean(b.incurred_on.clone()) {
        Some(d) => crate::workforce::parse_date(&d, "incurred_on")?,
        None => crate::workforce::Rules::load(&db, scope.tenant_id)
            .await
            .local_date(now.into()),
    };
    // Resolve (or create) every item first, locking existing rows.
    let mut items: Vec<entity::inventory_item::Model> = Vec::new();
    for l in &b.lines {
        if l.quantity <= 0 || l.unit_cost_cents < 0 {
            return Err(ApiError::BadRequest(
                "each line needs a quantity and a cost".into(),
            ));
        }
        let item = match (l.inventory_item_id, &l.new_item) {
            (Some(id), _) => InventoryItem::find_by_id(id)
                .filter(entity::inventory_item::Column::TenantId.eq(scope.tenant_id))
                .lock_exclusive()
                .one(&db)
                .await?
                .ok_or_else(|| ApiError::NotFound("stock item not found".into()))?,
            (None, Some(n)) => {
                let name = n.name.trim().to_string();
                if name.is_empty() {
                    return Err(ApiError::BadRequest("the new item needs a name".into()));
                }
                let barcode = clean(n.barcode.clone());
                if let Some(code) = &barcode {
                    if InventoryItem::find()
                        .filter(entity::inventory_item::Column::TenantId.eq(scope.tenant_id))
                        .filter(entity::inventory_item::Column::Barcode.eq(code.clone()))
                        .one(&db)
                        .await?
                        .is_some()
                    {
                        return Err(ApiError::Conflict(format!(
                            "another item already has the barcode {code}"
                        )));
                    }
                }
                entity::inventory_item::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    tenant_id: Set(scope.tenant_id),
                    property_id: Set(None),
                    name: Set(name),
                    sku: Set(clean(n.sku.clone())),
                    barcode: Set(barcode),
                    unit: Set(clean(n.unit.clone()).unwrap_or_else(|| "ea".into())),
                    vendor: Set(clean(b.vendor.clone())),
                    category: Set(clean(n.category.clone()).unwrap_or_else(|| "part".into())),
                    quantity: Set(0),
                    unit_cost_cents: Set(None),
                    reorder_level: Set(n.reorder_level.unwrap_or(0).max(0)),
                    storage_location: Set(clean(n.storage_location.clone())),
                    serial_numbers: Set(serde_json::json!([])),
                    notes: Set(None),
                    low_stock_alerted_at: Set(None),
                    status: Set("active".into()),
                    created_by: Set(Some(user.user_id)),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(&db)
                .await?
            }
            (None, None) => {
                return Err(ApiError::BadRequest(
                    "each line needs an item or a new item".into(),
                ))
            }
        };
        items.push(item);
    }
    let subtotal: i64 = b
        .lines
        .iter()
        .map(|l| l.quantity as i64 * l.unit_cost_cents)
        .sum();
    let total = b.total_cents.filter(|t| *t > 0).unwrap_or(subtotal);
    let landed = landed_costs(
        &b.lines
            .iter()
            .map(|l| (l.quantity, l.unit_cost_cents))
            .collect::<Vec<_>>(),
        (total - subtotal).max(0),
    );
    // One expense for the delivery.
    let vendor = clean(b.vendor.clone());
    let expense = entity::expense::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        incurred_on: Set(day.to_string()),
        category: Set("materials".into()),
        vendor: Set(vendor.clone()),
        description: Set(format!(
            "Stock: {}",
            items.iter().map(|i| i.name.clone()).collect::<Vec<_>>().join(", ")
        )),
        amount_cents: Set(total),
        miles_hundredths: Set(None),
        mileage_rate_mills: Set(None),
        tax_deductible: Set(true),
        vehicle: Set("none".into()),
        reimbursable: Set(false),
        reimbursed_at: Set(None),
        billable_to_owner: Set(false),
        billed_bill_id: Set(None),
        user_id: Set(None),
        maintenance_ticket_id: Set(None),
        rehab_project_id: Set(None),
        property_id: Set(None),
        asset_id: Set(None),
        details: Set(serde_json::json!({ "stock": b.lines.iter().zip(&items).map(|(l, i)| serde_json::json!({ "item_id": i.id, "quantity": l.quantity, "unit_cost_cents": l.unit_cost_cents })).collect::<Vec<_>>(), "note": clean(b.note.clone()) })),
        recorded_by: Set(Some(user.user_id)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;
    let mut out = Vec::new();
    for ((l, item), landed_each) in b.lines.iter().zip(items).zip(landed) {
        let avg = weighted_average(
            item.quantity,
            item.unit_cost_cents.unwrap_or(landed_each),
            l.quantity,
            landed_each,
        );
        record_movement(
            &db,
            scope.tenant_id,
            item.id,
            "receive",
            l.quantity,
            landed_each,
            None,
            Some(expense.id),
            vendor.as_deref(),
            Some(user.user_id),
        )
        .await?;
        let mut am: entity::inventory_item::ActiveModel = item.clone().into();
        am.quantity = Set(item.quantity.max(0) + l.quantity);
        am.unit_cost_cents = Set(Some(avg));
        if item.vendor.is_none() {
            am.vendor = Set(vendor.clone());
        }
        // Back above the reorder level: the next dip alerts again.
        if item.quantity.max(0) + l.quantity > item.reorder_level {
            am.low_stock_alerted_at = Set(None);
        }
        am.updated_at = Set(now.into());
        out.push(InventoryItemDto::from(am.update(&db).await?));
    }
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::STOCK_RECEIVE,
        Some("expense"),
        Some(expense.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "lines": b.lines.len(), "total_cents": total })),
    )
    .await;
    Ok(Json(ReceiveResp {
        expense_id: expense.id,
        items: out,
        total_cents: total,
    }))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CountReq {
    pub counted: i32,
    pub note: Option<String>,
}

/// `POST /inventory/<id>/count` — a physical count; the difference is recorded.
#[rocket_okapi::openapi(tag = "Stock")]
#[post("/inventory/<id>/count", data = "<body>")]
pub async fn count(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<CountReq>,
) -> ApiResult<Json<InventoryItemDto>> {
    user.require(Permission::MaintenanceManage)?;
    if body.counted < 0 {
        return Err(ApiError::BadRequest("a count can't be negative".into()));
    }
    let item = InventoryItem::find_by_id(parse_id(id, "stock item")?)
        .filter(entity::inventory_item::Column::TenantId.eq(scope.tenant_id))
        .lock_exclusive()
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("stock item not found".into()))?;
    let diff = body.counted - item.quantity;
    record_movement(
        &db,
        scope.tenant_id,
        item.id,
        "count",
        diff,
        item.unit_cost_cents.unwrap_or(0),
        None,
        None,
        body.note.as_deref(),
        Some(user.user_id),
    )
    .await?;
    let mut am: entity::inventory_item::ActiveModel = item.into();
    am.quantity = Set(body.counted);
    am.updated_at = Set(Utc::now().into());
    Ok(Json(InventoryItemDto::from(am.update(&db).await?)))
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct MovementDto {
    pub id: Uuid,
    pub inventory_item_id: Uuid,
    pub item_name: String,
    pub kind: String,
    pub quantity: i32,
    pub unit_cost_cents: i64,
    pub value_cents: i64,
    pub ticket_id: Option<Uuid>,
    pub ticket_title: Option<String>,
    pub expense_id: Option<Uuid>,
    pub note: Option<String>,
    pub recorded_by: Option<String>,
    pub created_at: String,
}

/// `GET /inventory/movements?item_id&ticket_id&kind` — the stock ledger.
#[rocket_okapi::openapi(tag = "Stock")]
#[get("/inventory/movements?<item_id>&<ticket_id>&<kind>")]
pub async fn movements(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    item_id: Option<String>,
    ticket_id: Option<String>,
    kind: Option<String>,
) -> ApiResult<Json<Vec<MovementDto>>> {
    user.require(Permission::MaintenanceRead)?;
    let mut q = InventoryMovement::find()
        .filter(entity::inventory_movement::Column::TenantId.eq(scope.tenant_id));
    if let Some(i) = item_id.filter(|s| !s.is_empty()) {
        q = q.filter(entity::inventory_movement::Column::InventoryItemId.eq(parse_id(&i, "item")?));
    }
    if let Some(t) = ticket_id.filter(|s| !s.is_empty()) {
        q = q.filter(entity::inventory_movement::Column::TicketId.eq(parse_id(&t, "work order")?));
    }
    if let Some(k) = kind.filter(|s| !s.is_empty()) {
        q = q.filter(entity::inventory_movement::Column::Kind.eq(k));
    }
    let rows = q
        .order_by_desc(entity::inventory_movement::Column::CreatedAt)
        .limit(500)
        .all(&db)
        .await?;
    let names: HashMap<Uuid, String> = InventoryItem::find()
        .filter(
            entity::inventory_item::Column::Id
                .is_in(rows.iter().map(|r| r.inventory_item_id).collect::<Vec<_>>()),
        )
        .all(&db)
        .await?
        .into_iter()
        .map(|i| (i.id, i.name))
        .collect();
    let tickets: HashMap<Uuid, String> = MaintenanceTicket::find()
        .filter(
            entity::maintenance_ticket::Column::Id
                .is_in(rows.iter().filter_map(|r| r.ticket_id).collect::<Vec<_>>()),
        )
        .all(&db)
        .await?
        .into_iter()
        .map(|t| (t.id, t.title))
        .collect();
    let users: HashMap<Uuid, String> = User::find()
        .filter(
            entity::user::Column::Id.is_in(
                rows.iter()
                    .filter_map(|r| r.recorded_by)
                    .collect::<Vec<_>>(),
            ),
        )
        .all(&db)
        .await?
        .into_iter()
        .map(|u| (u.id, u.name))
        .collect();
    Ok(Json(
        rows.into_iter()
            .map(|m| MovementDto {
                item_name: names.get(&m.inventory_item_id).cloned().unwrap_or_default(),
                ticket_title: m.ticket_id.and_then(|t| tickets.get(&t).cloned()),
                recorded_by: m.recorded_by.and_then(|u| users.get(&u).cloned()),
                value_cents: (m.quantity.abs() as i64) * m.unit_cost_cents,
                id: m.id,
                inventory_item_id: m.inventory_item_id,
                kind: m.kind,
                quantity: m.quantity,
                unit_cost_cents: m.unit_cost_cents,
                ticket_id: m.ticket_id,
                expense_id: m.expense_id,
                note: m.note,
                created_at: m.created_at.to_rfc3339(),
            })
            .collect(),
    ))
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ReorderGroup {
    pub vendor: String,
    pub items: Vec<InventoryItemDto>,
}

/// `GET /inventory/reorder` — what to order, grouped by vendor.
#[rocket_okapi::openapi(tag = "Stock")]
#[get("/inventory/reorder")]
pub async fn reorder(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<ReorderGroup>>> {
    user.require(Permission::MaintenanceRead)?;
    let items = InventoryItem::find()
        .filter(entity::inventory_item::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::inventory_item::Column::Status.eq("active"))
        .filter(entity::inventory_item::Column::ReorderLevel.gt(0))
        .order_by_asc(entity::inventory_item::Column::Name)
        .all(&db)
        .await?;
    let mut groups: BTreeMap<String, Vec<InventoryItemDto>> = BTreeMap::new();
    for i in items.into_iter().filter(|i| i.quantity <= i.reorder_level) {
        groups
            .entry(i.vendor.clone().unwrap_or_else(|| "No vendor".into()))
            .or_default()
            .push(InventoryItemDto::from(i));
    }
    Ok(Json(
        groups
            .into_iter()
            .map(|(vendor, items)| ReorderGroup { vendor, items })
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weighted_average_cost() {
        // 10 on hand at $4, 10 arrive at $6 → $5.
        assert_eq!(weighted_average(10, 400, 10, 600), 500);
        // Nothing on hand: the landed cost.
        assert_eq!(weighted_average(0, 900, 5, 300), 300);
        // A negative shelf counts as empty.
        assert_eq!(weighted_average(-3, 900, 5, 300), 300);
    }

    #[test]
    fn tax_and_shipping_spread_by_value() {
        // $100 of filters (10 × $10) and $50 of belts (5 × $10); $15 extra.
        let landed = landed_costs(&[(10, 1000), (5, 1000)], 1500);
        assert_eq!(landed, vec![1100, 1100]);
        // Uneven: the last line absorbs the rounding.
        let landed = landed_costs(&[(3, 333), (1, 100)], 100);
        let total: i64 = 3 * (landed[0] - 333) + (landed[1] - 100);
        assert!((99..=101).contains(&total), "{landed:?}");
        assert_eq!(landed_costs(&[(2, 500)], 0), vec![500]);
    }
}
