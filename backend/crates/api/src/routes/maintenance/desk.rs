//! The **service desk** on one work order: its tasks (by trade, with vendors
//! for the ones that need a contractor), kits added after the fact, photos and
//! receipts, what it's costing against the estimate, and expenses with their
//! receipts. Everything is addressed through `/tickets/<id>/…`, so property
//! reach applies (see [`crate::tenancy::access`]).

use crate::audit::actions as act;
use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::servicedesk::{self, TASK_STATUSES, TRADES};
use crate::storage::{ObjectStore, SIGNED_URL_TTL_SECS};
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{
    Counterparty, Document, Expense, IssueTemplate, MaintenanceTicket, Property, TicketLine,
    TicketPart, TicketQuote, TicketTask,
};
use rocket::serde::json::Json;
use rocket::{delete, get, patch, post};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Shapes
// ---------------------------------------------------------------------------

#[derive(Serialize, schemars::JsonSchema)]
pub struct TaskDto {
    pub id: Uuid,
    pub position: i32,
    pub title: String,
    pub trade: String,
    pub est_minutes: Option<i32>,
    pub est_cost_cents: Option<i64>,
    pub est_cost_label: Option<String>,
    pub needs_contractor: bool,
    pub assignee_entity_id: Option<Uuid>,
    pub assignee_name: Option<String>,
    pub status: String,
    pub done_at: Option<String>,
    pub dispatched_at: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct TaskReq {
    pub title: String,
    pub trade: Option<String>,
    pub est_minutes: Option<i32>,
    pub needs_contractor: Option<bool>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct TaskPatch {
    pub title: Option<String>,
    pub trade: Option<String>,
    pub est_minutes: Option<i32>,
    pub needs_contractor: Option<bool>,
    /// `todo` | `doing` | `done` | `skipped`.
    pub status: Option<String>,
    /// A vendor's id, or `""` to clear.
    pub assignee_entity_id: Option<String>,
    pub position: Option<i32>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct DispatchTaskReq {
    pub entity_id: Uuid,
    pub note: Option<String>,
    /// Why to send a vendor without current insurance, when that's required.
    pub coi_override_reason: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ApplyKitReq {
    pub issue_template_id: Uuid,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct TradeNeed {
    pub trade: String,
    /// Open contractor tasks in this trade.
    pub open_tasks: usize,
    /// Whether every one of them has a vendor.
    pub covered: bool,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct CostSummary {
    pub ticket_id: Uuid,
    /// Labor estimate from the tasks still to do or done (skipped excluded).
    pub est_labor_cents: i64,
    /// Parts on the list (not "maybe"), at stock or typical cost.
    pub est_parts_cents: i64,
    pub est_total_cents: i64,
    pub est_total_label: String,
    /// Recorded line items (parts used, labor, fees).
    pub lines_cents: i64,
    /// Expenses logged against the work order (receipts).
    pub expenses_cents: i64,
    /// Approved vendor quotes.
    pub approved_quotes_cents: i64,
    pub actual_total_cents: i64,
    pub actual_total_label: String,
    /// Actual minus estimate (positive = over).
    pub variance_cents: i64,
    pub variance_label: String,
    pub receipts: usize,
    pub tasks_total: usize,
    pub tasks_done: usize,
    pub trades_needed: Vec<TradeNeed>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UploadReq {
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: Option<i64>,
    /// `photo` | `receipt` | `document` (default `photo` for images).
    pub kind: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct FileDto {
    pub id: Uuid,
    pub filename: String,
    pub mime_type: String,
    /// `photo` | `receipt` | `document`.
    pub kind: String,
    pub size_bytes: i64,
    /// A signed link, good for 15 minutes.
    pub url: Option<String>,
    pub created_at: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct UploadResp {
    pub file: FileDto,
    pub upload_url: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct TicketExpenseReq {
    pub description: String,
    pub amount_cents: i64,
    pub vendor: Option<String>,
    /// `materials` (default), `repairs`, `equipment`, `other`, …
    pub category: Option<String>,
    /// `YYYY-MM-DD`, default today.
    pub incurred_on: Option<String>,
    /// Receipt files already uploaded to this work order.
    #[serde(default)]
    pub receipt_document_ids: Vec<Uuid>,
    #[serde(default)]
    pub billable_to_owner: bool,
    /// Paid out of pocket, to be paid back.
    #[serde(default)]
    pub reimbursable: bool,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct TicketExpenseDto {
    pub id: Uuid,
    pub incurred_on: String,
    pub category: String,
    pub vendor: Option<String>,
    pub description: String,
    pub amount_cents: i64,
    pub amount_label: String,
    pub billable_to_owner: bool,
    pub reimbursable: bool,
    pub receipt_document_ids: Vec<Uuid>,
    pub created_at: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct VendorOption {
    pub id: Uuid,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub trades: Vec<String>,
    /// Covers the trade asked about.
    pub matches: bool,
    /// Has general liability insurance in force today.
    pub coi_current: bool,
    /// Linked to a partner system (e.g. Alpha Power Wash): work orders go
    /// straight into their job board.
    pub linked: bool,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn uuid(raw: &str, what: &str) -> ApiResult<Uuid> {
    Uuid::parse_str(raw).map_err(|_| ApiError::BadRequest(format!("invalid {what} id")))
}

async fn ticket(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    id: &str,
) -> ApiResult<entity::maintenance_ticket::Model> {
    MaintenanceTicket::find_by_id(uuid(id, "work order")?)
        .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("work order not found".into()))
}

async fn task(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    ticket_id: Uuid,
    id: &str,
) -> ApiResult<entity::ticket_task::Model> {
    TicketTask::find_by_id(uuid(id, "task")?)
        .filter(entity::ticket_task::Column::TenantId.eq(tenant_id))
        .filter(entity::ticket_task::Column::TicketId.eq(ticket_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("task not found".into()))
}

fn trade_or_general(raw: Option<&str>) -> ApiResult<String> {
    match raw.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok("general".into()),
        Some(t) if TRADES.contains(&t) => Ok(t.to_string()),
        Some(t) => Err(ApiError::BadRequest(format!(
            "unknown trade {t} (expected one of {})",
            TRADES.join(", ")
        ))),
    }
}

async fn tasks_of(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    ticket_id: Uuid,
) -> ApiResult<Vec<TaskDto>> {
    let rows = TicketTask::find()
        .filter(entity::ticket_task::Column::TenantId.eq(tenant_id))
        .filter(entity::ticket_task::Column::TicketId.eq(ticket_id))
        .order_by_asc(entity::ticket_task::Column::Position)
        .order_by_asc(entity::ticket_task::Column::CreatedAt)
        .all(db)
        .await?;
    let ids: Vec<Uuid> = rows.iter().filter_map(|t| t.assignee_entity_id).collect();
    let names: HashMap<Uuid, String> = if ids.is_empty() {
        HashMap::new()
    } else {
        Counterparty::find()
            .filter(entity::counterparty::Column::TenantId.eq(tenant_id))
            .filter(entity::counterparty::Column::Id.is_in(ids))
            .all(db)
            .await?
            .into_iter()
            .map(|c| (c.id, c.name))
            .collect()
    };
    Ok(rows
        .into_iter()
        .map(|t| TaskDto {
            assignee_name: t.assignee_entity_id.and_then(|i| names.get(&i).cloned()),
            est_cost_label: t.est_cost_cents.map(usd),
            id: t.id,
            position: t.position,
            title: t.title,
            trade: t.trade,
            est_minutes: t.est_minutes,
            est_cost_cents: t.est_cost_cents,
            needs_contractor: t.needs_contractor,
            assignee_entity_id: t.assignee_entity_id,
            status: t.status,
            done_at: t.done_at.map(|d| d.to_rfc3339()),
            dispatched_at: t.dispatched_at.map(|d| d.to_rfc3339()),
        })
        .collect())
}

fn kind_of(category: Option<&str>) -> &'static str {
    match category {
        Some("photo") => "photo",
        Some("receipt") => "receipt",
        _ => "document",
    }
}

async fn record(
    db: &impl ConnectionTrait,
    user: &AuthUser,
    scope: &TenantScope,
    action: &str,
    ticket_id: Uuid,
    detail: serde_json::Value,
) {
    crate::audit::record(
        db,
        Some(user.user_id),
        action,
        Some("maintenance_ticket"),
        Some(ticket_id.to_string()),
        Some(scope.tenant_id),
        Some(detail),
    )
    .await;
}

// ---------------------------------------------------------------------------
// Tasks
// ---------------------------------------------------------------------------

/// `GET /tickets/<id>/tasks` — the work, line by line.
#[rocket_okapi::openapi(tag = "Service Desk")]
#[get("/tickets/<id>/tasks")]
pub async fn list_tasks(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Vec<TaskDto>>> {
    user.require(Permission::MaintenanceRead)?;
    let t = ticket(&db, scope.tenant_id, id).await?;
    Ok(Json(tasks_of(&db, scope.tenant_id, t.id).await?))
}

/// `POST /tickets/<id>/tasks` — add a task at the end.
#[rocket_okapi::openapi(tag = "Service Desk")]
#[post("/tickets/<id>/tasks", data = "<body>")]
pub async fn add_task(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<TaskReq>,
) -> ApiResult<Json<Vec<TaskDto>>> {
    user.require(Permission::MaintenanceManage)?;
    let t = ticket(&db, scope.tenant_id, id).await?;
    let b = body.into_inner();
    let title = b.title.trim().to_string();
    if title.is_empty() {
        return Err(ApiError::BadRequest("a task needs a title".into()));
    }
    let trade = trade_or_general(b.trade.as_deref())?;
    let minutes = b.est_minutes.filter(|m| *m > 0);
    let contractor = b.needs_contractor.unwrap_or(false);
    let rates = servicedesk::rates(&db, scope.tenant_id).await;
    let next = TicketTask::find()
        .filter(entity::ticket_task::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::ticket_task::Column::TicketId.eq(t.id))
        .order_by_desc(entity::ticket_task::Column::Position)
        .one(&db)
        .await?
        .map(|x| x.position + 1)
        .unwrap_or(0);
    let now = Utc::now();
    let saved = entity::ticket_task::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        ticket_id: Set(t.id),
        position: Set(next),
        title: Set(title),
        trade: Set(trade),
        est_minutes: Set(minutes),
        est_cost_cents: Set(servicedesk::task_cost(minutes, contractor, rates)),
        needs_contractor: Set(contractor),
        assignee_entity_id: Set(None),
        status: Set("todo".into()),
        done_at: Set(None),
        done_by: Set(None),
        dispatched_at: Set(None),
        created_by: Set(Some(user.user_id)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;
    record(
        &db,
        &user,
        &scope,
        act::TICKET_TASK_SAVE,
        t.id,
        json!({ "task_id": saved.id, "title": saved.title }),
    )
    .await;
    Ok(Json(tasks_of(&db, scope.tenant_id, t.id).await?))
}

/// `PATCH /tickets/<id>/tasks/<task_id>` — tick it off, re-time it, give it
/// to a vendor, or move it.
#[rocket_okapi::openapi(tag = "Service Desk")]
#[patch("/tickets/<id>/tasks/<task_id>", data = "<body>")]
pub async fn update_task(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    task_id: &str,
    body: Json<TaskPatch>,
) -> ApiResult<Json<Vec<TaskDto>>> {
    user.require(Permission::MaintenanceManage)?;
    let t = ticket(&db, scope.tenant_id, id).await?;
    let existing = task(&db, scope.tenant_id, t.id, task_id).await?;
    let b = body.into_inner();
    let rates = servicedesk::rates(&db, scope.tenant_id).await;
    let mut minutes = existing.est_minutes;
    let mut contractor = existing.needs_contractor;
    let mut am: entity::ticket_task::ActiveModel = existing.clone().into();
    if let Some(title) = b.title.map(|s| s.trim().to_string()) {
        if title.is_empty() {
            return Err(ApiError::BadRequest("a task needs a title".into()));
        }
        am.title = Set(title);
    }
    if let Some(trade) = b.trade {
        am.trade = Set(trade_or_general(Some(&trade))?);
    }
    if let Some(m) = b.est_minutes {
        minutes = Some(m).filter(|m| *m > 0);
        am.est_minutes = Set(minutes);
    }
    if let Some(c) = b.needs_contractor {
        contractor = c;
        am.needs_contractor = Set(c);
    }
    if minutes != existing.est_minutes || contractor != existing.needs_contractor {
        am.est_cost_cents = Set(servicedesk::task_cost(minutes, contractor, rates));
    }
    if let Some(status) = b.status.map(|s| s.trim().to_lowercase()) {
        if !TASK_STATUSES.contains(&status.as_str()) {
            return Err(ApiError::BadRequest(format!(
                "status must be one of {}",
                TASK_STATUSES.join(", ")
            )));
        }
        if status == "done" && existing.status != "done" {
            am.done_at = Set(Some(Utc::now().into()));
            am.done_by = Set(Some(user.user_id));
        } else if status != "done" {
            am.done_at = Set(None);
            am.done_by = Set(None);
        }
        am.status = Set(status);
    }
    if let Some(raw) = b.assignee_entity_id {
        let vendor = match raw.trim() {
            "" => None,
            v => {
                let vid = uuid(v, "vendor")?;
                Counterparty::find_by_id(vid)
                    .filter(entity::counterparty::Column::TenantId.eq(scope.tenant_id))
                    .one(&db)
                    .await?
                    .ok_or_else(|| ApiError::NotFound("vendor not found".into()))?;
                Some(vid)
            }
        };
        am.assignee_entity_id = Set(vendor);
    }
    if let Some(p) = b.position {
        am.position = Set(p.max(0));
    }
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&db).await?;
    record(
        &db,
        &user,
        &scope,
        act::TICKET_TASK_SAVE,
        t.id,
        json!({ "task_id": saved.id, "title": saved.title, "status": saved.status }),
    )
    .await;
    Ok(Json(tasks_of(&db, scope.tenant_id, t.id).await?))
}

/// `DELETE /tickets/<id>/tasks/<task_id>` — take a task off the work order.
#[rocket_okapi::openapi(tag = "Service Desk")]
#[delete("/tickets/<id>/tasks/<task_id>")]
pub async fn remove_task(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    task_id: &str,
) -> ApiResult<Json<Vec<TaskDto>>> {
    user.require(Permission::MaintenanceManage)?;
    let t = ticket(&db, scope.tenant_id, id).await?;
    let existing = task(&db, scope.tenant_id, t.id, task_id).await?;
    TicketTask::delete_by_id(existing.id).exec(&db).await?;
    record(
        &db,
        &user,
        &scope,
        act::TICKET_TASK_REMOVE,
        t.id,
        json!({ "task_id": existing.id, "title": existing.title }),
    )
    .await;
    Ok(Json(tasks_of(&db, scope.tenant_id, t.id).await?))
}

/// `POST /tickets/<id>/tasks/<task_id>/dispatch` — send this task to a vendor.
/// A vendor linked to a partner system (Alpha Power Wash and the like) gets
/// the job in their own board; any other vendor gets the work order by email.
/// The insurance rule applies.
#[rocket_okapi::openapi(tag = "Service Desk")]
#[post("/tickets/<id>/tasks/<task_id>/dispatch", data = "<body>")]
pub async fn dispatch_task(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    task_id: &str,
    body: Json<DispatchTaskReq>,
) -> ApiResult<Json<Vec<TaskDto>>> {
    user.require(Permission::MaintenanceManage)?;
    let t = ticket(&db, scope.tenant_id, id).await?;
    let existing = task(&db, scope.tenant_id, t.id, task_id).await?;
    let b = body.into_inner();
    let vendor = Counterparty::find_by_id(b.entity_id)
        .filter(entity::counterparty::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("vendor not found".into()))?;
    crate::vendor_compliance::check_dispatch(
        &db,
        scope.tenant_id,
        vendor.id,
        b.coi_override_reason.as_deref(),
        Some(user.user_id),
        t.id,
    )
    .await?;
    let note = b
        .note
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty());
    let task_line = format!("Task: {} ({})", existing.title, existing.trade);
    let how = if vendor.partner_kind.is_some() {
        crate::partner::dispatch(
            &db,
            scope.tenant_id,
            crate::partner::DispatchSpec {
                ticket_id: t.id,
                counterparty_id: vendor.id,
                requested_for: None,
                service_key: None,
                note: Some(match &note {
                    Some(n) => format!("{task_line}\n{n}"),
                    None => task_line.clone(),
                }),
            },
            Some(user.user_id),
        )
        .await?;
        "partner"
    } else {
        let Some(email) = vendor.email.as_deref().filter(|e| !e.trim().is_empty()) else {
            return Err(ApiError::BadRequest(format!(
                "{} has no email on file; add one, or call them",
                vendor.name
            )));
        };
        let property = Property::find_by_id(t.property_id)
            .one(&db)
            .await?
            .map(|p| format!("{}, {}", p.address, p.city))
            .unwrap_or_default();
        let mut description = task_line.clone();
        if let Some(n) = &note {
            description.push_str(&format!("\n{n}"));
        }
        if let Some(d) = t.description.as_deref().filter(|d| !d.is_empty()) {
            description.push_str(&format!("\n\n{d}"));
        }
        crate::scheduler::enqueue(
            &db,
            scope.tenant_id,
            "auto_email",
            json!({
                "template": "ticket_dispatch",
                "to": email,
                "owner_type": "maintenance_ticket",
                "owner_id": t.id,
                "trigger": format!("task_dispatch:{}:{}", existing.id, vendor.id),
                "vars": {
                    "title": format!("{} — {}", t.title, existing.title),
                    "priority": t.priority,
                    "property": property,
                    "due_line": t.due_date.as_deref().map(|d| format!(", wanted by {d}")).unwrap_or_default(),
                    "description": description,
                },
            }),
            0,
        )
        .await?;
        "email"
    };
    let mut am: entity::ticket_task::ActiveModel = existing.clone().into();
    am.assignee_entity_id = Set(Some(vendor.id));
    am.dispatched_at = Set(Some(Utc::now().into()));
    am.updated_at = Set(Utc::now().into());
    am.update(&db).await?;
    record(
        &db,
        &user,
        &scope,
        act::TICKET_TASK_DISPATCH,
        t.id,
        json!({ "task_id": existing.id, "vendor_id": vendor.id, "vendor": vendor.name, "via": how }),
    )
    .await;
    Ok(Json(tasks_of(&db, scope.tenant_id, t.id).await?))
}

/// `POST /tickets/<id>/kits` — add a job kit's tasks and parts to a work order
/// that's already open (e.g. a leak that turns out to need a new shower).
#[rocket_okapi::openapi(tag = "Service Desk")]
#[post("/tickets/<id>/kits", data = "<body>")]
pub async fn apply_kit(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<ApplyKitReq>,
) -> ApiResult<Json<Vec<TaskDto>>> {
    user.require(Permission::MaintenanceManage)?;
    let t = ticket(&db, scope.tenant_id, id).await?;
    let kit = IssueTemplate::find_by_id(body.issue_template_id)
        .filter(entity::issue_template::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::issue_template::Column::Active.eq(true))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("kit not found".into()))?;
    let (tasks, parts) =
        servicedesk::apply_kit(&db, scope.tenant_id, t.id, &kit, Some(user.user_id)).await?;
    record(
        &db,
        &user,
        &scope,
        act::TICKET_KIT_APPLY,
        t.id,
        json!({ "kit": kit.name, "tasks": tasks, "parts": parts }),
    )
    .await;
    Ok(Json(tasks_of(&db, scope.tenant_id, t.id).await?))
}

// ---------------------------------------------------------------------------
// Costs
// ---------------------------------------------------------------------------

/// `GET /tickets/<id>/costs` — the estimate (tasks and parts) against what's
/// been spent (line items, expenses with receipts, approved quotes), and the
/// trades still needing a vendor.
#[rocket_okapi::openapi(tag = "Service Desk")]
#[get("/tickets/<id>/costs")]
pub async fn costs(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<CostSummary>> {
    user.require(Permission::MaintenanceRead)?;
    let t = ticket(&db, scope.tenant_id, id).await?;
    let tenant_id = scope.tenant_id;
    let tasks = TicketTask::find()
        .filter(entity::ticket_task::Column::TenantId.eq(tenant_id))
        .filter(entity::ticket_task::Column::TicketId.eq(t.id))
        .all(&db)
        .await?;
    let est_labor: i64 = tasks
        .iter()
        .filter(|x| x.status != "skipped")
        .filter_map(|x| x.est_cost_cents)
        .sum();
    let parts = TicketPart::find()
        .filter(entity::ticket_part::Column::TenantId.eq(tenant_id))
        .filter(entity::ticket_part::Column::TicketId.eq(t.id))
        .all(&db)
        .await?;
    let stock_cost: HashMap<Uuid, i64> = {
        let ids: Vec<Uuid> = parts.iter().filter_map(|p| p.inventory_item_id).collect();
        if ids.is_empty() {
            HashMap::new()
        } else {
            entity::prelude::InventoryItem::find()
                .filter(entity::inventory_item::Column::TenantId.eq(tenant_id))
                .filter(entity::inventory_item::Column::Id.is_in(ids))
                .all(&db)
                .await?
                .into_iter()
                .filter_map(|i| i.unit_cost_cents.map(|c| (i.id, c)))
                .collect()
        }
    };
    let est_parts: i64 = parts
        .iter()
        .filter(|p| !["potential", "skipped"].contains(&p.status.as_str()))
        .map(|p| {
            let each = p
                .unit_cost_cents
                .or_else(|| {
                    p.inventory_item_id
                        .and_then(|i| stock_cost.get(&i).copied())
                })
                .unwrap_or(0);
            each * p.quantity.max(1) as i64
        })
        .sum();
    let lines: i64 = TicketLine::find()
        .filter(entity::ticket_line::Column::TenantId.eq(tenant_id))
        .filter(entity::ticket_line::Column::TicketId.eq(t.id))
        .all(&db)
        .await?
        .iter()
        .map(|l| l.total_cents)
        .sum();
    let expenses = Expense::find()
        .filter(entity::expense::Column::TenantId.eq(tenant_id))
        .filter(entity::expense::Column::MaintenanceTicketId.eq(t.id))
        .all(&db)
        .await?;
    let expenses_cents: i64 = expenses.iter().map(|e| e.amount_cents).sum();
    let receipts = Document::find()
        .filter(entity::document::Column::TenantId.eq(tenant_id))
        .filter(entity::document::Column::OwnerType.eq("maintenance_ticket"))
        .filter(entity::document::Column::OwnerId.eq(t.id))
        .filter(entity::document::Column::Category.eq("receipt"))
        .filter(entity::document::Column::Status.eq("stored"))
        .all(&db)
        .await?
        .len();
    let quotes: i64 = TicketQuote::find()
        .filter(entity::ticket_quote::Column::TenantId.eq(tenant_id))
        .filter(entity::ticket_quote::Column::TicketId.eq(t.id))
        .filter(entity::ticket_quote::Column::Status.eq("approved"))
        .all(&db)
        .await?
        .iter()
        .map(|q| q.amount_cents)
        .sum();

    let mut needs: Vec<TradeNeed> = vec![];
    for x in tasks
        .iter()
        .filter(|x| x.needs_contractor && !["done", "skipped"].contains(&x.status.as_str()))
    {
        match needs.iter_mut().find(|n| n.trade == x.trade) {
            Some(n) => {
                n.open_tasks += 1;
                n.covered &= x.assignee_entity_id.is_some();
            }
            None => needs.push(TradeNeed {
                trade: x.trade.clone(),
                open_tasks: 1,
                covered: x.assignee_entity_id.is_some(),
            }),
        }
    }

    let est = est_labor + est_parts;
    let actual = lines + expenses_cents + quotes;
    let variance = actual - est;
    Ok(Json(CostSummary {
        ticket_id: t.id,
        est_labor_cents: est_labor,
        est_parts_cents: est_parts,
        est_total_cents: est,
        est_total_label: usd(est),
        lines_cents: lines,
        expenses_cents,
        approved_quotes_cents: quotes,
        actual_total_cents: actual,
        actual_total_label: usd(actual),
        variance_cents: variance,
        variance_label: if variance < 0 {
            format!("{} under", usd(-variance))
        } else {
            format!("{} over", usd(variance))
        },
        receipts,
        tasks_total: tasks.iter().filter(|x| x.status != "skipped").count(),
        tasks_done: tasks.iter().filter(|x| x.status == "done").count(),
        trades_needed: needs,
    }))
}

// ---------------------------------------------------------------------------
// Photos, receipts, files
// ---------------------------------------------------------------------------

/// `POST /tickets/<id>/uploads` — file a photo, receipt or document on the
/// work order. Returns a signed URL to `PUT` the bytes to.
#[rocket_okapi::openapi(tag = "Service Desk")]
#[post("/tickets/<id>/uploads", data = "<body>")]
pub async fn upload(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UploadReq>,
) -> ApiResult<Json<UploadResp>> {
    user.require(Permission::MaintenanceManage)?;
    let t = ticket(&db, scope.tenant_id, id).await?;
    let b = body.into_inner();
    let filename = b.filename.trim().to_string();
    if filename.is_empty() || filename.contains('/') || filename.contains('\\') {
        return Err(ApiError::BadRequest("invalid filename".into()));
    }
    let mime = b.mime_type.trim().to_lowercase();
    if mime.is_empty() {
        return Err(ApiError::BadRequest("mime_type is required".into()));
    }
    let size = b.size_bytes.unwrap_or(0);
    if !(0..=crate::routes::documents::MAX_SIZE_BYTES).contains(&size) {
        return Err(ApiError::BadRequest("that file is too large".into()));
    }
    let kind = match b.kind.as_deref().map(str::trim) {
        Some("receipt") => "receipt",
        Some("document") => "document",
        Some("photo") => "photo",
        None if mime.starts_with("image/") => "photo",
        None => "document",
        Some(k) => {
            return Err(ApiError::BadRequest(format!(
                "kind must be photo, receipt or document, not {k}"
            )))
        }
    };
    if kind == "photo" && !mime.starts_with("image/") {
        return Err(ApiError::BadRequest("a photo has to be an image".into()));
    }
    let doc_id = Uuid::new_v4();
    let key = format!("{}/{}", scope.tenant_id, doc_id);
    let now = Utc::now();
    let saved = entity::document::ActiveModel {
        id: Set(doc_id),
        tenant_id: Set(scope.tenant_id),
        owner_type: Set("maintenance_ticket".into()),
        owner_id: Set(t.id),
        filename: Set(filename),
        category: Set(match kind {
            "document" => Some("other".into()),
            k => Some(k.into()),
        }),
        requires_wet_ink: Set(false),
        physical_location: Set(None),
        mime_type: Set(mime),
        size_bytes: Set(size),
        checksum: Set(None),
        version: Set(1),
        previous_version_id: Set(None),
        storage_key: Set(key.clone()),
        status: Set("pending_upload".into()),
        retention_expires_at: Set(None),
        created_by: Set(Some(user.user_id)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;
    let signed = ObjectStore::from_env()?.signed_put_url(&key, SIGNED_URL_TTL_SECS)?;
    record(
        &db,
        &user,
        &scope,
        crate::audit::actions::DOCUMENT_UPLOAD,
        t.id,
        json!({ "document_id": saved.id, "kind": kind }),
    )
    .await;
    Ok(Json(UploadResp {
        file: FileDto {
            id: saved.id,
            filename: saved.filename,
            mime_type: saved.mime_type,
            kind: kind.into(),
            size_bytes: saved.size_bytes,
            url: None,
            created_at: saved.created_at.to_rfc3339(),
        },
        upload_url: signed.url,
    }))
}

/// `GET /tickets/<id>/files` — photos, receipts and documents on the work
/// order, newest first, with short-lived links.
#[rocket_okapi::openapi(tag = "Service Desk")]
#[get("/tickets/<id>/files")]
pub async fn files(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Vec<FileDto>>> {
    user.require(Permission::MaintenanceRead)?;
    let t = ticket(&db, scope.tenant_id, id).await?;
    let store = ObjectStore::from_env().ok();
    let rows = Document::find()
        .filter(entity::document::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::document::Column::OwnerType.eq("maintenance_ticket"))
        .filter(entity::document::Column::OwnerId.eq(t.id))
        .filter(entity::document::Column::Status.eq("stored"))
        .order_by_desc(entity::document::Column::CreatedAt)
        .all(&db)
        .await?;
    Ok(Json(
        rows.into_iter()
            .map(|d| FileDto {
                url: store
                    .as_ref()
                    .and_then(|s| s.signed_get_url(&d.storage_key, SIGNED_URL_TTL_SECS).ok())
                    .map(|s| s.url),
                kind: kind_of(d.category.as_deref()).into(),
                id: d.id,
                filename: d.filename,
                mime_type: d.mime_type,
                size_bytes: d.size_bytes,
                created_at: d.created_at.to_rfc3339(),
            })
            .collect(),
    ))
}

// ---------------------------------------------------------------------------
// Expenses
// ---------------------------------------------------------------------------

fn expense_dto(e: entity::expense::Model) -> TicketExpenseDto {
    TicketExpenseDto {
        receipt_document_ids: serde_json::from_value(
            e.details
                .get("receipt_document_ids")
                .cloned()
                .unwrap_or(json!([])),
        )
        .unwrap_or_default(),
        amount_label: usd(e.amount_cents),
        id: e.id,
        incurred_on: e.incurred_on,
        category: e.category,
        vendor: e.vendor,
        description: e.description,
        amount_cents: e.amount_cents,
        billable_to_owner: e.billable_to_owner,
        reimbursable: e.reimbursable,
        created_at: e.created_at.to_rfc3339(),
    }
}

/// `GET /tickets/<id>/expenses` — money spent on this work order.
#[rocket_okapi::openapi(tag = "Service Desk")]
#[get("/tickets/<id>/expenses")]
pub async fn list_expenses(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Vec<TicketExpenseDto>>> {
    user.require(Permission::MaintenanceRead)?;
    let t = ticket(&db, scope.tenant_id, id).await?;
    let rows = Expense::find()
        .filter(entity::expense::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::expense::Column::MaintenanceTicketId.eq(t.id))
        .order_by_desc(entity::expense::Column::IncurredOn)
        .all(&db)
        .await?;
    Ok(Json(rows.into_iter().map(expense_dto).collect()))
}

/// `POST /tickets/<id>/expenses` — log a purchase (a run to the hardware
/// store, a vendor invoice paid on the spot) with its receipts.
#[rocket_okapi::openapi(tag = "Service Desk")]
#[post("/tickets/<id>/expenses", data = "<body>")]
pub async fn add_expense(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<TicketExpenseReq>,
) -> ApiResult<Json<TicketExpenseDto>> {
    user.require(Permission::MaintenanceManage)?;
    let t = ticket(&db, scope.tenant_id, id).await?;
    let b = body.into_inner();
    let description = b.description.trim().to_string();
    if description.is_empty() {
        return Err(ApiError::BadRequest("say what was bought".into()));
    }
    if b.amount_cents <= 0 {
        return Err(ApiError::BadRequest(
            "the amount must be more than zero".into(),
        ));
    }
    let category = b
        .category
        .map(|c| c.trim().to_lowercase())
        .filter(|c| !c.is_empty())
        .unwrap_or_else(|| "materials".into());
    if !crate::workforce::EXPENSE_CATEGORIES.contains(&category.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "category must be one of {}",
            crate::workforce::EXPENSE_CATEGORIES.join(", ")
        )));
    }
    let incurred_on = match b
        .incurred_on
        .map(|d| d.trim().to_string())
        .filter(|d| !d.is_empty())
    {
        Some(d) => {
            chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d")
                .map_err(|_| ApiError::BadRequest("incurred_on must be YYYY-MM-DD".into()))?;
            d
        }
        None => Utc::now().date_naive().to_string(),
    };
    // Receipts must be files already on this work order.
    if !b.receipt_document_ids.is_empty() {
        let found = Document::find()
            .filter(entity::document::Column::TenantId.eq(scope.tenant_id))
            .filter(entity::document::Column::OwnerType.eq("maintenance_ticket"))
            .filter(entity::document::Column::OwnerId.eq(t.id))
            .filter(entity::document::Column::Id.is_in(b.receipt_document_ids.clone()))
            .all(&db)
            .await?;
        if found.len() != b.receipt_document_ids.len() {
            return Err(ApiError::BadRequest(
                "receipts must be uploaded to this work order".into(),
            ));
        }
    }
    let now = Utc::now();
    let saved = entity::expense::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        incurred_on: Set(incurred_on),
        category: Set(category),
        vendor: Set(b
            .vendor
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())),
        description: Set(description),
        amount_cents: Set(b.amount_cents),
        miles_hundredths: Set(None),
        mileage_rate_mills: Set(None),
        tax_deductible: Set(true),
        vehicle: Set("none".into()),
        reimbursable: Set(b.reimbursable),
        reimbursed_at: Set(None),
        billable_to_owner: Set(b.billable_to_owner),
        billed_bill_id: Set(None),
        user_id: Set(Some(user.user_id)),
        maintenance_ticket_id: Set(Some(t.id)),
        rehab_project_id: Set(None),
        property_id: Set(Some(t.property_id)),
        asset_id: Set(t.asset_id),
        details: Set(json!({ "receipt_document_ids": b.receipt_document_ids })),
        recorded_by: Set(Some(user.user_id)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;
    record(
        &db,
        &user,
        &scope,
        act::TICKET_EXPENSE_ADD,
        t.id,
        json!({ "expense_id": saved.id, "amount_cents": saved.amount_cents }),
    )
    .await;
    Ok(Json(expense_dto(saved)))
}

// ---------------------------------------------------------------------------
// Vendors
// ---------------------------------------------------------------------------

/// `GET /tickets/<id>/vendors?trade=` — vendors who can take work on this
/// work order, the ones covering `trade` first.
#[rocket_okapi::openapi(tag = "Service Desk")]
#[get("/tickets/<id>/vendors?<trade>")]
pub async fn vendors(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    trade: Option<String>,
) -> ApiResult<Json<Vec<VendorOption>>> {
    user.require(Permission::MaintenanceRead)?;
    ticket(&db, scope.tenant_id, id).await?;
    let today = Utc::now().date_naive();
    let want = trade
        .map(|t| t.trim().to_lowercase())
        .filter(|t| !t.is_empty());
    let rows = Counterparty::find()
        .filter(entity::counterparty::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::counterparty::Column::Kind.eq("contractor"))
        .order_by_asc(entity::counterparty::Column::Name)
        .all(&db)
        .await?;
    let mut out = vec![];
    for c in rows {
        let trades: Vec<String> = serde_json::from_value(c.trades.clone()).unwrap_or_default();
        let matches = want.as_ref().is_some_and(|w| trades.iter().any(|t| t == w));
        out.push(VendorOption {
            coi_current: crate::vendor_compliance::coi_current(&db, scope.tenant_id, c.id, today)
                .await?,
            linked: c.partner_kind.is_some(),
            id: c.id,
            name: c.name,
            email: c.email,
            phone: c.phone,
            trades,
            matches,
        });
    }
    out.sort_by_key(|v| !v.matches);
    Ok(Json(out))
}
