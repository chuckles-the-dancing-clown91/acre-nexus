//! **Needs attention**, everywhere: the one list of what the portfolio owes
//! someone, and the maintenance page's **To schedule** list.
//!
//! `GET /attention` rolls up, across the properties in reach: the profile
//! suggestions (permits, insurance, appliances, crime), routines coming due
//! without a work order, work orders with no date and no visit, vendors with
//! paperwork problems, and owner approvals waiting. `GET /to-schedule` is the
//! maintenance slice of the same. `POST /maintenance-plans/<id>/run-now`
//! opens the routine's work order early. `GET /mandates?property_id=` and
//! `POST /properties/<id>/mandates` are the code-required items.

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::mandates::{self, MandateStatus};
use crate::rbac::Permission;
use crate::routes::maintenance::dto::{MaintenancePlanDto, TicketDto};
use crate::routes::property_records::attention::{suggest, Facts, Suggestion};
use crate::state::AppState;
use crate::tenancy::{Access, TenantScope};
use chrono::{Duration, NaiveDate, Utc};
use entity::prelude::{
    ActionItem, Appointment, Asset, Counterparty, InsurancePolicy, MaintenancePlan,
    MaintenanceTicket, OwnerApproval, Property, PropertyCrime, PropertyDetail, PropertyPermit,
    PropertySchool,
};
use rocket::serde::json::Json;
use rocket::{get, post, State};
use schemars::JsonSchema;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// One thing to do, wherever it comes from.
#[derive(Serialize, JsonSchema, Debug, Clone)]
pub struct AttentionItem {
    /// `profile` | `routine` | `unscheduled` | `vendor` | `approval`
    pub kind: String,
    /// Stable within its kind.
    pub key: String,
    pub title: String,
    pub detail: String,
    pub property_id: Option<Uuid>,
    pub property_name: Option<String>,
    /// Where to go: a console path.
    pub href: String,
    pub due_on: Option<String>,
    /// high | normal | low
    pub priority: String,
}

#[derive(Serialize, JsonSchema)]
pub struct AttentionResp {
    pub items: Vec<AttentionItem>,
    pub counts: HashMap<String, usize>,
}

/// A routine coming due, with where it stands.
#[derive(Serialize, JsonSchema)]
pub struct DuePlan {
    pub plan: MaintenancePlanDto,
    pub property_name: String,
    /// Days until due; negative when overdue.
    pub days: i64,
    pub mandate: bool,
}

#[derive(Serialize, JsonSchema)]
pub struct ToScheduleResp {
    /// Routines due within their lead time with no open work order.
    pub plans: Vec<DuePlan>,
    /// Open work orders with no due date and no visit booked.
    pub tickets: Vec<TicketDto>,
    pub lead_days: i32,
}

fn rank(p: &str) -> u8 {
    match p {
        "urgent" | "high" => 0,
        "normal" | "medium" => 1,
        _ => 2,
    }
}

async fn properties_in(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    access: &Access,
) -> ApiResult<Vec<entity::property::Model>> {
    let mut q = Property::find().filter(entity::property::Column::TenantId.eq(tenant_id));
    if let Some(ids) = access.property_ids() {
        q = q.filter(entity::property::Column::Id.is_in(ids));
    }
    Ok(q.order_by_asc(entity::property::Column::Name)
        .all(db)
        .await?)
}

/// Routines due within `lead_days` (the plan's own, else the workspace's)
/// whose last work order isn't still open.
async fn due_plans(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    props: &[entity::property::Model],
    today: NaiveDate,
    lead_days: i32,
) -> ApiResult<Vec<DuePlan>> {
    let ids: Vec<Uuid> = props.iter().map(|p| p.id).collect();
    let plans = MaintenancePlan::find()
        .filter(entity::maintenance_plan::Column::TenantId.eq(tenant_id))
        .filter(entity::maintenance_plan::Column::PropertyId.is_in(ids))
        .filter(entity::maintenance_plan::Column::Active.eq(true))
        .order_by_asc(entity::maintenance_plan::Column::NextDueDate)
        .all(db)
        .await?;
    let last_ids: Vec<Uuid> = plans.iter().filter_map(|p| p.last_ticket_id).collect();
    let still_open: HashSet<Uuid> = if last_ids.is_empty() {
        HashSet::new()
    } else {
        MaintenanceTicket::find()
            .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
            .filter(entity::maintenance_ticket::Column::Id.is_in(last_ids))
            .filter(
                entity::maintenance_ticket::Column::Status
                    .is_in(crate::routes::maintenance::OPEN_STATUSES.to_vec()),
            )
            .all(db)
            .await?
            .into_iter()
            .map(|t| t.id)
            .collect()
    };
    let mut out = Vec::new();
    for plan in plans {
        let due = NaiveDate::parse_from_str(&plan.next_due_date, "%Y-%m-%d").unwrap_or(today);
        let lead = plan.lead_days.unwrap_or(lead_days).max(0);
        if due > today + Duration::days(lead as i64) {
            continue;
        }
        if plan.last_ticket_id.is_some_and(|t| still_open.contains(&t)) {
            continue;
        }
        let property_name = props
            .iter()
            .find(|p| p.id == plan.property_id)
            .map(|p| p.name.clone())
            .unwrap_or_default();
        out.push(DuePlan {
            days: (due - today).num_days(),
            property_name,
            mandate: plan.mandate_key.is_some(),
            plan: MaintenancePlanDto::from(plan),
        });
    }
    Ok(out)
}

/// Open work orders (open or triage) with no due date and no visit that
/// isn't cancelled or declined.
async fn unscheduled(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    props: &[entity::property::Model],
) -> ApiResult<Vec<TicketDto>> {
    let ids: Vec<Uuid> = props.iter().map(|p| p.id).collect();
    let rows = MaintenanceTicket::find()
        .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
        .filter(entity::maintenance_ticket::Column::PropertyId.is_in(ids))
        .filter(entity::maintenance_ticket::Column::Status.is_in(["open", "triage"]))
        .filter(entity::maintenance_ticket::Column::DueDate.is_null())
        .order_by_asc(entity::maintenance_ticket::Column::CreatedAt)
        .all(db)
        .await?;
    if rows.is_empty() {
        return Ok(vec![]);
    }
    let tids: Vec<Uuid> = rows.iter().map(|t| t.id).collect();
    let booked: HashSet<Uuid> = Appointment::find()
        .filter(entity::appointment::Column::TenantId.eq(tenant_id))
        .filter(entity::appointment::Column::SubjectType.eq("ticket"))
        .filter(entity::appointment::Column::SubjectId.is_in(tids))
        .filter(entity::appointment::Column::Status.is_not_in(["cancelled", "declined"]))
        .all(db)
        .await?
        .into_iter()
        .filter_map(|a| a.subject_id)
        .collect();
    let rows: Vec<_> = rows
        .into_iter()
        .filter(|t| !booked.contains(&t.id))
        .collect();
    let mut dtos = crate::routes::maintenance::queue::decorate(db, tenant_id, rows).await?;
    dtos.sort_by_key(|t| rank(&t.priority));
    Ok(dtos)
}

/// `GET /to-schedule` — the maintenance page's list of what needs a date.
#[rocket_okapi::openapi(tag = "Maintenance")]
#[get("/to-schedule")]
pub async fn to_schedule(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
) -> ApiResult<Json<ToScheduleResp>> {
    user.require(Permission::MaintenanceRead)?;
    let t = scope.tenant_id;
    let lead_days =
        crate::settings::get_i64(&db, t, crate::settings::HELPDESK_PLAN_LEAD_DAYS).await as i32;
    let today = Utc::now().date_naive();
    let props = properties_in(&db, t, &access).await?;
    Ok(Json(ToScheduleResp {
        plans: due_plans(&db, t, &props, today, lead_days).await?,
        tickets: unscheduled(&db, t, &props).await?,
        lead_days,
    }))
}

/// `POST /maintenance-plans/<id>/run-now` — open the routine's work order
/// now instead of on its due date; the plan's next date moves on a cadence
/// from today.
#[rocket_okapi::openapi(tag = "Maintenance")]
#[post("/maintenance-plans/<id>/run-now")]
pub async fn run_plan_now(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    id: &str,
) -> ApiResult<Json<TicketDto>> {
    user.require(Permission::MaintenanceManage)?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::NotFound("plan not found".into()))?;
    let plan = MaintenancePlan::find_by_id(pid)
        .filter(entity::maintenance_plan::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .filter(|p| access.sees(p.property_id))
        .ok_or_else(|| ApiError::NotFound("plan not found".into()))?;
    if let Some(last) = plan.last_ticket_id {
        let open = MaintenanceTicket::find_by_id(last)
            .filter(entity::maintenance_ticket::Column::TenantId.eq(scope.tenant_id))
            .one(&db)
            .await?
            .is_some_and(|t| crate::routes::maintenance::is_open(&t.status));
        if open {
            return Err(ApiError::Conflict(
                "this routine already has an open work order".into(),
            ));
        }
    }
    let today = Utc::now().date_naive();
    let ticket =
        crate::helpdesk::run_plan(&db, scope.tenant_id, plan, today, Some(user.user_id)).await?;
    let mut dtos =
        crate::routes::maintenance::queue::decorate(&db, scope.tenant_id, vec![ticket]).await?;
    Ok(Json(dtos.remove(0)))
}

/// `GET /mandates?property_id=` — code-required items for a property, with
/// the routine each has.
#[rocket_okapi::openapi(tag = "Maintenance")]
#[get("/mandates?<property_id>")]
pub async fn list_mandates(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    property_id: &str,
) -> ApiResult<Json<Vec<MandateStatus>>> {
    user.require(Permission::MaintenanceRead)?;
    let p = crate::routes::property_records::property_in(&db, scope.tenant_id, property_id).await?;
    if !access.sees(p.id) {
        return Err(ApiError::NotFound("property not found".into()));
    }
    Ok(Json(mandates::status(&db, scope.tenant_id, &p).await?))
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct ApplyMandatesReq {
    /// Item keys to add; empty adds every item that applies, the conditional
    /// ones (pool, septic, boiler) aside.
    #[serde(default)]
    pub keys: Vec<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct ApplyMandatesResp {
    pub created: usize,
    pub items: Vec<MandateStatus>,
}

/// `POST /properties/<id>/mandates` — add routines for the code-required
/// items the property is missing.
#[rocket_okapi::openapi(tag = "Maintenance")]
#[post("/properties/<id>/mandates", data = "<body>")]
pub async fn apply_mandates(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    id: &str,
    body: Option<Json<ApplyMandatesReq>>,
) -> ApiResult<Json<ApplyMandatesResp>> {
    user.require(Permission::MaintenanceManage)?;
    let p = crate::routes::property_records::property_in(&db, scope.tenant_id, id).await?;
    if !access.sees(p.id) {
        return Err(ApiError::NotFound("property not found".into()));
    }
    let keys = body.map(|b| b.into_inner().keys).unwrap_or_default();
    for k in &keys {
        if mandates::by_key(k).is_none() {
            return Err(ApiError::BadRequest(format!("unknown item: {k}")));
        }
    }
    let today = Utc::now().date_naive();
    let created =
        mandates::apply(&db, scope.tenant_id, &p, &keys, today, Some(user.user_id)).await?;
    Ok(Json(ApplyMandatesResp {
        created,
        items: mandates::status(&db, scope.tenant_id, &p).await?,
    }))
}

/// `GET /attention` — everything that needs someone, across the portfolio.
#[rocket_okapi::openapi(tag = "Property Profile")]
#[get("/attention")]
pub async fn portfolio(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
) -> ApiResult<Json<AttentionResp>> {
    user.require(Permission::PropertyRead)?;
    let t = scope.tenant_id;
    let today = Utc::now().date_naive();
    let props = properties_in(&db, t, &access).await?;
    let ids: Vec<Uuid> = props.iter().map(|p| p.id).collect();
    let name_of: HashMap<Uuid, String> = props.iter().map(|p| (p.id, p.name.clone())).collect();
    let mut items: Vec<AttentionItem> = Vec::new();

    // Profile suggestions, per property, less the ones already on a list.
    let details = PropertyDetail::find()
        .filter(entity::property_detail::Column::TenantId.eq(t))
        .filter(entity::property_detail::Column::PropertyId.is_in(ids.clone()))
        .all(&db)
        .await?;
    let permits = PropertyPermit::find()
        .filter(entity::property_permit::Column::TenantId.eq(t))
        .filter(entity::property_permit::Column::PropertyId.is_in(ids.clone()))
        .all(&db)
        .await?;
    let policies = InsurancePolicy::find()
        .filter(entity::insurance_policy::Column::TenantId.eq(t))
        .filter(entity::insurance_policy::Column::PropertyId.is_in(ids.clone()))
        .all(&db)
        .await?;
    let assets = Asset::find()
        .filter(entity::asset::Column::TenantId.eq(t))
        .filter(entity::asset::Column::PropertyId.is_in(ids.clone()))
        .all(&db)
        .await?;
    let schools = PropertySchool::find()
        .filter(entity::property_school::Column::TenantId.eq(t))
        .filter(entity::property_school::Column::PropertyId.is_in(ids.clone()))
        .all(&db)
        .await?;
    let crimes = PropertyCrime::find()
        .filter(entity::property_crime::Column::TenantId.eq(t))
        .filter(entity::property_crime::Column::PropertyId.is_in(ids.clone()))
        .all(&db)
        .await?;
    let taken: HashSet<(Uuid, String)> = ActionItem::find()
        .filter(entity::action_item::Column::TenantId.eq(t))
        .filter(entity::action_item::Column::PropertyId.is_in(ids.clone()))
        .filter(entity::action_item::Column::SuggestionKey.is_not_null())
        .all(&db)
        .await?
        .into_iter()
        .filter_map(|a| a.suggestion_key.map(|k| (a.property_id, k)))
        .collect();
    for p in &props {
        let my_permits: Vec<_> = permits
            .iter()
            .filter(|x| x.property_id == p.id)
            .cloned()
            .collect();
        let my_policies: Vec<_> = policies
            .iter()
            .filter(|x| x.property_id == p.id)
            .cloned()
            .collect();
        let my_assets: Vec<_> = assets
            .iter()
            .filter(|x| x.property_id == p.id)
            .cloned()
            .collect();
        let my_schools: Vec<_> = schools
            .iter()
            .filter(|x| x.property_id == p.id)
            .cloned()
            .collect();
        let sugg: Vec<Suggestion> = suggest(&Facts {
            today,
            detail: details.iter().find(|d| d.property_id == p.id),
            permits: &my_permits,
            policies: &my_policies,
            assets: &my_assets,
            schools: &my_schools,
            crime: crimes.iter().find(|c| c.property_id == p.id),
        });
        for s in sugg {
            if taken.contains(&(p.id, s.key.clone())) {
                continue;
            }
            items.push(AttentionItem {
                kind: "profile".into(),
                key: s.key,
                title: s.title,
                detail: s.detail,
                property_id: Some(p.id),
                property_name: Some(p.name.clone()),
                href: format!("/console/properties/{}", p.id),
                due_on: s.due_on,
                priority: s.priority,
            });
        }
    }

    // Routines coming due and work orders with no date.
    let lead_days =
        crate::settings::get_i64(&db, t, crate::settings::HELPDESK_PLAN_LEAD_DAYS).await as i32;
    for d in due_plans(&db, t, &props, today, lead_days).await? {
        let when = if d.days < 0 {
            format!("{} days overdue", -d.days)
        } else if d.days == 0 {
            "due today".into()
        } else {
            format!("due in {} days", d.days)
        };
        items.push(AttentionItem {
            kind: "routine".into(),
            key: d.plan.id.to_string(),
            title: d.plan.title.clone(),
            detail: if d.mandate {
                format!("Required by code; {when}. Schedule it.")
            } else {
                format!("Routine {when}. Schedule it.")
            },
            property_id: Some(d.plan.property_id),
            property_name: Some(d.property_name),
            href: "/console/maintenance/schedule".into(),
            due_on: Some(d.plan.next_due_date.clone()),
            priority: if d.days < 0 {
                "high".into()
            } else {
                d.plan.priority.clone()
            },
        });
    }
    for tk in unscheduled(&db, t, &props).await? {
        items.push(AttentionItem {
            kind: "unscheduled".into(),
            key: tk.id.to_string(),
            title: tk.title.clone(),
            detail: "Open with no date and no visit booked.".into(),
            property_id: Some(tk.property_id),
            property_name: name_of.get(&tk.property_id).cloned(),
            href: format!("/console/maintenance/{}", tk.id),
            due_on: None,
            priority: match tk.priority.as_str() {
                "urgent" | "high" => "high".into(),
                "low" => "low".into(),
                _ => "normal".into(),
            },
        });
    }

    // Owner approvals waiting on the owner.
    let pending = OwnerApproval::find()
        .filter(entity::owner_approval::Column::TenantId.eq(t))
        .filter(entity::owner_approval::Column::Status.eq("pending"))
        .all(&db)
        .await?;
    if !pending.is_empty() {
        let tids: Vec<Uuid> = pending.iter().map(|a| a.ticket_id).collect();
        let tickets: HashMap<Uuid, entity::maintenance_ticket::Model> = MaintenanceTicket::find()
            .filter(entity::maintenance_ticket::Column::TenantId.eq(t))
            .filter(entity::maintenance_ticket::Column::Id.is_in(tids))
            .filter(entity::maintenance_ticket::Column::PropertyId.is_in(ids.clone()))
            .all(&db)
            .await?
            .into_iter()
            .map(|x| (x.id, x))
            .collect();
        for a in pending {
            let Some(tk) = tickets.get(&a.ticket_id) else {
                continue;
            };
            let what = if a.kind == "signoff" {
                "Owner sign-off waiting"
            } else {
                "Owner approval waiting"
            };
            let age = (Utc::now() - a.requested_at.with_timezone(&Utc)).num_days();
            items.push(AttentionItem {
                kind: "approval".into(),
                key: a.id.to_string(),
                title: format!("{what}: {}", tk.title),
                detail: format!(
                    "{} asked {} ago.",
                    crate::owner_approvals::money(a.amount_cents),
                    if age == 0 {
                        "today".to_string()
                    } else {
                        format!("{age} days")
                    }
                ),
                property_id: Some(tk.property_id),
                property_name: name_of.get(&tk.property_id).cloned(),
                href: format!("/console/maintenance/{}", tk.id),
                due_on: None,
                priority: if age >= 5 {
                    "high".into()
                } else {
                    "normal".into()
                },
            });
        }
    }

    // Vendors with paperwork problems: company-wide, so only for people who
    // see the whole company.
    if access.property_ids().is_none() {
        let vendors = Counterparty::find()
            .filter(entity::counterparty::Column::TenantId.eq(t))
            .filter(entity::counterparty::Column::Kind.eq("contractor"))
            .all(&db)
            .await?;
        for v in vendors {
            let c = crate::routes::vendors::compliance(&db, t, &v).await?;
            if c.problems.is_empty() {
                continue;
            }
            items.push(AttentionItem {
                kind: "vendor".into(),
                key: v.id.to_string(),
                title: v.name.clone(),
                detail: c.problems.join(" "),
                property_id: None,
                property_name: None,
                href: format!("/console/vendors/{}", v.id),
                due_on: None,
                priority: if c.coi_current {
                    "low".into()
                } else {
                    "normal".into()
                },
            });
        }
    }

    items.sort_by(|a, b| {
        rank(&a.priority)
            .cmp(&rank(&b.priority))
            .then_with(|| a.due_on.cmp(&b.due_on))
    });
    let mut counts: HashMap<String, usize> = HashMap::new();
    for i in &items {
        *counts.entry(i.kind.clone()).or_default() += 1;
    }
    Ok(Json(AttentionResp { items, counts }))
}
