//! The **issue catalog**: pick a common problem, click generate, and get a
//! work order with its checklist and a shopping list built from inventory.
//! A starter set is created the first time a workspace opens the catalog and
//! stays editable.

use super::dto::TicketDto;
use super::parts::{self, PartsList};
use crate::audit::actions as act;
use crate::audit::change::{self, Ctx};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{Asset, InventoryItem, IssueTemplate, Property, Unit};
use rocket::serde::json::Json;
use rocket::{delete, get, post, put, State};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

const CATEGORIES: &[&str] = &[
    "plumbing",
    "electrical",
    "hvac",
    "appliance",
    "structural",
    "general",
];
const PRIORITIES: &[&str] = &["low", "normal", "high", "urgent"];

/// `(name, area, category, priority, minutes, checklist, parts)`.
type Starter = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    i32,
    &'static [&'static str],
    &'static [(&'static str, i32)],
);

const STARTERS: &[Starter] = &[
    (
        "Leaking faucet",
        "Kitchen / bath",
        "plumbing",
        "normal",
        45,
        &[
            "Shut off the supply",
            "Replace cartridge or washers",
            "Check for drips under load",
            "Wipe down",
        ],
        &[("Faucet cartridge", 1), ("Supply line", 2)],
    ),
    (
        "Running toilet",
        "Bathroom",
        "plumbing",
        "normal",
        30,
        &[
            "Check flapper and fill valve",
            "Replace the worn part",
            "Flush-test three times",
        ],
        &[("Toilet flapper", 1), ("Toilet fill valve", 1)],
    ),
    (
        "Clogged drain",
        "Kitchen / bath",
        "plumbing",
        "normal",
        40,
        &[
            "Clear with snake",
            "Flush with hot water",
            "Check trap for damage",
        ],
        &[("P-trap", 1)],
    ),
    (
        "No hot water",
        "Utility",
        "plumbing",
        "high",
        90,
        &[
            "Check pilot or breaker",
            "Test thermostat and element",
            "Flush the tank if sediment",
        ],
        &[("Water heater element", 1), ("Water heater thermostat", 1)],
    ),
    (
        "Outlet not working",
        "Any room",
        "electrical",
        "normal",
        30,
        &[
            "Check the GFCI and breaker",
            "Test with a meter",
            "Replace the outlet if dead",
        ],
        &[("Outlet receptacle", 1), ("Cover plate", 1)],
    ),
    (
        "Light fixture out",
        "Any room",
        "electrical",
        "low",
        20,
        &[
            "Replace the bulb",
            "Test the switch",
            "Replace the fixture if needed",
        ],
        &[("LED bulb", 2)],
    ),
    (
        "Smoke or CO detector chirping",
        "Hallway / bedroom",
        "electrical",
        "high",
        15,
        &[
            "Replace the battery",
            "Test the alarm",
            "Replace the unit if past ten years",
        ],
        &[("9V battery", 2), ("Smoke detector", 1)],
    ),
    (
        "AC not cooling",
        "HVAC",
        "hvac",
        "high",
        90,
        &[
            "Check the filter",
            "Check the thermostat and breaker",
            "Inspect the condensate line",
            "Check refrigerant if the coil is cold",
        ],
        &[("HVAC filter", 1), ("Capacitor", 1)],
    ),
    (
        "No heat",
        "HVAC",
        "hvac",
        "urgent",
        90,
        &[
            "Check the thermostat and power",
            "Check the pilot or igniter",
            "Replace the filter",
        ],
        &[("HVAC filter", 1), ("Igniter", 1)],
    ),
    (
        "Dishwasher not draining",
        "Kitchen",
        "appliance",
        "normal",
        45,
        &["Clean the filter", "Check the drain hose", "Test the pump"],
        &[("Dishwasher drain pump", 1)],
    ),
    (
        "Refrigerator not cold",
        "Kitchen",
        "appliance",
        "high",
        60,
        &[
            "Check the temperature settings",
            "Clean the coils",
            "Test the fan and thermostat",
        ],
        &[("Refrigerator thermostat", 1)],
    ),
    (
        "Door will not lock",
        "Entry",
        "general",
        "high",
        30,
        &[
            "Check the strike plate and alignment",
            "Replace the latch or cylinder",
            "Test with all keys",
        ],
        &[("Door lockset", 1)],
    ),
    (
        "Drywall hole or damage",
        "Any room",
        "structural",
        "low",
        60,
        &["Cut and patch", "Tape, mud and sand", "Prime and paint"],
        &[("Drywall patch", 1), ("Joint compound", 1), ("Primer", 1)],
    ),
    (
        "Pest sighting",
        "Any room",
        "general",
        "normal",
        30,
        &[
            "Identify the pest",
            "Treat and seal entry points",
            "Schedule a follow-up",
        ],
        &[("Pest bait stations", 2)],
    ),
];

// ---------------------------------------------------------------------------
// Shapes
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, schemars::JsonSchema)]
pub struct IssuePart {
    pub name: String,
    pub quantity: i32,
    pub inventory_item_id: Option<Uuid>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct IssueDto {
    pub id: Uuid,
    pub name: String,
    pub area: Option<String>,
    pub category: String,
    pub priority: String,
    pub description: Option<String>,
    pub est_minutes: Option<i32>,
    pub checklist: Vec<String>,
    pub parts: Vec<IssuePart>,
    pub active: bool,
}

impl From<entity::issue_template::Model> for IssueDto {
    fn from(m: entity::issue_template::Model) -> Self {
        IssueDto {
            id: m.id,
            name: m.name,
            area: m.area,
            category: m.category,
            priority: m.priority,
            description: m.description,
            est_minutes: m.est_minutes,
            checklist: serde_json::from_value(m.checklist).unwrap_or_default(),
            parts: serde_json::from_value(m.parts).unwrap_or_default(),
            active: m.active,
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct IssueReq {
    pub name: String,
    pub area: Option<String>,
    pub category: Option<String>,
    pub priority: Option<String>,
    pub description: Option<String>,
    pub est_minutes: Option<i32>,
    pub checklist: Option<Vec<String>>,
    pub parts: Option<Vec<IssuePart>>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct GenerateReq {
    pub property_id: Uuid,
    pub unit_id: Option<Uuid>,
    pub asset_id: Option<Uuid>,
    /// What the employee saw, added to the ticket.
    pub note: Option<String>,
    pub priority: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct Generated {
    pub ticket: TicketDto,
    /// The shopping list: from stock, to buy, maybe.
    pub parts: PartsList,
}

fn clean(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn check(b: &IssueReq) -> Result<(String, String), ApiError> {
    if b.name.trim().is_empty() {
        return Err(ApiError::BadRequest("name is required".into()));
    }
    let category = b.category.clone().unwrap_or_else(|| "general".into());
    if !CATEGORIES.contains(&category.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "invalid category (expected one of {})",
            CATEGORIES.join(", ")
        )));
    }
    let priority = b.priority.clone().unwrap_or_else(|| "normal".into());
    if !PRIORITIES.contains(&priority.as_str()) {
        return Err(ApiError::BadRequest("invalid priority".into()));
    }
    Ok((category, priority))
}

fn parts_json(parts: Option<Vec<IssuePart>>) -> serde_json::Value {
    let p: Vec<IssuePart> = parts
        .unwrap_or_default()
        .into_iter()
        .filter(|p| !p.name.trim().is_empty())
        .map(|p| IssuePart {
            name: p.name.trim().to_string(),
            quantity: p.quantity.max(1),
            inventory_item_id: p.inventory_item_id,
        })
        .collect();
    json!(p)
}

async fn ensure_starters(db: &impl ConnectionTrait, tenant_id: Uuid) -> ApiResult<()> {
    let have = IssueTemplate::find()
        .filter(entity::issue_template::Column::TenantId.eq(tenant_id))
        .count(db)
        .await?;
    if have > 0 {
        return Ok(());
    }
    let now = Utc::now();
    for (name, area, category, priority, minutes, checklist, parts) in STARTERS {
        let parts: Vec<IssuePart> = parts
            .iter()
            .map(|(n, q)| IssuePart {
                name: n.to_string(),
                quantity: *q,
                inventory_item_id: None,
            })
            .collect();
        entity::issue_template::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            name: Set(name.to_string()),
            area: Set(Some(area.to_string())),
            category: Set(category.to_string()),
            priority: Set(priority.to_string()),
            description: Set(None),
            est_minutes: Set(Some(*minutes)),
            checklist: Set(json!(checklist)),
            parts: Set(json!(parts)),
            active: Set(true),
            seeded: Set(true),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

/// `GET /issue-templates` — the catalog (the starter set appears on first use).
#[rocket_okapi::openapi(tag = "Maintenance")]
#[get("/issue-templates")]
pub async fn list_issues(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<IssueDto>>> {
    user.require(Permission::MaintenanceRead)?;
    ensure_starters(&db, scope.tenant_id).await?;
    let rows = IssueTemplate::find()
        .filter(entity::issue_template::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::issue_template::Column::Active.eq(true))
        .order_by_asc(entity::issue_template::Column::Category)
        .order_by_asc(entity::issue_template::Column::Name)
        .all(&db)
        .await?;
    Ok(Json(rows.into_iter().map(IssueDto::from).collect()))
}

/// `POST /issue-templates` — add an issue to the catalog.
#[rocket_okapi::openapi(tag = "Maintenance")]
#[post("/issue-templates", data = "<body>")]
pub async fn create_issue(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<IssueReq>,
) -> ApiResult<Json<IssueDto>> {
    user.require(Permission::MaintenanceManage)?;
    let b = body.into_inner();
    let (category, priority) = check(&b)?;
    let now = Utc::now();
    let saved = entity::issue_template::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        name: Set(b.name.trim().to_string()),
        area: Set(clean(b.area)),
        category: Set(category),
        priority: Set(priority),
        description: Set(clean(b.description)),
        est_minutes: Set(b.est_minutes.filter(|m| *m > 0)),
        checklist: Set(json!(b
            .checklist
            .unwrap_or_default()
            .into_iter()
            .map(|c| c.trim().to_string())
            .filter(|c| !c.is_empty())
            .collect::<Vec<_>>())),
        parts: Set(parts_json(b.parts)),
        active: Set(true),
        seeded: Set(false),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;
    change::created(
        &db,
        Ctx::new(&user, &scope),
        act::ISSUE_TEMPLATE_SAVE,
        "issue_template",
        saved.id,
        None,
        &format!("Issue: {}", saved.name),
    )
    .await;
    Ok(Json(IssueDto::from(saved)))
}

/// `PUT /issue-templates/<id>` — edit an issue.
#[rocket_okapi::openapi(tag = "Maintenance")]
#[put("/issue-templates/<id>", data = "<body>")]
pub async fn update_issue(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<IssueReq>,
) -> ApiResult<Json<IssueDto>> {
    user.require(Permission::MaintenanceManage)?;
    let existing = find(&db, scope.tenant_id, id).await?;
    let b = body.into_inner();
    let (category, priority) = check(&b)?;
    let before = existing.clone();
    let mut am: entity::issue_template::ActiveModel = existing.into();
    am.name = Set(b.name.trim().to_string());
    am.area = Set(clean(b.area));
    am.category = Set(category);
    am.priority = Set(priority);
    am.description = Set(clean(b.description));
    am.est_minutes = Set(b.est_minutes.filter(|m| *m > 0));
    am.checklist = Set(json!(b
        .checklist
        .unwrap_or_default()
        .into_iter()
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty())
        .collect::<Vec<_>>()));
    am.parts = Set(parts_json(b.parts));
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&db).await?;
    change::change(
        &db,
        Ctx::new(&user, &scope),
        act::ISSUE_TEMPLATE_SAVE,
        "issue_template",
        saved.id,
        None,
        &format!("Issue: {}", saved.name),
        &before,
        &saved,
    )
    .await;
    Ok(Json(IssueDto::from(saved)))
}

/// `DELETE /issue-templates/<id>` — retire an issue (kept for history).
#[rocket_okapi::openapi(tag = "Maintenance")]
#[delete("/issue-templates/<id>")]
pub async fn retire_issue(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::MaintenanceManage)?;
    let existing = find(&db, scope.tenant_id, id).await?;
    let name = existing.name.clone();
    let tid = existing.id;
    let mut am: entity::issue_template::ActiveModel = existing.into();
    am.active = Set(false);
    am.updated_at = Set(Utc::now().into());
    am.update(&db).await?;
    change::removed(
        &db,
        Ctx::new(&user, &scope),
        act::ISSUE_TEMPLATE_SAVE,
        "issue_template",
        tid,
        None,
        &format!("Issue: {name}"),
        None,
    )
    .await;
    Ok(Json(json!({ "ok": true })))
}

async fn find(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    id: &str,
) -> ApiResult<entity::issue_template::Model> {
    let id = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    IssueTemplate::find_by_id(id)
        .filter(entity::issue_template::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("issue not found".into()))
}

/// `POST /issue-templates/<id>/generate` — open the work order for this issue
/// on a property (and unit / appliance), list its parts, and build the
/// shopping list from inventory.
#[rocket_okapi::openapi(tag = "Maintenance")]
#[post("/issue-templates/<id>/generate", data = "<body>")]
pub async fn generate_ticket(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<GenerateReq>,
) -> ApiResult<Json<Generated>> {
    user.require(Permission::MaintenanceManage)?;
    let issue = find(&db, scope.tenant_id, id).await?;
    if !issue.active {
        return Err(ApiError::Conflict("this issue has been retired".into()));
    }
    let b = body.into_inner();
    let property = Property::find_by_id(b.property_id)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    let unit = match b.unit_id {
        Some(u) => Some(
            Unit::find_by_id(u)
                .filter(entity::unit::Column::TenantId.eq(scope.tenant_id))
                .filter(entity::unit::Column::PropertyId.eq(property.id))
                .one(&db)
                .await?
                .ok_or_else(|| ApiError::NotFound("unit not found on this property".into()))?,
        ),
        None => None,
    };
    if let Some(a) = b.asset_id {
        Asset::find_by_id(a)
            .filter(entity::asset::Column::TenantId.eq(scope.tenant_id))
            .filter(entity::asset::Column::PropertyId.eq(property.id))
            .one(&db)
            .await?
            .ok_or_else(|| ApiError::NotFound("appliance not found on this property".into()))?;
    }
    let priority = clean(b.priority)
        .filter(|p| PRIORITIES.contains(&p.as_str()))
        .unwrap_or_else(|| issue.priority.clone());

    let checklist: Vec<String> =
        serde_json::from_value(issue.checklist.clone()).unwrap_or_default();
    let mut description = issue.description.clone().unwrap_or_default();
    if let Some(n) = clean(b.note) {
        if !description.is_empty() {
            description.push_str("\n\n");
        }
        description.push_str(&format!("Reported: {n}"));
    }
    if !checklist.is_empty() {
        if !description.is_empty() {
            description.push_str("\n\n");
        }
        description.push_str("Checklist:\n");
        for c in &checklist {
            description.push_str(&format!("- {c}\n"));
        }
    }
    let title = match &unit {
        Some(u) => format!("{} — Unit {}", issue.name, u.unit_number),
        None => issue.name.clone(),
    };
    let mut ticket = crate::helpdesk::open_ticket(
        &db,
        scope.tenant_id,
        crate::helpdesk::OpenTicket {
            property_id: property.id,
            unit_id: unit.as_ref().map(|u| u.id),
            lease_id: None,
            title,
            description: if description.is_empty() {
                None
            } else {
                Some(description.trim_end().to_string())
            },
            category: issue.category.clone(),
            priority,
            reporter: Some("Issue catalog".into()),
            due_date: None,
        },
        Some(user.user_id),
    )
    .await?;
    if let Some(a) = b.asset_id {
        let mut am: entity::maintenance_ticket::ActiveModel = ticket.into();
        am.asset_id = Set(Some(a));
        ticket = am.update(&db).await?;
    }

    // The usual parts, tied to real stock where the name (or id) matches.
    let stock = InventoryItem::find()
        .filter(entity::inventory_item::Column::TenantId.eq(scope.tenant_id))
        .all(&db)
        .await?;
    let wanted: Vec<IssuePart> = serde_json::from_value(issue.parts.clone()).unwrap_or_default();
    for p in wanted {
        let item = p
            .inventory_item_id
            .and_then(|i| stock.iter().find(|s| s.id == i))
            .or_else(|| {
                stock
                    .iter()
                    .find(|s| s.name.trim().eq_ignore_ascii_case(p.name.trim()))
            });
        parts::add_part(
            &db,
            scope.tenant_id,
            ticket.id,
            item.map(|i| i.id),
            item.map(|i| i.name.as_str()).unwrap_or(&p.name),
            p.quantity,
            "needed",
            "plan",
            None,
            Some(user.user_id),
        )
        .await?;
    }
    let list = parts::generate(&db, scope.tenant_id, &ticket).await?;

    change::created(
        &db,
        Ctx::new(&user, &scope),
        act::ISSUE_GENERATE,
        "maintenance_ticket",
        ticket.id,
        Some(property.id),
        &format!("{} (from the issue catalog)", ticket.title),
    )
    .await;
    Ok(Json(Generated {
        ticket: TicketDto::from(ticket),
        parts: list,
    }))
}
