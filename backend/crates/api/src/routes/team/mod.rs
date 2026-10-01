//! **The team** (Vantedge phase 2B): staff profiles, the clock, timesheets,
//! shifts and time off. Office routes live under `/team/*` (`team:read` to
//! look, `team:manage` to change; pay and bill rates only with
//! `payroll:read`). Self-service routes live under `/me/*` and need only an
//! employee profile — clocking in, fixing your own unapproved time, and asking
//! for time off, like Alpha's crew app.

pub mod clock;
pub mod employees;
pub mod schedule;
pub mod time;

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::workforce::{entry_minutes, minutes_cost, needs_review, Rules};
use chrono::Utc;
use entity::prelude::{MaintenanceTicket, Property, RehabProject, User};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

pub(crate) fn parse_id(id: &str, what: &str) -> ApiResult<Uuid> {
    Uuid::parse_str(id).map_err(|_| ApiError::BadRequest(format!("invalid {what} id")))
}

/// Whether the caller may see pay and bill rates.
pub(crate) fn sees_pay(user: &AuthUser) -> bool {
    user.require(Permission::PayrollRead).is_ok()
}

/// The caller's own profile, or 403 — self-service is for people on the team.
pub(crate) async fn my_profile(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    user: &AuthUser,
) -> ApiResult<entity::employee_profile::Model> {
    crate::workforce::profile(db, tenant_id, user.user_id)
        .await?
        .ok_or_else(|| {
            ApiError::Forbidden(
                "you're not on the team yet — ask the office to add your employee profile".into(),
            )
        })
}

#[derive(Serialize, schemars::JsonSchema, Clone)]
pub struct TimeEntryDto {
    pub id: Uuid,
    pub user_id: Uuid,
    pub user_name: String,
    /// `work_order` | `project` | `property` | `travel` | `shop` | `admin` | `other`
    pub kind: String,
    pub maintenance_ticket_id: Option<Uuid>,
    pub work_order_title: Option<String>,
    pub rehab_project_id: Option<Uuid>,
    pub project_name: Option<String>,
    pub property_id: Option<Uuid>,
    pub property_name: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub break_minutes: i32,
    pub minutes: i64,
    pub notes: Option<String>,
    /// Only with `payroll:read`.
    pub pay_rate_cents: Option<i64>,
    pub bill_rate_cents: Option<i64>,
    pub labor_cost_cents: Option<i64>,
    pub approved: bool,
    pub approved_at: Option<String>,
    pub missed_punch: bool,
    pub missed_punch_reason: Option<String>,
    /// A missed punch the office hasn't settled: held from payroll.
    pub needs_review: bool,
    pub claimed_end: Option<String>,
    pub punch_note: Option<String>,
    pub in_distance_m: Option<i32>,
    pub out_distance_m: Option<i32>,
    /// A punch was farther from the property than the radius.
    pub away: bool,
    /// Charged to the owner on an in-house bill (locked until that bill is void).
    pub billed: bool,
}

/// Turn entries into DTOs, resolving names in a few batched lookups.
pub(crate) async fn entry_dtos(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    entries: Vec<entity::time_entry::Model>,
    show_pay: bool,
    rules: &Rules,
) -> ApiResult<Vec<TimeEntryDto>> {
    let now = Utc::now();
    let ids = |f: fn(&entity::time_entry::Model) -> Option<Uuid>| -> Vec<Uuid> {
        let mut v: Vec<Uuid> = entries.iter().filter_map(f).collect();
        v.sort();
        v.dedup();
        v
    };
    let user_ids = {
        let mut v: Vec<Uuid> = entries.iter().map(|e| e.user_id).collect();
        v.sort();
        v.dedup();
        v
    };
    let users: HashMap<Uuid, String> = User::find()
        .filter(entity::user::Column::Id.is_in(user_ids))
        .all(db)
        .await?
        .into_iter()
        .map(|u| (u.id, u.name))
        .collect();
    let tickets: HashMap<Uuid, String> = MaintenanceTicket::find()
        .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
        .filter(entity::maintenance_ticket::Column::Id.is_in(ids(|e| e.maintenance_ticket_id)))
        .all(db)
        .await?
        .into_iter()
        .map(|t| (t.id, t.title))
        .collect();
    let projects: HashMap<Uuid, String> = RehabProject::find()
        .filter(entity::rehab_project::Column::TenantId.eq(tenant_id))
        .filter(entity::rehab_project::Column::Id.is_in(ids(|e| e.rehab_project_id)))
        .all(db)
        .await?
        .into_iter()
        .map(|p| (p.id, p.name))
        .collect();
    let properties: HashMap<Uuid, String> = Property::find()
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .filter(entity::property::Column::Id.is_in(ids(|e| e.property_id)))
        .all(db)
        .await?
        .into_iter()
        .map(|p| (p.id, p.name))
        .collect();
    let bills = billed_live(db, tenant_id, &entries).await?;
    let radius = rules.clock_radius_m as i32;
    Ok(entries
        .into_iter()
        .map(|e| {
            let minutes = entry_minutes(&e, now);
            let rate = e.pay_rate_cents;
            TimeEntryDto {
                id: e.id,
                user_id: e.user_id,
                user_name: users.get(&e.user_id).cloned().unwrap_or_default(),
                kind: e.kind.clone(),
                maintenance_ticket_id: e.maintenance_ticket_id,
                work_order_title: e
                    .maintenance_ticket_id
                    .and_then(|t| tickets.get(&t).cloned()),
                rehab_project_id: e.rehab_project_id,
                project_name: e.rehab_project_id.and_then(|p| projects.get(&p).cloned()),
                property_id: e.property_id,
                property_name: e.property_id.and_then(|p| properties.get(&p).cloned()),
                started_at: e.started_at.to_rfc3339(),
                ended_at: e.ended_at.map(|d| d.to_rfc3339()),
                break_minutes: e.break_minutes,
                minutes,
                notes: e.notes.clone(),
                pay_rate_cents: rate.filter(|_| show_pay),
                bill_rate_cents: e.bill_rate_cents.filter(|_| show_pay),
                labor_cost_cents: rate.map(|r| minutes_cost(minutes, r)).filter(|_| show_pay),
                approved: e.approved_at.is_some(),
                approved_at: e.approved_at.map(|d| d.to_rfc3339()),
                missed_punch: e.missed_punch,
                missed_punch_reason: e.missed_punch_reason.clone(),
                needs_review: needs_review(&e),
                claimed_end: e.claimed_end.map(|d| d.to_rfc3339()),
                punch_note: e.punch_note.clone(),
                in_distance_m: e.in_distance_m,
                out_distance_m: e.out_distance_m,
                away: e.in_distance_m.is_some_and(|d| d > radius)
                    || e.out_distance_m.is_some_and(|d| d > radius),
                billed: e.billed_bill_id.is_some_and(|b| bills.contains(&b)),
            }
        })
        .collect())
}

/// Which of the entries' owner bills are still live (not void).
pub(crate) async fn billed_live(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    entries: &[entity::time_entry::Model],
) -> ApiResult<std::collections::HashSet<Uuid>> {
    let ids: Vec<Uuid> = entries.iter().filter_map(|e| e.billed_bill_id).collect();
    if ids.is_empty() {
        return Ok(Default::default());
    }
    Ok(entity::prelude::VendorBill::find()
        .filter(entity::vendor_bill::Column::TenantId.eq(tenant_id))
        .filter(entity::vendor_bill::Column::Id.is_in(ids))
        .all(db)
        .await?
        .into_iter()
        .filter(crate::workforce::costing::bill_counts)
        .map(|b| b.id)
        .collect())
}

/// Whether an entry is on a live owner bill (and so can't change).
pub(crate) async fn is_billed(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    e: &entity::time_entry::Model,
) -> ApiResult<bool> {
    Ok(!billed_live(db, tenant_id, std::slice::from_ref(e))
        .await?
        .is_empty())
}

/// Clock in / log time against work.
#[derive(Deserialize, schemars::JsonSchema)]
pub struct TargetReq {
    /// `work_order` | `project` | `property` | `travel` | `shop` | `admin` | `other`
    pub kind: String,
    pub maintenance_ticket_id: Option<Uuid>,
    pub rehab_project_id: Option<Uuid>,
    pub property_id: Option<Uuid>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct Location {
    pub lat: f64,
    pub lng: f64,
    /// How sure the phone was, in metres; fixes coarser than 5 km are ignored.
    pub accuracy_m: Option<f64>,
}

impl Location {
    pub fn fix(loc: Option<&Location>) -> Option<crate::workforce::Fix> {
        let l = loc?;
        if !(-90.0..=90.0).contains(&l.lat) || !(-180.0..=180.0).contains(&l.lng) {
            return None;
        }
        if l.accuracy_m.is_some_and(|a| a > 5_000.0) {
            return None;
        }
        Some(crate::workforce::Fix {
            lat: l.lat,
            lng: l.lng,
        })
    }
}

/// A manual entry, or an edit of one.
#[derive(Deserialize, schemars::JsonSchema)]
pub struct EntryReq {
    #[serde(flatten)]
    pub target: TargetReq,
    /// RFC 3339.
    pub started_at: String,
    /// RFC 3339; omitted = still running (office only).
    pub ended_at: Option<String>,
    #[serde(default)]
    pub break_minutes: i32,
    pub notes: Option<String>,
    /// Office only: whose time this is (defaults to the caller).
    pub user_id: Option<Uuid>,
}
