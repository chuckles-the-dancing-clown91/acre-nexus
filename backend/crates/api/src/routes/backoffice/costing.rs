//! Work-order and rehab costing, and billing in-house work to the owner.
//!
//! * `GET …/costs` — the profit panel: labor (pay + overtime share + burden),
//!   parts, mileage, expenses, billed vs unbilled, margin, the bill rate that
//!   would reach the target (`payroll:read`, since labor cost reveals pay).
//! * `GET …/bill-preview` — exactly what billing the owner would charge now:
//!   approved, unbilled hours at each person's frozen bill rate, parts and
//!   billable expenses with the markup (`team:read`).
//! * `POST …/bill-owner` — turn the preview into a draft AP bill from
//!   "In-house maintenance" on the property's LLC, so it follows the normal
//!   submit → approve (posts to the owner's books) → pay flow and lands on the
//!   owner statement; the hours and expenses are marked billed
//!   (`team:manage` + `payable:manage`). Voiding that bill frees them again.

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::payables::{LineItem, NewBill};
use crate::rbac::Permission;
use crate::routes::team::parse_id;
use crate::tenancy::TenantScope;
use crate::workforce::costing::{
    self, bill_counts, with_markup, Cost, Work, IN_HOUSE_KIND, IN_HOUSE_NAME, PARTS_LINE_PREFIX,
};
use crate::workforce::{entry_minutes, minutes_cost, needs_review, Rules};
use chrono::Utc;
use entity::prelude::{
    Counterparty, Expense, MaintenanceTicket, RehabProject, TicketLine, TimeEntry, User, VendorBill,
};
use rocket::serde::json::Json;
use rocket::{get, post};
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

/// Resolve `kind` (`work-orders` | `rehab-projects`) + id into a `Work`, its
/// property and a label, checking it belongs to the workspace.
pub(crate) async fn resolve_work(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    kind: &str,
    id: &str,
) -> ApiResult<(Work, Uuid, String)> {
    match kind {
        "work-orders" => {
            let t = MaintenanceTicket::find_by_id(parse_id(id, "work order")?)
                .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
                .one(db)
                .await?
                .ok_or_else(|| ApiError::NotFound("work order not found".into()))?;
            Ok((Work::Ticket(t.id), t.property_id, t.title))
        }
        "rehab-projects" => {
            let p = RehabProject::find_by_id(parse_id(id, "project")?)
                .filter(entity::rehab_project::Column::TenantId.eq(tenant_id))
                .one(db)
                .await?
                .ok_or_else(|| ApiError::NotFound("rehab project not found".into()))?;
            Ok((Work::Project(p.id), p.property_id, p.name))
        }
        _ => Err(ApiError::NotFound("unknown kind of work".into())),
    }
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct CostsDto {
    pub title: String,
    #[serde(flatten)]
    pub cost: Cost,
    pub target_margin_bps: i64,
    pub under_target: bool,
    pub overtime_rule: String,
    pub by_person: Vec<PersonCost>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct PersonCost {
    pub user_id: Uuid,
    pub name: String,
    pub minutes: i64,
    pub billable_cents: i64,
}

/// `GET /costs/<kind>/<id>` — `kind` is `work-orders` or `rehab-projects`.
#[rocket_okapi::openapi(tag = "Costing")]
#[get("/costs/<kind>/<id>")]
pub async fn costs(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    kind: &str,
    id: &str,
) -> ApiResult<Json<CostsDto>> {
    user.require(Permission::PayrollRead)?;
    let (work, _, title) = resolve_work(&db, scope.tenant_id, kind, id).await?;
    let rules = Rules::load(&db, scope.tenant_id).await;
    let mut all = costing::cost_work(&db, scope.tenant_id, &[work], &rules).await?;
    let cost = all.remove(&work).unwrap_or_default();
    let names: HashMap<Uuid, String> = User::find()
        .filter(entity::user::Column::Id.is_in(cost.minutes_by.keys().copied().collect::<Vec<_>>()))
        .all(&db)
        .await?
        .into_iter()
        .map(|u| (u.id, u.name))
        .collect();
    let mut by_person: Vec<PersonCost> = cost
        .minutes_by
        .iter()
        .map(|(u, m)| PersonCost {
            user_id: *u,
            name: names.get(u).cloned().unwrap_or_default(),
            minutes: *m,
            billable_cents: cost.labor_by.get(u).copied().unwrap_or(0),
        })
        .collect();
    by_person.sort_by_key(|a| std::cmp::Reverse(a.minutes));
    Ok(Json(CostsDto {
        title,
        under_target: cost.revenue_cents > 0 && cost.gross_bps < rules.target_margin_bps,
        target_margin_bps: rules.target_margin_bps,
        overtime_rule: rules.overtime.describe().to_string(),
        by_person,
        cost,
    }))
}

#[derive(Serialize, schemars::JsonSchema, Clone)]
pub struct BillLine {
    pub description: String,
    pub amount_cents: i64,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct BillPreview {
    pub title: String,
    pub property_id: Uuid,
    pub lines: Vec<BillLine>,
    pub total_cents: i64,
    pub markup_bps: i64,
    /// Time left out, and why (not approved, missed punch, still running).
    pub held_back: Vec<String>,
    /// Earlier in-house bills on this work (not void).
    pub previous_bills: Vec<PreviousBill>,
    #[serde(skip)]
    pub entry_ids: Vec<Uuid>,
    #[serde(skip)]
    pub expense_ids: Vec<Uuid>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct PreviousBill {
    pub id: Uuid,
    pub bill_number: String,
    pub status: String,
    pub amount_cents: i64,
}

fn money(c: i64) -> String {
    let sign = if c < 0 { "-" } else { "" };
    let c = c.abs();
    format!("{sign}${}.{:02}", c / 100, c % 100)
}

fn hours(m: i64) -> String {
    format!("{}:{:02}", m / 60, m % 60)
}

/// Work out what billing the owner would charge now.
pub(crate) async fn preview(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    work: Work,
    property_id: Uuid,
    title: String,
    rules: &Rules,
) -> ApiResult<BillPreview> {
    let now = Utc::now();
    let in_house = costing::in_house_vendor_ids(db, tenant_id).await?;
    let entries = TimeEntry::find()
        .filter(entity::time_entry::Column::TenantId.eq(tenant_id))
        .filter(match work {
            Work::Ticket(id) => entity::time_entry::Column::MaintenanceTicketId.eq(id),
            Work::Project(id) => entity::time_entry::Column::RehabProjectId.eq(id),
        })
        .all(db)
        .await?;
    let expenses = Expense::find()
        .filter(entity::expense::Column::TenantId.eq(tenant_id))
        .filter(entity::expense::Column::BillableToOwner.eq(true))
        .filter(match work {
            Work::Ticket(id) => entity::expense::Column::MaintenanceTicketId.eq(id),
            Work::Project(id) => entity::expense::Column::RehabProjectId.eq(id),
        })
        .all(db)
        .await?;
    // Bills already carrying things: live ones only.
    let mut bill_ids: HashSet<Uuid> = entries.iter().filter_map(|e| e.billed_bill_id).collect();
    bill_ids.extend(expenses.iter().filter_map(|x| x.billed_bill_id));
    let mut bills = VendorBill::find()
        .filter(entity::vendor_bill::Column::TenantId.eq(tenant_id))
        .filter(entity::vendor_bill::Column::Id.is_in(bill_ids.into_iter().collect::<Vec<_>>()))
        .all(db)
        .await?;
    if let Work::Ticket(t) = work {
        for b in VendorBill::find()
            .filter(entity::vendor_bill::Column::TenantId.eq(tenant_id))
            .filter(entity::vendor_bill::Column::MaintenanceTicketId.eq(t))
            .all(db)
            .await?
        {
            if !bills.iter().any(|x| x.id == b.id) {
                bills.push(b);
            }
        }
    }
    bills.retain(|b| bill_counts(b) && in_house.contains(&b.counterparty_id));
    let live: HashSet<Uuid> = bills.iter().map(|b| b.id).collect();
    let billed = |b: Option<Uuid>| b.is_some_and(|b| live.contains(&b));

    let names: HashMap<Uuid, String> = User::find()
        .filter(
            entity::user::Column::Id.is_in(entries.iter().map(|e| e.user_id).collect::<Vec<_>>()),
        )
        .all(db)
        .await?
        .into_iter()
        .map(|u| (u.id, u.name))
        .collect();
    let mut lines = Vec::new();
    let mut held_back = Vec::new();
    let mut entry_ids = Vec::new();
    // Labor: one line per person per bill rate.
    let mut labor: BTreeMap<(String, i64), (i64, Uuid)> = BTreeMap::new();
    for e in entries.iter().filter(|e| !billed(e.billed_bill_id)) {
        let who = names.get(&e.user_id).cloned().unwrap_or_default();
        let m = entry_minutes(e, now);
        if e.ended_at.is_none() {
            held_back.push(format!("{who}: still clocked in"));
            continue;
        }
        if needs_review(e) {
            held_back.push(format!("{who}: a missed punch to settle ({})", hours(m)));
            continue;
        }
        if e.approved_at.is_none() {
            held_back.push(format!("{who}: {} not approved yet", hours(m)));
            continue;
        }
        let rate = e.bill_rate_cents.unwrap_or(0);
        if rate <= 0 {
            held_back.push(format!("{who}: no bill rate set ({})", hours(m)));
            continue;
        }
        let slot = labor.entry((who, rate)).or_insert((0, e.user_id));
        slot.0 += m;
        entry_ids.push(e.id);
    }
    for ((who, rate), (m, _)) in &labor {
        let amount = minutes_cost(*m, *rate);
        if amount > 0 {
            lines.push(BillLine {
                description: format!("Labor — {who}: {} h at {}/h", hours(*m), money(*rate)),
                amount_cents: amount,
            });
        }
    }
    // Parts and other line items, once per work order.
    if let Work::Ticket(t) = work {
        let parts_billed = bills.iter().any(|b| {
            b.line_items.as_array().is_some_and(|items| {
                items.iter().any(|i| {
                    i.get("description")
                        .and_then(|d| d.as_str())
                        .is_some_and(|d| {
                            d.starts_with(PARTS_LINE_PREFIX) || d.starts_with("Line item — ")
                        })
                })
            })
        });
        if !parts_billed {
            let tl = TicketLine::find()
                .filter(entity::ticket_line::Column::TenantId.eq(tenant_id))
                .filter(entity::ticket_line::Column::TicketId.eq(t))
                .all(db)
                .await?;
            let parts: i64 = tl
                .iter()
                .filter(|l| l.kind == "part")
                .map(|l| l.total_cents)
                .sum();
            if parts > 0 {
                let label = if rules.markup_bps > 0 {
                    format!(
                        "{PARTS_LINE_PREFIX} ({} at cost + {}% markup)",
                        money(parts),
                        rules.markup_bps as f64 / 100.0
                    )
                } else {
                    format!("{PARTS_LINE_PREFIX} (at cost)")
                };
                lines.push(BillLine {
                    description: label,
                    amount_cents: with_markup(parts, rules.markup_bps),
                });
            }
            for l in tl.iter().filter(|l| l.kind != "part" && l.total_cents > 0) {
                lines.push(BillLine {
                    description: format!("Line item — {}", l.description),
                    amount_cents: l.total_cents,
                });
            }
        }
    }
    // Billable expenses, with the markup.
    let mut expense_ids = Vec::new();
    for x in expenses
        .iter()
        .filter(|x| !billed(x.billed_bill_id) && x.amount_cents > 0)
    {
        let what = x
            .vendor
            .clone()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| x.description.clone());
        lines.push(BillLine {
            description: format!("{} — {} ({})", title_case(&x.category), what, x.incurred_on),
            amount_cents: with_markup(x.amount_cents, rules.markup_bps),
        });
        expense_ids.push(x.id);
    }
    let total = lines.iter().map(|l| l.amount_cents).sum();
    Ok(BillPreview {
        title,
        property_id,
        lines,
        total_cents: total,
        markup_bps: rules.markup_bps,
        held_back,
        previous_bills: bills
            .into_iter()
            .map(|b| PreviousBill {
                id: b.id,
                bill_number: b.bill_number,
                status: b.status,
                amount_cents: b.amount_cents,
            })
            .collect(),
        entry_ids,
        expense_ids,
    })
}

fn title_case(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// `GET /costs/<kind>/<id>/bill-preview`.
#[rocket_okapi::openapi(tag = "Costing")]
#[get("/costs/<kind>/<id>/bill-preview")]
pub async fn bill_preview(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    kind: &str,
    id: &str,
) -> ApiResult<Json<BillPreview>> {
    user.require(Permission::TeamRead)?;
    let (work, property, title) = resolve_work(&db, scope.tenant_id, kind, id).await?;
    let rules = Rules::load(&db, scope.tenant_id).await;
    Ok(Json(
        preview(&db, scope.tenant_id, work, property, title, &rules).await?,
    ))
}

/// The workspace's in-house maintenance vendor, created on first use.
pub(crate) async fn in_house_vendor(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
) -> ApiResult<entity::counterparty::Model> {
    if let Some(c) = Counterparty::find()
        .filter(entity::counterparty::Column::TenantId.eq(tenant_id))
        .filter(entity::counterparty::Column::Kind.eq(IN_HOUSE_KIND))
        .one(db)
        .await?
    {
        return Ok(c);
    }
    let now = Utc::now();
    Ok(entity::counterparty::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        kind: Set(IN_HOUSE_KIND.into()),
        name: Set(IN_HOUSE_NAME.into()),
        contact_name: Set(None),
        email: Set(None),
        phone: Set(None),
        website: Set(None),
        address: Set(None),
        notes: Set(Some(
            "Your own maintenance team. Bills from here charge owners for in-house \
             labor, parts and expenses."
                .into(),
        )),
        partner_kind: Set(None),
        partner_base_url: Set(None),
        partner_web_url: Set(None),
        partner_linked_at: Set(None),
        partner_status: Set(None),
        partner_error: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?)
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct BillOwnerReq {
    pub memo: Option<String>,
    pub due_date: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct BillOwnerResp {
    pub bill_id: Uuid,
    pub bill_number: String,
    pub amount_cents: i64,
    pub status: String,
    pub lines: Vec<BillLine>,
    pub held_back: Vec<String>,
}

/// `POST /costs/<kind>/<id>/bill-owner` — bill the owner for in-house work.
#[rocket_okapi::openapi(tag = "Costing")]
#[post("/costs/<kind>/<id>/bill-owner", data = "<body>")]
pub async fn bill_owner(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    kind: &str,
    id: &str,
    body: Json<BillOwnerReq>,
) -> ApiResult<Json<BillOwnerResp>> {
    user.require(Permission::TeamManage)?;
    user.require(Permission::PayableManage)?;
    let (work, property, title) = resolve_work(&db, scope.tenant_id, kind, id).await?;
    let rules = Rules::load(&db, scope.tenant_id).await;
    let p = preview(&db, scope.tenant_id, work, property, title.clone(), &rules).await?;
    if p.lines.is_empty() {
        return Err(ApiError::Conflict(if p.held_back.is_empty() {
            "there's nothing unbilled on this work".into()
        } else {
            format!("nothing is ready to bill: {}", p.held_back.join("; "))
        }));
    }
    let vendor = in_house_vendor(&db, scope.tenant_id).await?;
    let bill = crate::payables::create_bill(
        &db,
        scope.tenant_id,
        NewBill {
            counterparty_id: vendor.id,
            entity_id: None,
            property_id: Some(property),
            maintenance_ticket_id: match work {
                Work::Ticket(t) => Some(t),
                Work::Project(_) => None,
            },
            memo: Some(
                body.memo
                    .clone()
                    .filter(|m| !m.trim().is_empty())
                    .unwrap_or_else(|| format!("In-house maintenance — {title}")),
            ),
            line_items: p
                .lines
                .iter()
                .map(|l| LineItem {
                    description: l.description.clone(),
                    amount_cents: l.amount_cents,
                })
                .collect(),
            amount_cents: None,
            due_date: body.due_date.clone(),
        },
        user.user_id,
    )
    .await?;
    for id in &p.entry_ids {
        if let Some(e) = TimeEntry::find_by_id(*id).one(&db).await? {
            let mut am: entity::time_entry::ActiveModel = e.into();
            am.billed_bill_id = Set(Some(bill.id));
            am.update(&db).await?;
        }
    }
    for id in &p.expense_ids {
        if let Some(x) = Expense::find_by_id(*id).one(&db).await? {
            let mut am: entity::expense::ActiveModel = x.into();
            am.billed_bill_id = Set(Some(bill.id));
            am.update(&db).await?;
        }
    }
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::OWNER_BILL_CREATE,
        Some("vendor_bill"),
        Some(bill.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({
            "work": format!("{kind}/{id}"),
            "entries": p.entry_ids.len(),
            "expenses": p.expense_ids.len(),
            "amount_cents": bill.amount_cents,
        })),
    )
    .await;
    Ok(Json(BillOwnerResp {
        bill_id: bill.id,
        bill_number: bill.bill_number,
        amount_cents: bill.amount_cents,
        status: bill.status,
        lines: p.lines,
        held_back: p.held_back,
    }))
}
