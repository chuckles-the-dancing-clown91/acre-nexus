//! **The parts loop** (Vantedge phase 2C): what a work order needs, from
//! "might need" to "on the truck" to "in the unit".
//!
//! * **Findings** — what the technician found (a `finding` comment with photos
//!   through the document service) and the parts it needs.
//! * **Potential parts** — pre-listed from the appliance's parts catalog when a
//!   ticket is opened for it (replace / repair, or a maintenance plan).
//! * **Generate a parts list** — merges the ticket's potential parts and every
//!   finding's parts, marks what's *in stock* and what's *to order*, and hands
//!   back a pick list (printable as a PDF for the truck or the store run).
//! * **Close-out** — the night before: every work order due tomorrow with its
//!   shopping list; the office decides each item: *order* (vendor, tracking,
//!   ship to the property or the office — an expense billable to the owner),
//!   *pick up*, *from stock* (consumes inventory), or *skip*. Ordered parts
//!   arrive as *received*; *used* puts them on the ticket as a part line, so
//!   costing and the owner bill see them.

use super::dto::{TicketCommentDto, TicketLineDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::pdfdoc::{Block, Column, Document, Table};
use crate::rbac::Permission;
use crate::routes::reports::ReportFile;
use crate::routes::team::parse_id;
use crate::tenancy::TenantScope;
use chrono::{Duration, NaiveDate, Utc};
use entity::prelude::{
    Asset, AssetPart, InventoryItem, MaintenanceTicket, Property, TicketComment, TicketPart, User,
};
use rocket::serde::json::Json;
use rocket::{delete, get, patch, post, put};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Statuses that still want a decision at close-out.
pub const OPEN_STATUSES: &[&str] = &["needed", "to_order"];
const SHIP_TO: &[&str] = &["property", "office", "other"];

#[derive(Serialize, schemars::JsonSchema, Clone)]
pub struct TicketPartDto {
    pub id: Uuid,
    pub ticket_id: Uuid,
    pub inventory_item_id: Option<Uuid>,
    pub name: String,
    pub quantity: i32,
    pub status: String,
    pub source: String,
    pub finding_comment_id: Option<Uuid>,
    pub need_by: Option<String>,
    pub ship_to: Option<String>,
    pub ship_to_note: Option<String>,
    pub vendor: Option<String>,
    pub tracking: Option<String>,
    pub unit_cost_cents: Option<i64>,
    pub note: Option<String>,
    /// Where to buy it, and the store that is.
    pub url: Option<String>,
    pub store: Option<String>,
    /// How many are on the shelf right now (when it's a stock item).
    pub in_stock: Option<i32>,
    pub ordered_at: Option<String>,
    pub received_at: Option<String>,
    pub created_at: String,
}

pub(super) async fn part_dtos(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    parts: Vec<entity::ticket_part::Model>,
) -> ApiResult<Vec<TicketPartDto>> {
    let ids: Vec<Uuid> = parts.iter().filter_map(|p| p.inventory_item_id).collect();
    let stock: HashMap<Uuid, i32> = if ids.is_empty() {
        HashMap::new()
    } else {
        InventoryItem::find()
            .filter(entity::inventory_item::Column::TenantId.eq(tenant_id))
            .filter(entity::inventory_item::Column::Id.is_in(ids))
            .all(db)
            .await?
            .into_iter()
            .map(|i| (i.id, i.quantity))
            .collect()
    };
    Ok(parts
        .into_iter()
        .map(|p| TicketPartDto {
            in_stock: p.inventory_item_id.and_then(|i| stock.get(&i).copied()),
            id: p.id,
            ticket_id: p.ticket_id,
            inventory_item_id: p.inventory_item_id,
            name: p.name,
            quantity: p.quantity,
            status: p.status,
            source: p.source,
            finding_comment_id: p.finding_comment_id,
            need_by: p.need_by,
            ship_to: p.ship_to,
            ship_to_note: p.ship_to_note,
            store: p.vendor.clone().or_else(|| {
                p.url
                    .as_deref()
                    .and_then(crate::servicedesk::store_of)
                    .map(str::to_string)
            }),
            url: p.url,
            vendor: p.vendor,
            tracking: p.tracking,
            unit_cost_cents: p.unit_cost_cents,
            note: p.note,
            ordered_at: p.ordered_at.map(|d| d.to_rfc3339()),
            received_at: p.received_at.map(|d| d.to_rfc3339()),
            created_at: p.created_at.to_rfc3339(),
        })
        .collect())
}

pub(super) async fn parts_for_ticket(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    ticket_id: Uuid,
) -> ApiResult<Vec<TicketPartDto>> {
    let parts = TicketPart::find()
        .filter(entity::ticket_part::Column::TenantId.eq(tenant_id))
        .filter(entity::ticket_part::Column::TicketId.eq(ticket_id))
        .order_by_asc(entity::ticket_part::Column::CreatedAt)
        .all(db)
        .await?;
    part_dtos(db, tenant_id, parts).await
}

/// Insert one part row.
#[allow(clippy::too_many_arguments)]
pub async fn add_part(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    ticket_id: Uuid,
    inventory_item_id: Option<Uuid>,
    name: &str,
    quantity: i32,
    status: &str,
    source: &str,
    finding_comment_id: Option<Uuid>,
    created_by: Option<Uuid>,
) -> ApiResult<entity::ticket_part::Model> {
    let now = Utc::now();
    Ok(entity::ticket_part::ActiveModel {
        url: Set(None),
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        ticket_id: Set(ticket_id),
        inventory_item_id: Set(inventory_item_id),
        name: Set(name.trim().to_string()),
        quantity: Set(quantity.max(1)),
        status: Set(status.into()),
        source: Set(source.into()),
        finding_comment_id: Set(finding_comment_id),
        need_by: Set(None),
        ship_to: Set(None),
        ship_to_note: Set(None),
        vendor: Set(None),
        tracking: Set(None),
        unit_cost_cents: Set(None),
        expense_id: Set(None),
        ticket_line_id: Set(None),
        note: Set(None),
        ordered_at: Set(None),
        ordered_by: Set(None),
        received_at: Set(None),
        created_by: Set(created_by),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?)
}

/// Pre-list an appliance's catalog on a ticket as *potential* parts (skipping
/// items already on the list).
pub async fn add_potential_from_asset(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    ticket_id: Uuid,
    asset_id: Uuid,
    created_by: Option<Uuid>,
) -> ApiResult<usize> {
    let catalog = AssetPart::find()
        .filter(entity::asset_part::Column::TenantId.eq(tenant_id))
        .filter(entity::asset_part::Column::AssetId.eq(asset_id))
        .all(db)
        .await?;
    if catalog.is_empty() {
        return Ok(0);
    }
    let existing: Vec<Option<Uuid>> = TicketPart::find()
        .filter(entity::ticket_part::Column::TenantId.eq(tenant_id))
        .filter(entity::ticket_part::Column::TicketId.eq(ticket_id))
        .all(db)
        .await?
        .into_iter()
        .map(|p| p.inventory_item_id)
        .collect();
    let items: HashMap<Uuid, entity::inventory_item::Model> = InventoryItem::find()
        .filter(entity::inventory_item::Column::TenantId.eq(tenant_id))
        .filter(
            entity::inventory_item::Column::Id.is_in(
                catalog
                    .iter()
                    .map(|c| c.inventory_item_id)
                    .collect::<Vec<_>>(),
            ),
        )
        .all(db)
        .await?
        .into_iter()
        .map(|i| (i.id, i))
        .collect();
    let mut n = 0;
    for c in catalog {
        if existing.contains(&Some(c.inventory_item_id)) {
            continue;
        }
        let Some(item) = items.get(&c.inventory_item_id) else {
            continue;
        };
        let name = match &c.role {
            Some(r) => format!("{} ({r})", item.name),
            None => item.name.clone(),
        };
        add_part(
            db,
            tenant_id,
            ticket_id,
            Some(item.id),
            &name,
            c.quantity,
            "potential",
            "asset",
            None,
            created_by,
        )
        .await?;
        n += 1;
    }
    Ok(n)
}

// ---------------------------------------------------------------------------
// Findings
// ---------------------------------------------------------------------------

#[derive(Deserialize, schemars::JsonSchema)]
pub struct PartReq {
    pub inventory_item_id: Option<Uuid>,
    /// Required when it isn't a stock item.
    pub name: Option<String>,
    pub quantity: Option<i32>,
    pub note: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct FindingReq {
    pub body: String,
    /// `public` (the resident sees it) | `internal`
    pub visibility: Option<String>,
    #[serde(default)]
    pub parts: Vec<PartReq>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct FindingDto {
    #[serde(flatten)]
    pub comment: TicketCommentDto,
    pub parts: Vec<TicketPartDto>,
}

async fn resolve_part_name(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    r: &PartReq,
) -> ApiResult<(Option<Uuid>, String)> {
    if let Some(id) = r.inventory_item_id {
        let item = InventoryItem::find_by_id(id)
            .filter(entity::inventory_item::Column::TenantId.eq(tenant_id))
            .one(db)
            .await?
            .ok_or_else(|| ApiError::NotFound("stock item not found".into()))?;
        return Ok((
            Some(item.id),
            r.name
                .clone()
                .filter(|n| !n.trim().is_empty())
                .unwrap_or(item.name),
        ));
    }
    let name = r
        .name
        .clone()
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .ok_or_else(|| ApiError::BadRequest("say what the part is".into()))?;
    Ok((None, name))
}

/// `POST /tickets/<id>/findings` — "add finding": what you found, and the parts
/// it needs. Photos attach to the ticket through the document service.
#[rocket_okapi::openapi(tag = "Parts")]
#[post("/tickets/<id>/findings", data = "<body>")]
pub async fn add_finding(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<FindingReq>,
) -> ApiResult<Json<FindingDto>> {
    user.require(Permission::MaintenanceManage)?;
    let ticket = super::quotes::find_ticket(&db, scope.tenant_id, id).await?;
    let text = body.body.trim().to_string();
    if text.is_empty() {
        return Err(ApiError::BadRequest("describe what you found".into()));
    }
    let visibility = match body.visibility.as_deref() {
        None | Some("internal") => "internal",
        Some("public") => "public",
        Some(v) => return Err(ApiError::BadRequest(format!("invalid visibility: {v}"))),
    };
    let author = User::find_by_id(user.user_id)
        .one(&db)
        .await?
        .map(|u| u.name);
    let comment = entity::ticket_comment::ActiveModel {
        action: Set(None),
        document_ids: Set(serde_json::json!([])),
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        ticket_id: Set(ticket.id),
        task_id: Set(None),
        author_user_id: Set(Some(user.user_id)),
        kind: Set("finding".into()),
        visibility: Set(visibility.into()),
        author_name: Set(author),
        body: Set(text),
        created_at: Set(Utc::now().into()),
    }
    .insert(&db)
    .await?;
    let mut parts = Vec::new();
    for r in &body.parts {
        let (item_id, name) = resolve_part_name(&db, scope.tenant_id, r).await?;
        let p = add_part(
            &db,
            scope.tenant_id,
            ticket.id,
            item_id,
            &name,
            r.quantity.unwrap_or(1),
            "needed",
            "finding",
            Some(comment.id),
            Some(user.user_id),
        )
        .await?;
        if let Some(n) = r.note.clone().filter(|n| !n.trim().is_empty()) {
            let mut am: entity::ticket_part::ActiveModel = p.clone().into();
            am.note = Set(Some(n));
            parts.push(am.update(&db).await?);
        } else {
            parts.push(p);
        }
    }
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::TICKET_FINDING,
        Some("maintenance_ticket"),
        Some(ticket.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "parts": parts.len() })),
    )
    .await;
    Ok(Json(FindingDto {
        comment: TicketCommentDto::from(comment),
        parts: part_dtos(&db, scope.tenant_id, parts).await?,
    }))
}

/// `GET /tickets/<id>/findings` — findings with their parts.
#[rocket_okapi::openapi(tag = "Parts")]
#[get("/tickets/<id>/findings")]
pub async fn list_findings(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Vec<FindingDto>>> {
    user.require(Permission::MaintenanceRead)?;
    let ticket = super::quotes::find_ticket(&db, scope.tenant_id, id).await?;
    let comments = TicketComment::find()
        .filter(entity::ticket_comment::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::ticket_comment::Column::TicketId.eq(ticket.id))
        .filter(entity::ticket_comment::Column::Kind.eq("finding"))
        .order_by_desc(entity::ticket_comment::Column::CreatedAt)
        .all(&db)
        .await?;
    let parts = parts_for_ticket(&db, scope.tenant_id, ticket.id).await?;
    Ok(Json(
        comments
            .into_iter()
            .map(|c| FindingDto {
                parts: parts
                    .iter()
                    .filter(|p| p.finding_comment_id == Some(c.id))
                    .cloned()
                    .collect(),
                comment: TicketCommentDto::from(c),
            })
            .collect(),
    ))
}

// ---------------------------------------------------------------------------
// Parts on a ticket
// ---------------------------------------------------------------------------

/// `GET /tickets/<id>/parts`.
#[rocket_okapi::openapi(tag = "Parts")]
#[get("/tickets/<id>/parts")]
pub async fn list_parts(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Vec<TicketPartDto>>> {
    user.require(Permission::MaintenanceRead)?;
    let ticket = super::quotes::find_ticket(&db, scope.tenant_id, id).await?;
    Ok(Json(
        parts_for_ticket(&db, scope.tenant_id, ticket.id).await?,
    ))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct AddPartReq {
    #[serde(flatten)]
    pub part: PartReq,
    /// `potential` (might need) or `needed` (default).
    pub status: Option<String>,
}

/// `POST /tickets/<id>/parts` — add a part by hand.
#[rocket_okapi::openapi(tag = "Parts")]
#[post("/tickets/<id>/parts", data = "<body>")]
pub async fn create_part(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<AddPartReq>,
) -> ApiResult<Json<TicketPartDto>> {
    user.require(Permission::MaintenanceManage)?;
    let ticket = super::quotes::find_ticket(&db, scope.tenant_id, id).await?;
    let status = match body.status.as_deref() {
        None | Some("needed") => "needed",
        Some("potential") => "potential",
        Some(s) => {
            return Err(ApiError::BadRequest(format!(
                "status must be potential or needed, not {s}"
            )))
        }
    };
    let (item_id, name) = resolve_part_name(&db, scope.tenant_id, &body.part).await?;
    let p = add_part(
        &db,
        scope.tenant_id,
        ticket.id,
        item_id,
        &name,
        body.part.quantity.unwrap_or(1),
        status,
        "typed",
        None,
        Some(user.user_id),
    )
    .await?;
    Ok(Json(
        part_dtos(&db, scope.tenant_id, vec![p]).await?.remove(0),
    ))
}

pub(super) async fn find_part(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    id: &str,
) -> ApiResult<entity::ticket_part::Model> {
    TicketPart::find_by_id(parse_id(id, "part")?)
        .filter(entity::ticket_part::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("part not found".into()))
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct UpdatePartReq {
    pub name: Option<String>,
    pub quantity: Option<i32>,
    /// `potential` | `needed` | `skipped` (the rest move through close-out).
    pub status: Option<String>,
    pub need_by: Option<String>,
    pub note: Option<String>,
    /// Where to buy it ("" clears it); the store comes from the link.
    pub url: Option<String>,
}

/// `PATCH /parts/<id>`.
#[rocket_okapi::openapi(tag = "Parts")]
#[patch("/parts/<id>", data = "<body>")]
pub async fn update_part(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdatePartReq>,
) -> ApiResult<Json<TicketPartDto>> {
    user.require(Permission::MaintenanceManage)?;
    let p = find_part(&db, scope.tenant_id, id).await?;
    if p.status == "used" {
        return Err(ApiError::Conflict(
            "this part was used on the work order".into(),
        ));
    }
    let mut am: entity::ticket_part::ActiveModel = p.into();
    if let Some(n) = body
        .name
        .clone()
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
    {
        am.name = Set(n);
    }
    if let Some(q) = body.quantity {
        if q <= 0 {
            return Err(ApiError::BadRequest("quantity must be at least 1".into()));
        }
        am.quantity = Set(q);
    }
    if let Some(s) = body.status.as_deref() {
        if !["potential", "needed", "skipped"].contains(&s) {
            return Err(ApiError::BadRequest(
                "status must be potential, needed or skipped here".into(),
            ));
        }
        am.status = Set(s.into());
    }
    if let Some(d) = body.need_by.clone() {
        let d = d.trim().to_string();
        if !d.is_empty() {
            d.parse::<NaiveDate>()
                .map_err(|_| ApiError::BadRequest("need_by must be a date".into()))?;
        }
        am.need_by = Set(Some(d).filter(|d| !d.is_empty()));
    }
    if body.note.is_some() {
        am.note = Set(body
            .note
            .clone()
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty()));
    }
    if let Some(u) = body.url.as_deref() {
        let u = crate::servicedesk::clean_url(u).map_err(ApiError::BadRequest)?;
        am.vendor = Set(u
            .as_deref()
            .and_then(crate::servicedesk::store_of)
            .map(str::to_string));
        am.url = Set(u);
    }
    am.updated_at = Set(Utc::now().into());
    let p = am.update(&db).await?;
    Ok(Json(
        part_dtos(&db, scope.tenant_id, vec![p]).await?.remove(0),
    ))
}

/// `DELETE /parts/<id>`.
#[rocket_okapi::openapi(tag = "Parts")]
#[delete("/parts/<id>")]
pub async fn delete_part(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::MaintenanceManage)?;
    let p = find_part(&db, scope.tenant_id, id).await?;
    if ["ordered", "received", "used", "from_stock"].contains(&p.status.as_str()) {
        return Err(ApiError::Conflict(
            "this part is already on its way — skip it instead".into(),
        ));
    }
    TicketPart::delete_by_id(p.id).exec(&db).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct PartsList {
    pub ticket_id: Uuid,
    pub title: String,
    pub property: String,
    /// On the shelf: grab these from stock.
    pub from_stock: Vec<TicketPartDto>,
    /// Not in stock: the shopping list.
    pub to_buy: Vec<TicketPartDto>,
    /// Might be needed — decide on site.
    pub maybe: Vec<TicketPartDto>,
    /// Already on order / picked up / received.
    pub coming: Vec<TicketPartDto>,
}

/// Merge the ticket's parts into a pick list: every *needed* part becomes
/// *from_stock* when the shelf has enough, else *to_order*; *potential* stays
/// as "maybe". Idempotent — run it again after a finding.
pub async fn generate(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    ticket: &entity::maintenance_ticket::Model,
) -> ApiResult<PartsList> {
    // Pre-list the appliance's catalog if nothing has been listed yet.
    if let Some(aid) = ticket.asset_id {
        add_potential_from_asset(db, tenant_id, ticket.id, aid, None).await?;
    }
    let parts = TicketPart::find()
        .filter(entity::ticket_part::Column::TenantId.eq(tenant_id))
        .filter(entity::ticket_part::Column::TicketId.eq(ticket.id))
        .all(db)
        .await?;
    // Stock claimed by other open tickets counts as spoken for.
    let ids: Vec<Uuid> = parts.iter().filter_map(|p| p.inventory_item_id).collect();
    let mut stock: HashMap<Uuid, i32> = InventoryItem::find()
        .filter(entity::inventory_item::Column::TenantId.eq(tenant_id))
        .filter(entity::inventory_item::Column::Id.is_in(ids.clone()))
        .all(db)
        .await?
        .into_iter()
        .map(|i| (i.id, i.quantity))
        .collect();
    for other in TicketPart::find()
        .filter(entity::ticket_part::Column::TenantId.eq(tenant_id))
        .filter(entity::ticket_part::Column::TicketId.ne(ticket.id))
        .filter(entity::ticket_part::Column::Status.eq("from_stock"))
        .filter(entity::ticket_part::Column::InventoryItemId.is_in(ids))
        .all(db)
        .await?
    {
        if let Some(q) = other.inventory_item_id.and_then(|i| stock.get_mut(&i)) {
            *q -= other.quantity;
        }
    }
    for p in parts
        .iter()
        .filter(|p| p.status == "needed" || p.status == "to_order")
    {
        let have = p.inventory_item_id.and_then(|i| stock.get_mut(&i));
        let next = match have {
            Some(q) if *q >= p.quantity => {
                *q -= p.quantity;
                "from_stock"
            }
            _ => "to_order",
        };
        if next != p.status {
            let mut am: entity::ticket_part::ActiveModel = p.clone().into();
            am.status = Set(next.into());
            am.updated_at = Set(Utc::now().into());
            am.update(db).await?;
        }
    }
    let all = parts_for_ticket(db, tenant_id, ticket.id).await?;
    let property = Property::find_by_id(ticket.property_id)
        .one(db)
        .await?
        .map(|p| p.name)
        .unwrap_or_default();
    let pick = |s: &[&str]| {
        all.iter()
            .filter(|p| s.contains(&p.status.as_str()))
            .cloned()
            .collect::<Vec<_>>()
    };
    Ok(PartsList {
        ticket_id: ticket.id,
        title: ticket.title.clone(),
        property,
        from_stock: pick(&["from_stock"]),
        to_buy: pick(&["to_order"]),
        maybe: pick(&["potential"]),
        coming: pick(&["ordered", "pick_up", "received"]),
    })
}

/// `POST /tickets/<id>/parts/generate` — generate the parts list.
#[rocket_okapi::openapi(tag = "Parts")]
#[post("/tickets/<id>/parts/generate")]
pub async fn generate_list(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<PartsList>> {
    user.require(Permission::MaintenanceManage)?;
    let ticket = super::quotes::find_ticket(&db, scope.tenant_id, id).await?;
    let list = generate(&db, scope.tenant_id, &ticket).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::TICKET_PARTS_LIST,
        Some("maintenance_ticket"),
        Some(ticket.id.to_string()),
        Some(scope.tenant_id),
        Some(
            serde_json::json!({ "from_stock": list.from_stock.len(), "to_buy": list.to_buy.len() }),
        ),
    )
    .await;
    Ok(Json(list))
}

/// `GET /tickets/<id>/parts/list.pdf` — the pick list for the truck.
#[rocket_okapi::openapi(skip)]
#[get("/tickets/<id>/parts/list.pdf")]
pub async fn parts_list_pdf(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<ReportFile> {
    user.require(Permission::MaintenanceRead)?;
    let ticket = super::quotes::find_ticket(&db, scope.tenant_id, id).await?;
    let list = generate(&db, scope.tenant_id, &ticket).await?;
    let org = entity::prelude::Theme::find()
        .filter(entity::theme::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .map(|t| t.company_name)
        .unwrap_or_default();
    let table = |rows: &[TicketPartDto]| {
        Block::Table(Table {
            columns: vec![
                Column::right("Qty", 1.0),
                Column::left("Part", 6.0),
                Column::left("Note", 4.0),
                Column::left("☐", 1.0),
            ],
            rows: rows
                .iter()
                .map(|p| {
                    vec![
                        p.quantity.to_string(),
                        p.name.clone(),
                        p.note.clone().unwrap_or_default(),
                        String::new(),
                    ]
                })
                .collect(),
            totals: None,
        })
    };
    let mut blocks = vec![Block::KeyValues(vec![(
        "Property".into(),
        list.property.clone(),
    )])];
    for (h, rows) in [
        ("From stock", &list.from_stock),
        ("To buy", &list.to_buy),
        ("Might need — decide on site", &list.maybe),
        ("Already coming", &list.coming),
    ] {
        if !rows.is_empty() {
            blocks.push(Block::Heading(h.into()));
            blocks.push(table(rows));
        }
    }
    if blocks.len() == 1 {
        blocks.push(Block::Paragraph("No parts listed yet.".into()));
    }
    let doc = Document {
        title: format!("Parts list — {}", ticket.title),
        subtitle: Some(Utc::now().format("%b %-d, %Y").to_string()),
        organization: org,
        landscape: false,
        blocks,
    };
    Ok(ReportFile::new(
        crate::pdfdoc::render(&doc),
        "application/pdf",
        "parts-list.pdf".into(),
    ))
}

// ---------------------------------------------------------------------------
// Close-out
// ---------------------------------------------------------------------------

#[derive(Serialize, schemars::JsonSchema)]
pub struct CloseoutTicket {
    pub ticket_id: Uuid,
    pub title: String,
    pub property_id: Uuid,
    pub property: String,
    pub property_address: String,
    pub status: String,
    pub priority: String,
    pub due_date: Option<String>,
    pub assignee: Option<String>,
    pub parts: Vec<TicketPartDto>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct Closeout {
    pub date: String,
    /// Work orders due on `date` (or already overdue) with parts to decide.
    pub tickets: Vec<CloseoutTicket>,
    /// Parts to decide across them.
    pub to_decide: usize,
    /// Ordered parts not received yet.
    pub on_order: Vec<TicketPartDto>,
}

/// `GET /closeout?date` — tomorrow's work and what it still needs (default:
/// tomorrow). Open work orders due on or before the date, plus any work order
/// with a part still waiting on a decision.
#[rocket_okapi::openapi(tag = "Parts")]
#[get("/closeout?<date>")]
pub async fn closeout(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    date: Option<String>,
) -> ApiResult<Json<Closeout>> {
    user.require(Permission::MaintenanceRead)?;
    let rules = crate::workforce::Rules::load(&db, scope.tenant_id).await;
    let day = match date.filter(|d| !d.is_empty()) {
        Some(d) => crate::workforce::parse_date(&d, "date")?,
        None => rules.local_date(Utc::now().into()) + Duration::days(1),
    };
    let open_parts = TicketPart::find()
        .filter(entity::ticket_part::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::ticket_part::Column::Status.is_in(OPEN_STATUSES.to_vec()))
        .all(&db)
        .await?;
    let mut ticket_ids: Vec<Uuid> = open_parts.iter().map(|p| p.ticket_id).collect();
    let due = MaintenanceTicket::find()
        .filter(entity::maintenance_ticket::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::maintenance_ticket::Column::Status.is_in(super::OPEN_STATUSES.to_vec()))
        .filter(entity::maintenance_ticket::Column::DueDate.lte(day.to_string()))
        .all(&db)
        .await?;
    ticket_ids.extend(due.iter().map(|t| t.id));
    ticket_ids.sort();
    ticket_ids.dedup();
    let tickets = MaintenanceTicket::find()
        .filter(entity::maintenance_ticket::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::maintenance_ticket::Column::Id.is_in(ticket_ids.clone()))
        .filter(entity::maintenance_ticket::Column::Status.is_in(super::OPEN_STATUSES.to_vec()))
        .order_by_asc(entity::maintenance_ticket::Column::DueDate)
        .all(&db)
        .await?;
    let props: HashMap<Uuid, (String, String)> = Property::find()
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .all(&db)
        .await?
        .into_iter()
        .map(|p| (p.id, (p.name.clone(), crate::geo::full_address(&p))))
        .collect();
    let users: HashMap<Uuid, String> = User::find()
        .filter(
            entity::user::Column::Id.is_in(
                tickets
                    .iter()
                    .filter_map(|t| t.assignee_user_id)
                    .collect::<Vec<_>>(),
            ),
        )
        .all(&db)
        .await?
        .into_iter()
        .map(|u| (u.id, u.name))
        .collect();
    let all_parts = part_dtos(
        &db,
        scope.tenant_id,
        TicketPart::find()
            .filter(entity::ticket_part::Column::TenantId.eq(scope.tenant_id))
            .filter(entity::ticket_part::Column::TicketId.is_in(ticket_ids))
            .filter(entity::ticket_part::Column::Status.ne("used"))
            .filter(entity::ticket_part::Column::Status.ne("skipped"))
            .order_by_asc(entity::ticket_part::Column::CreatedAt)
            .all(&db)
            .await?,
    )
    .await?;
    let mut out = Vec::new();
    for t in tickets {
        let (name, addr) = props.get(&t.property_id).cloned().unwrap_or_default();
        out.push(CloseoutTicket {
            parts: all_parts
                .iter()
                .filter(|p| p.ticket_id == t.id)
                .cloned()
                .collect(),
            ticket_id: t.id,
            title: t.title,
            property_id: t.property_id,
            property: name,
            property_address: addr,
            status: t.status,
            priority: t.priority,
            due_date: t.due_date,
            assignee: t.assignee_user_id.and_then(|u| users.get(&u).cloned()),
        });
    }
    let on_order = part_dtos(
        &db,
        scope.tenant_id,
        TicketPart::find()
            .filter(entity::ticket_part::Column::TenantId.eq(scope.tenant_id))
            .filter(entity::ticket_part::Column::Status.is_in(["ordered", "pick_up"]))
            .order_by_asc(entity::ticket_part::Column::OrderedAt)
            .limit(200)
            .all(&db)
            .await?,
    )
    .await?;
    Ok(Json(Closeout {
        date: day.to_string(),
        to_decide: open_parts.len(),
        tickets: out,
        on_order,
    }))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct DecideReq {
    /// `order` | `pick_up` | `from_stock` | `skip`
    pub action: String,
    pub vendor: Option<String>,
    pub tracking: Option<String>,
    /// `property` | `office` | `other` (orders)
    pub ship_to: Option<String>,
    pub ship_to_note: Option<String>,
    pub unit_cost_cents: Option<i64>,
    /// Charge the owner for it (default true).
    pub billable_to_owner: Option<bool>,
    pub need_by: Option<String>,
}

/// Take a *from_stock* part off the shelf: a `use` movement and a part line on
/// the ticket, so costing and the owner bill see it.
async fn consume_from_stock(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    part: &entity::ticket_part::Model,
    actor: Uuid,
) -> ApiResult<Uuid> {
    let item_id = part
        .inventory_item_id
        .ok_or_else(|| ApiError::BadRequest("this part isn't a stock item".into()))?;
    let item = InventoryItem::find_by_id(item_id)
        .filter(entity::inventory_item::Column::TenantId.eq(tenant_id))
        .lock_exclusive()
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("stock item not found".into()))?;
    if item.quantity < part.quantity {
        return Err(ApiError::Conflict(format!(
            "only {} of {} on the shelf",
            item.quantity, item.name
        )));
    }
    let cost = item.unit_cost_cents.unwrap_or(0);
    let mut am: entity::inventory_item::ActiveModel = item.clone().into();
    am.quantity = Set(item.quantity - part.quantity);
    am.updated_at = Set(Utc::now().into());
    am.update(db).await?;
    super::stock::record_movement(
        db,
        tenant_id,
        item.id,
        "use",
        -part.quantity,
        cost,
        Some(part.ticket_id),
        None,
        Some(&part.name),
        Some(actor),
    )
    .await?;
    let line = entity::ticket_line::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        ticket_id: Set(part.ticket_id),
        kind: Set("part".into()),
        description: Set(part.name.clone()),
        inventory_item_id: Set(Some(item.id)),
        serial_number: Set(None),
        quantity: Set(part.quantity),
        unit_cost_cents: Set(cost),
        total_cents: Set(cost * part.quantity as i64),
        created_by: Set(Some(actor)),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await?;
    Ok(line.id)
}

/// `POST /parts/<id>/decide` — the close-out decision for one part.
#[rocket_okapi::openapi(tag = "Parts")]
#[post("/parts/<id>/decide", data = "<body>")]
pub async fn decide(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<DecideReq>,
) -> ApiResult<Json<TicketPartDto>> {
    user.require(Permission::MaintenanceManage)?;
    let p = find_part(&db, scope.tenant_id, id).await?;
    if !["potential", "needed", "to_order", "from_stock", "pick_up"].contains(&p.status.as_str()) {
        return Err(ApiError::Conflict(format!(
            "this part is already {}",
            p.status.replace('_', " ")
        )));
    }
    let ticket = MaintenanceTicket::find_by_id(p.ticket_id)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("work order not found".into()))?;
    let now = Utc::now();
    let clean = |s: Option<String>| s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
    let mut am: entity::ticket_part::ActiveModel = p.clone().into();
    match body.action.as_str() {
        "skip" => {
            am.status = Set("skipped".into());
        }
        "from_stock" => {
            let line_id = consume_from_stock(&db, scope.tenant_id, &p, user.user_id).await?;
            am.status = Set("used".into());
            am.ticket_line_id = Set(Some(line_id));
            super::lines::sync_ticket_cost(&db, scope.tenant_id, ticket.clone()).await?;
        }
        a @ ("order" | "pick_up") => {
            let ship = body.ship_to.clone().unwrap_or_else(|| {
                if a == "order" {
                    "property".into()
                } else {
                    "office".into()
                }
            });
            if !SHIP_TO.contains(&ship.as_str()) {
                return Err(ApiError::BadRequest(
                    "ship_to must be property, office or other".into(),
                ));
            }
            am.status = Set(if a == "order" { "ordered" } else { "pick_up" }.into());
            am.vendor = Set(clean(body.vendor.clone()));
            am.tracking = Set(clean(body.tracking.clone()));
            am.ship_to = Set(Some(ship));
            am.ship_to_note = Set(clean(body.ship_to_note.clone()));
            am.unit_cost_cents = Set(body.unit_cost_cents.filter(|c| *c >= 0));
            am.ordered_at = Set(Some(now.into()));
            am.ordered_by = Set(Some(user.user_id));
            if let Some(d) = clean(body.need_by.clone()) {
                d.parse::<NaiveDate>()
                    .map_err(|_| ApiError::BadRequest("need_by must be a date".into()))?;
                am.need_by = Set(Some(d));
            }
            // A priced order is an expense on the work order, billable to the owner.
            if let Some(cost) = body.unit_cost_cents.filter(|c| *c > 0) {
                let x = entity::expense::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    tenant_id: Set(scope.tenant_id),
                    incurred_on: Set(crate::workforce::Rules::load(&db, scope.tenant_id).await.local_date(now.into()).to_string()),
                    category: Set("materials".into()),
                    vendor: Set(clean(body.vendor.clone())),
                    description: Set(format!("{} × {}", p.quantity, p.name)),
                    amount_cents: Set(cost * p.quantity as i64),
                    miles_hundredths: Set(None),
                    mileage_rate_mills: Set(None),
                    tax_deductible: Set(true),
                    vehicle: Set("none".into()),
                    reimbursable: Set(false),
                    reimbursed_at: Set(None),
                    billable_to_owner: Set(body.billable_to_owner.unwrap_or(true)),
                    billed_bill_id: Set(None),
                    user_id: Set(None),
                    maintenance_ticket_id: Set(Some(ticket.id)),
                    rehab_project_id: Set(None),
                    property_id: Set(Some(ticket.property_id)),
                    asset_id: Set(ticket.asset_id),
                    details: Set(serde_json::json!({ "part_id": p.id, "tracking": clean(body.tracking.clone()) })),
                    recorded_by: Set(Some(user.user_id)),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(&db)
                .await?;
                am.expense_id = Set(Some(x.id));
            }
        }
        other => {
            return Err(ApiError::BadRequest(format!(
                "action must be order, pick_up, from_stock or skip, not {other}"
            )))
        }
    }
    am.updated_at = Set(now.into());
    let p = am.update(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::PART_DECIDE,
        Some("ticket_part"),
        Some(p.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "action": body.action, "status": p.status })),
    )
    .await;
    Ok(Json(
        part_dtos(&db, scope.tenant_id, vec![p]).await?.remove(0),
    ))
}

/// `POST /parts/<id>/receive` — an ordered / picked-up part arrived.
#[rocket_okapi::openapi(tag = "Parts")]
#[post("/parts/<id>/receive")]
pub async fn receive(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<TicketPartDto>> {
    user.require(Permission::MaintenanceManage)?;
    let p = find_part(&db, scope.tenant_id, id).await?;
    if p.status != "ordered" && p.status != "pick_up" {
        return Err(ApiError::Conflict(
            "only an ordered or pick-up part can be received".into(),
        ));
    }
    let mut am: entity::ticket_part::ActiveModel = p.into();
    am.status = Set("received".into());
    am.received_at = Set(Some(Utc::now().into()));
    am.updated_at = Set(Utc::now().into());
    let p = am.update(&db).await?;
    Ok(Json(
        part_dtos(&db, scope.tenant_id, vec![p]).await?.remove(0),
    ))
}

/// `POST /parts/<id>/use` — the part went into the unit: it becomes a part
/// line on the work order (at what it cost), so the owner bill carries it.
#[rocket_okapi::openapi(tag = "Parts")]
#[post("/parts/<id>/use")]
pub async fn use_part(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<TicketLineDto>> {
    user.require(Permission::MaintenanceManage)?;
    let p = find_part(&db, scope.tenant_id, id).await?;
    let ticket = MaintenanceTicket::find_by_id(p.ticket_id)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("work order not found".into()))?;
    let line_id = match p.status.as_str() {
        "from_stock" => consume_from_stock(&db, scope.tenant_id, &p, user.user_id).await?,
        "received" | "ordered" | "pick_up" | "needed" | "potential" | "to_order" => {
            // Bought for this job: the cost came with the order (or is 0).
            let cost = p.unit_cost_cents.unwrap_or(0);
            entity::ticket_line::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(scope.tenant_id),
                ticket_id: Set(p.ticket_id),
                kind: Set("part".into()),
                description: Set(p.name.clone()),
                inventory_item_id: Set(None),
                serial_number: Set(None),
                quantity: Set(p.quantity),
                unit_cost_cents: Set(cost),
                total_cents: Set(cost * p.quantity as i64),
                created_by: Set(Some(user.user_id)),
                created_at: Set(Utc::now().into()),
            }
            .insert(&db)
            .await?
            .id
        }
        s => return Err(ApiError::Conflict(format!("this part is {s}"))),
    };
    let mut am: entity::ticket_part::ActiveModel = p.into();
    am.status = Set("used".into());
    am.ticket_line_id = Set(Some(line_id));
    am.updated_at = Set(Utc::now().into());
    am.update(&db).await?;
    super::lines::sync_ticket_cost(&db, scope.tenant_id, ticket).await?;
    let line = entity::prelude::TicketLine::find_by_id(line_id)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("line not found".into()))?;
    Ok(Json(TicketLineDto::from(line)))
}

// ---------------------------------------------------------------------------
// Appliances: catalog + replace / repair
// ---------------------------------------------------------------------------

#[derive(Serialize, schemars::JsonSchema)]
pub struct AssetPartDto {
    pub id: Uuid,
    pub inventory_item_id: Uuid,
    pub name: String,
    pub sku: Option<String>,
    pub quantity: i32,
    pub role: Option<String>,
    pub note: Option<String>,
    pub in_stock: i32,
    pub unit_cost_cents: Option<i64>,
}

async fn find_asset(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    id: &str,
) -> ApiResult<entity::asset::Model> {
    Asset::find_by_id(parse_id(id, "asset")?)
        .filter(entity::asset::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("appliance not found".into()))
}

pub(super) async fn catalog_for(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    asset_id: Uuid,
) -> ApiResult<Vec<AssetPartDto>> {
    let rows = AssetPart::find()
        .filter(entity::asset_part::Column::TenantId.eq(tenant_id))
        .filter(entity::asset_part::Column::AssetId.eq(asset_id))
        .order_by_asc(entity::asset_part::Column::CreatedAt)
        .all(db)
        .await?;
    let items: HashMap<Uuid, entity::inventory_item::Model> = InventoryItem::find()
        .filter(
            entity::inventory_item::Column::Id
                .is_in(rows.iter().map(|r| r.inventory_item_id).collect::<Vec<_>>()),
        )
        .all(db)
        .await?
        .into_iter()
        .map(|i| (i.id, i))
        .collect();
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            let i = items.get(&r.inventory_item_id)?;
            Some(AssetPartDto {
                id: r.id,
                inventory_item_id: i.id,
                name: i.name.clone(),
                sku: i.sku.clone(),
                quantity: r.quantity,
                role: r.role,
                note: r.note,
                in_stock: i.quantity,
                unit_cost_cents: i.unit_cost_cents,
            })
        })
        .collect())
}

/// `GET /assets/<id>/parts` — the parts that fit this appliance.
#[rocket_okapi::openapi(tag = "Parts")]
#[get("/assets/<id>/parts")]
pub async fn asset_parts(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Vec<AssetPartDto>>> {
    user.require(Permission::MaintenanceRead)?;
    let a = find_asset(&db, scope.tenant_id, id).await?;
    Ok(Json(catalog_for(&db, scope.tenant_id, a.id).await?))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct AssetPartReq {
    pub inventory_item_id: Uuid,
    pub quantity: Option<i32>,
    pub role: Option<String>,
    pub note: Option<String>,
}

/// `PUT /assets/<id>/parts` — add (or update) a part that fits.
#[rocket_okapi::openapi(tag = "Parts")]
#[put("/assets/<id>/parts", data = "<body>")]
pub async fn put_asset_part(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<AssetPartReq>,
) -> ApiResult<Json<Vec<AssetPartDto>>> {
    user.require(Permission::MaintenanceManage)?;
    let a = find_asset(&db, scope.tenant_id, id).await?;
    InventoryItem::find_by_id(body.inventory_item_id)
        .filter(entity::inventory_item::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("stock item not found".into()))?;
    let clean = |s: Option<String>| s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
    let existing = AssetPart::find()
        .filter(entity::asset_part::Column::AssetId.eq(a.id))
        .filter(entity::asset_part::Column::InventoryItemId.eq(body.inventory_item_id))
        .one(&db)
        .await?;
    match existing {
        Some(e) => {
            let mut am: entity::asset_part::ActiveModel = e.into();
            am.quantity = Set(body.quantity.unwrap_or(1).max(1));
            am.role = Set(clean(body.role.clone()));
            am.note = Set(clean(body.note.clone()));
            am.update(&db).await?;
        }
        None => {
            entity::asset_part::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(scope.tenant_id),
                asset_id: Set(a.id),
                inventory_item_id: Set(body.inventory_item_id),
                quantity: Set(body.quantity.unwrap_or(1).max(1)),
                role: Set(clean(body.role.clone())),
                note: Set(clean(body.note.clone())),
                created_at: Set(Utc::now().into()),
            }
            .insert(&db)
            .await?;
        }
    }
    Ok(Json(catalog_for(&db, scope.tenant_id, a.id).await?))
}

/// `DELETE /assets/<id>/parts/<part_id>`.
#[rocket_okapi::openapi(tag = "Parts")]
#[delete("/assets/<id>/parts/<part_id>")]
pub async fn delete_asset_part(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    part_id: &str,
) -> ApiResult<Json<Vec<AssetPartDto>>> {
    user.require(Permission::MaintenanceManage)?;
    let a = find_asset(&db, scope.tenant_id, id).await?;
    let pid = parse_id(part_id, "part")?;
    AssetPart::delete_many()
        .filter(entity::asset_part::Column::Id.eq(pid))
        .filter(entity::asset_part::Column::AssetId.eq(a.id))
        .exec(&db)
        .await?;
    Ok(Json(catalog_for(&db, scope.tenant_id, a.id).await?))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct AssetWorkReq {
    /// `repair` | `replace` | `service`
    pub kind: String,
    pub description: Option<String>,
    pub priority: Option<String>,
    pub due_date: Option<String>,
}

/// `POST /assets/<id>/work-order` — open a work order on this appliance with
/// its parts pre-listed as *potential*.
#[rocket_okapi::openapi(tag = "Parts")]
#[post("/assets/<id>/work-order", data = "<body>")]
pub async fn asset_work_order(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<AssetWorkReq>,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::MaintenanceManage)?;
    let a = find_asset(&db, scope.tenant_id, id).await?;
    let verb = match body.kind.as_str() {
        "repair" => "Repair",
        "replace" => "Replace",
        "service" => "Service",
        k => {
            return Err(ApiError::BadRequest(format!(
                "kind must be repair, replace or service, not {k}"
            )))
        }
    };
    let category = match a.kind.as_str() {
        "hvac" => "hvac",
        "plumbing" => "plumbing",
        "electrical" => "electrical",
        "appliance" => "appliance",
        _ => "general",
    };
    let ticket = crate::helpdesk::open_ticket(
        &db,
        scope.tenant_id,
        crate::helpdesk::OpenTicket {
            property_id: a.property_id,
            unit_id: a.unit_id,
            lease_id: None,
            title: format!("{verb} {}", a.name),
            description: body.description.clone().or_else(|| {
                Some(
                    format!(
                        "{verb} the {} ({} {}).",
                        a.name,
                        a.make.clone().unwrap_or_default(),
                        a.model.clone().unwrap_or_default()
                    )
                    .trim()
                    .to_string(),
                )
            }),
            category: category.into(),
            priority: body.priority.clone().unwrap_or_else(|| "normal".into()),
            reporter: None,
            due_date: body.due_date.clone(),
        },
        Some(user.user_id),
    )
    .await?;
    let mut am: entity::maintenance_ticket::ActiveModel = ticket.clone().into();
    am.asset_id = Set(Some(a.id));
    am.update(&db).await?;
    let listed =
        add_potential_from_asset(&db, scope.tenant_id, ticket.id, a.id, Some(user.user_id)).await?;
    Ok(Json(
        serde_json::json!({ "ticket_id": ticket.id, "title": ticket.title, "potential_parts": listed }),
    ))
}

// ---------------------------------------------------------------------------
// Appliance service history
// ---------------------------------------------------------------------------

#[derive(Serialize, schemars::JsonSchema)]
pub struct AssetHistory {
    #[serde(flatten)]
    pub asset: super::dto::AssetDto,
    pub property: String,
    /// Work orders on this appliance, newest first.
    pub tickets: Vec<super::dto::TicketDto>,
    /// Routine maintenance scheduled on it.
    pub plans: Vec<super::dto::MaintenancePlanDto>,
    /// Warranty papers, manuals, receipts.
    pub documents: Vec<crate::routes::documents::dto::DocumentDto>,
    /// The parts that fit.
    pub parts: Vec<AssetPartDto>,
    /// Everything spent on it: ticket costs + expenses booked to the asset.
    pub spend_cents: i64,
    pub spend_label: String,
    pub last_serviced: Option<String>,
}

/// `GET /assets/<id>/history` — one appliance's whole story.
#[rocket_okapi::openapi(tag = "Parts")]
#[get("/assets/<id>/history")]
pub async fn asset_history(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<AssetHistory>> {
    user.require(Permission::MaintenanceRead)?;
    let a = find_asset(&db, scope.tenant_id, id).await?;
    let property = Property::find_by_id(a.property_id)
        .one(&db)
        .await?
        .map(|p| p.name)
        .unwrap_or_default();
    let tickets = MaintenanceTicket::find()
        .filter(entity::maintenance_ticket::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::maintenance_ticket::Column::AssetId.eq(a.id))
        .order_by_desc(entity::maintenance_ticket::Column::CreatedAt)
        .all(&db)
        .await?;
    let ticket_ids: Vec<Uuid> = tickets.iter().map(|t| t.id).collect();
    let mut spend: i64 = tickets.iter().map(|t| t.cost_cents.unwrap_or(0)).sum();
    // Expenses on the asset that aren't already inside a ticket's cost.
    spend += entity::prelude::Expense::find()
        .filter(entity::expense::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::expense::Column::AssetId.eq(a.id))
        .all(&db)
        .await?
        .into_iter()
        .filter(|x| {
            x.maintenance_ticket_id
                .map(|t| !ticket_ids.contains(&t))
                .unwrap_or(true)
        })
        .map(|x| x.amount_cents)
        .sum::<i64>();
    let last_serviced = tickets
        .iter()
        .filter_map(|t| t.resolved_at)
        .max()
        .map(|d| d.format("%Y-%m-%d").to_string());
    let plans = entity::prelude::MaintenancePlan::find()
        .filter(entity::maintenance_plan::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::maintenance_plan::Column::AssetId.eq(a.id))
        .order_by_asc(entity::maintenance_plan::Column::NextDueDate)
        .all(&db)
        .await?;
    let documents = entity::prelude::Document::find()
        .filter(entity::document::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::document::Column::OwnerType.eq("asset"))
        .filter(entity::document::Column::OwnerId.eq(a.id))
        .filter(entity::document::Column::Status.ne("deleted"))
        .order_by_desc(entity::document::Column::CreatedAt)
        .all(&db)
        .await?;
    Ok(Json(AssetHistory {
        parts: catalog_for(&db, scope.tenant_id, a.id).await?,
        asset: super::dto::AssetDto::from(a),
        property,
        tickets: tickets
            .into_iter()
            .map(super::dto::TicketDto::from)
            .collect(),
        plans: plans
            .into_iter()
            .map(super::dto::MaintenancePlanDto::from)
            .collect(),
        documents: documents
            .into_iter()
            .map(crate::routes::documents::dto::DocumentDto::from)
            .collect(),
        spend_cents: spend,
        spend_label: crate::dto::usd(spend),
        last_serviced,
    }))
}
