//! The **parts/supplies stockroom** — inventory the maintenance team draws
//! from when working tickets: SKU, quantity on hand, unit cost, reorder
//! level, storage location, and a serial-number pool for serialized stock.
//! Ticket lines consume it (and restock on removal); the helpdesk scan
//! raises a low-stock alert when quantity falls to the reorder level.

use super::dto::{
    clean, serials_from_json, CreateInventoryReq, InventoryItemDto, UpdateInventoryReq,
};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{InventoryItem, Property};
use rocket::serde::json::Json;
use rocket::{get, patch, post, State};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use uuid::Uuid;

/// Inventory categories the API accepts.
pub const CATEGORIES: &[&str] = &["part", "material", "tool", "supply", "other"];

fn normalize_category(raw: Option<String>) -> Result<String, ApiError> {
    match raw.as_deref().map(str::trim) {
        None | Some("") => Ok("part".into()),
        Some(c) if CATEGORIES.contains(&c) => Ok(c.to_string()),
        Some(c) => Err(ApiError::BadRequest(format!(
            "invalid category: {c} (expected one of {})",
            CATEGORIES.join(", ")
        ))),
    }
}

/// Trim a serial pool and reject duplicates outright — a duplicate serial
/// would later be stripped in one `retain` while decrementing quantity by
/// one, leaving the pool and the count permanently out of step.
fn clean_serials(serials: Vec<String>) -> Result<Vec<String>, ApiError> {
    let out: Vec<String> = serials
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let mut seen = std::collections::HashSet::new();
    if let Some(dup) = out.iter().find(|s| !seen.insert(s.as_str())) {
        return Err(ApiError::BadRequest(format!(
            "duplicate serial number: {dup}"
        )));
    }
    Ok(out)
}

/// `GET /inventory?property_id&status&low_stock` — the stockroom, name order.
#[rocket_okapi::openapi(tag = "Maintenance")]
#[get("/inventory?<property_id>&<status>&<low_stock>")]
pub async fn list_inventory(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    property_id: Option<String>,
    status: Option<String>,
    low_stock: Option<bool>,
) -> ApiResult<Json<Vec<InventoryItemDto>>> {
    user.require(Permission::MaintenanceRead)?;
    let mut q =
        InventoryItem::find().filter(entity::inventory_item::Column::TenantId.eq(scope.tenant_id));
    if let Some(pid) = property_id.filter(|s| !s.trim().is_empty()) {
        let pid = Uuid::parse_str(&pid)
            .map_err(|_| ApiError::BadRequest("invalid property_id".into()))?;
        q = q.filter(entity::inventory_item::Column::PropertyId.eq(pid));
    }
    if let Some(s) = status.filter(|s| !s.trim().is_empty()) {
        q = q.filter(entity::inventory_item::Column::Status.eq(s.trim().to_lowercase()));
    }
    if low_stock.unwrap_or(false) {
        q = q
            .filter(entity::inventory_item::Column::ReorderLevel.gt(0))
            .filter(
                Expr::col(entity::inventory_item::Column::Quantity)
                    .lte(Expr::col(entity::inventory_item::Column::ReorderLevel)),
            );
    }
    let rows = q
        .order_by_asc(entity::inventory_item::Column::Name)
        .all(&db)
        .await?;
    Ok(Json(rows.into_iter().map(InventoryItemDto::from).collect()))
}

/// `POST /inventory` — stock a new item.
#[rocket_okapi::openapi(tag = "Maintenance")]
#[post("/inventory", data = "<body>")]
pub async fn create_inventory(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<CreateInventoryReq>,
) -> ApiResult<Json<InventoryItemDto>> {
    user.require(Permission::MaintenanceManage)?;
    let b = body.into_inner();
    let name = b.name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::BadRequest("name is required".into()));
    }
    let category = normalize_category(b.category)?;
    let quantity = b.quantity.unwrap_or(0);
    if quantity < 0 {
        return Err(ApiError::BadRequest("quantity cannot be negative".into()));
    }
    let reorder_level = b.reorder_level.unwrap_or(0);
    if reorder_level < 0 {
        return Err(ApiError::BadRequest(
            "reorder_level cannot be negative".into(),
        ));
    }
    if let Some(pid) = b.property_id {
        Property::find_by_id(pid)
            .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
            .one(&db)
            .await?
            .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    }
    let serials = clean_serials(b.serial_numbers.unwrap_or_default())?;
    if !serials.is_empty() && serials.len() as i32 != quantity {
        return Err(ApiError::BadRequest(
            "serialized stock needs one serial per unit (serials must match quantity)".into(),
        ));
    }

    let now = Utc::now();
    let saved = entity::inventory_item::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        property_id: Set(b.property_id),
        name: Set(name),
        sku: Set(clean(b.sku)),
        category: Set(category),
        quantity: Set(quantity),
        unit_cost_cents: Set(b.unit_cost_cents.filter(|c| *c >= 0)),
        reorder_level: Set(reorder_level),
        storage_location: Set(clean(b.storage_location)),
        serial_numbers: Set(serde_json::json!(serials)),
        notes: Set(clean(b.notes)),
        low_stock_alerted_at: Set(None),
        status: Set("active".into()),
        created_by: Set(Some(user.user_id)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::INVENTORY_CREATE,
        Some("inventory_item"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "name": saved.name, "quantity": saved.quantity })),
    )
    .await;

    Ok(Json(InventoryItemDto::from(saved)))
}

/// `PATCH /inventory/<id>` — restock, correct, edit, or archive an item.
#[rocket_okapi::openapi(tag = "Maintenance")]
#[patch("/inventory/<id>", data = "<body>")]
pub async fn update_inventory(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateInventoryReq>,
) -> ApiResult<Json<InventoryItemDto>> {
    user.require(Permission::MaintenanceManage)?;
    let iid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    // FOR UPDATE: quantity/serial edits race with ticket lines consuming
    // stock inside their own request transactions.
    let item = InventoryItem::find_by_id(iid)
        .filter(entity::inventory_item::Column::TenantId.eq(scope.tenant_id))
        .lock_exclusive()
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("inventory item not found".into()))?;
    let b = body.into_inner();

    // The final quantity/serials must stay consistent when either changes.
    let next_quantity = b.quantity.unwrap_or(item.quantity);
    if next_quantity < 0 {
        return Err(ApiError::BadRequest("quantity cannot be negative".into()));
    }
    let next_serials = match b.serial_numbers {
        Some(s) => clean_serials(s)?,
        None => serials_from_json(&item.serial_numbers),
    };
    if !next_serials.is_empty() && next_serials.len() as i32 != next_quantity {
        return Err(ApiError::BadRequest(
            "serialized stock needs one serial per unit (serials must match quantity)".into(),
        ));
    }

    let mut am: entity::inventory_item::ActiveModel = item.into();
    if let Some(v) = b.name.map(|s| s.trim().to_string()) {
        if v.is_empty() {
            return Err(ApiError::BadRequest("name cannot be empty".into()));
        }
        am.name = Set(v);
    }
    if b.category.is_some() {
        am.category = Set(normalize_category(b.category)?);
    }
    if let Some(v) = b.sku {
        am.sku = Set(clean(Some(v)));
    }
    am.quantity = Set(next_quantity);
    am.serial_numbers = Set(serde_json::json!(next_serials));
    if let Some(v) = b.unit_cost_cents {
        am.unit_cost_cents = Set(Some(v).filter(|c| *c >= 0));
    }
    if let Some(v) = b.reorder_level {
        if v < 0 {
            return Err(ApiError::BadRequest(
                "reorder_level cannot be negative".into(),
            ));
        }
        am.reorder_level = Set(v);
    }
    if let Some(v) = b.storage_location {
        am.storage_location = Set(clean(Some(v)));
    }
    if let Some(v) = b.notes {
        am.notes = Set(clean(Some(v)));
    }
    if let Some(v) = b.status.map(|s| s.trim().to_lowercase()) {
        if !matches!(v.as_str(), "active" | "archived") {
            return Err(ApiError::BadRequest(
                "status must be active|archived".into(),
            ));
        }
        am.status = Set(v);
    }
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::INVENTORY_UPDATE,
        Some("inventory_item"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "quantity": saved.quantity, "status": saved.status })),
    )
    .await;

    Ok(Json(InventoryItemDto::from(saved)))
}
