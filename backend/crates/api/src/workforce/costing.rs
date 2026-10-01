//! **Costing** — what in-house work cost, what it bills the owner, and the
//! margin in between. Alpha's job costing, with the work order as the job.
//!
//! Costs of a piece of work (a work order or a rehab project) —
//!
//! * **labor**: its time entries at each person's frozen pay rate, plus its
//!   share of that person's weekly overtime premium (the whole week split by
//!   the overtime rule, the premium spread over the week's entries by minutes),
//!   plus the labor burden % for employees — never for 1099 contractors;
//! * **parts**: work-order part lines (stock pulled at cost);
//! * **other lines**: the work order's other line items (fees etc.), at face;
//! * **mileage** and **expenses** logged against it;
//! * **overhead**: optional, per labor hour, for a net figure.
//!
//! Billed to the owner — what the in-house maintenance bills on it add up to
//! (void bills excluded). Until billed, the *unbilled* amount is what billing
//! would charge now: hours × each person's frozen bill rate, parts and
//! billable expenses plus the markup %, other lines at face. Outside vendor
//! bills are the owner's cost too, shown alongside ("total to owner") but not
//! part of the firm's margin.

use super::{entry_minutes, minutes_cost, overtime::div_round, Rules};
use chrono::Utc;
use entity::prelude::{Counterparty, Expense, TicketLine, TimeEntry, VendorBill};
use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// The counterparty kind the in-house maintenance vendor is created with.
pub const IN_HOUSE_KIND: &str = "in_house";
/// Its display name.
pub const IN_HOUSE_NAME: &str = "In-house maintenance";

/// Money in cents for one piece of work.
#[derive(Debug, Clone, Default, Serialize, schemars::JsonSchema)]
pub struct Cost {
    pub minutes: i64,
    pub labor_pay_cents: i64,
    pub overtime_premium_cents: i64,
    pub burden_cents: i64,
    pub labor_cents: i64,
    pub parts_cents: i64,
    pub other_lines_cents: i64,
    pub mileage_cents: i64,
    pub expenses_cents: i64,
    pub costs_cents: i64,
    pub overhead_cents: i64,
    /// Billed to the owner on in-house bills (not void).
    pub billed_cents: i64,
    /// What billing the not-yet-billed hours, parts and expenses would add now.
    pub unbilled_cents: i64,
    /// billed + unbilled — the work's revenue.
    pub revenue_cents: i64,
    pub gross_cents: i64,
    pub gross_bps: i64,
    pub net_cents: i64,
    /// Outside vendor bills on the work (the owner's cost, not the firm's).
    pub vendor_bills_cents: i64,
    /// revenue + vendor bills — what the owner pays for the work in all.
    pub owner_total_cents: i64,
    /// The hourly bill rate that would reach the target margin (0 = no hours).
    pub bill_rate_for_target_cents: i64,
    /// Minutes by person, for splitting by technician.
    #[serde(skip)]
    pub minutes_by: HashMap<Uuid, i64>,
    /// Revenue by person's labor, for technician rollups.
    #[serde(skip)]
    pub labor_by: HashMap<Uuid, i64>,
}

impl Cost {
    fn finish(&mut self, rules: &Rules) {
        self.labor_cents = self.labor_pay_cents + self.overtime_premium_cents + self.burden_cents;
        self.costs_cents = self.labor_cents
            + self.parts_cents
            + self.other_lines_cents
            + self.mileage_cents
            + self.expenses_cents;
        self.overhead_cents = minutes_cost(self.minutes, rules.overhead_per_hour_cents);
        self.revenue_cents = self.billed_cents + self.unbilled_cents;
        self.gross_cents = self.revenue_cents - self.costs_cents;
        self.gross_bps = bps(self.gross_cents, self.revenue_cents);
        self.net_cents = self.gross_cents - self.overhead_cents;
        self.owner_total_cents = self.revenue_cents + self.vendor_bills_cents;
        self.bill_rate_for_target_cents = bill_rate_for_target(self, rules.target_margin_bps);
    }
}

/// `part / whole` in basis points (0 when there's no whole).
pub fn bps(part: i64, whole: i64) -> i64 {
    if whole == 0 {
        0
    } else {
        div_round(part * 10_000, whole)
    }
}

/// The hourly bill rate at which this work's gross margin would reach
/// `target_bps`, keeping everything else as billed. Revenue needed =
/// costs ÷ (1 − margin); the labor share of it spread over the hours.
pub fn bill_rate_for_target(c: &Cost, target_bps: i64) -> i64 {
    if c.minutes == 0 || target_bps >= 10_000 {
        return 0;
    }
    let needed = div_round(c.costs_cents * 10_000, 10_000 - target_bps);
    let non_labor_revenue = c.revenue_cents - c.labor_billed_estimate();
    let labor_needed = (needed - non_labor_revenue).max(0);
    div_round(labor_needed * 60, c.minutes)
}

impl Cost {
    /// The labor part of revenue (billed labor isn't itemized once billed, so
    /// this uses the frozen bill rates, which is what billing charged).
    fn labor_billed_estimate(&self) -> i64 {
        self.labor_by.values().sum()
    }
}

/// What costing is keyed by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Work {
    Ticket(Uuid),
    Project(Uuid),
}

fn entry_work(e: &entity::time_entry::Model) -> Option<Work> {
    e.maintenance_ticket_id
        .map(Work::Ticket)
        .or(e.rehab_project_id.map(Work::Project))
}

fn expense_work(x: &entity::expense::Model) -> Option<Work> {
    x.maintenance_ticket_id
        .map(Work::Ticket)
        .or(x.rehab_project_id.map(Work::Project))
}

/// The workspace's in-house maintenance vendor ids.
pub async fn in_house_vendor_ids(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
) -> Result<HashSet<Uuid>, DbErr> {
    Ok(Counterparty::find()
        .filter(entity::counterparty::Column::TenantId.eq(tenant_id))
        .filter(entity::counterparty::Column::Kind.eq(IN_HOUSE_KIND))
        .all(db)
        .await?
        .into_iter()
        .map(|c| c.id)
        .collect())
}

/// Bills that are live (anything but void).
pub fn bill_counts(b: &entity::vendor_bill::Model) -> bool {
    b.status != "void"
}

/// Each overtime premium share per entry id for every person-week touched by
/// `entries` — loads the whole weeks, since overtime is decided per week.
pub async fn premium_shares(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    entries: &[entity::time_entry::Model],
    profiles: &HashMap<Uuid, entity::employee_profile::Model>,
    rules: &Rules,
) -> Result<HashMap<Uuid, i64>, DbErr> {
    let mut weeks: HashSet<(Uuid, chrono::NaiveDate)> = HashSet::new();
    for e in entries.iter().filter(|e| e.ended_at.is_some()) {
        weeks.insert((
            e.user_id,
            super::overtime::week_start(rules.local_date(e.started_at)),
        ));
    }
    let mut all: Vec<entity::time_entry::Model> = Vec::new();
    let mut seen = HashSet::new();
    for (user, monday) in weeks {
        for e in super::entries_for_weeks(db, tenant_id, monday, monday, Some(user), rules).await? {
            if seen.insert(e.id) {
                all.push(e);
            }
        }
    }
    Ok(super::week_splits(&all, profiles, rules).1)
}

/// Cost a batch of work orders / rehab projects.
pub async fn cost_work(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    work: &[Work],
    rules: &Rules,
) -> Result<HashMap<Work, Cost>, DbErr> {
    let tickets: Vec<Uuid> = work
        .iter()
        .filter_map(|w| match w {
            Work::Ticket(id) => Some(*id),
            _ => None,
        })
        .collect();
    let projects: Vec<Uuid> = work
        .iter()
        .filter_map(|w| match w {
            Work::Project(id) => Some(*id),
            _ => None,
        })
        .collect();
    let mut out: HashMap<Work, Cost> = work.iter().map(|w| (*w, Cost::default())).collect();
    if work.is_empty() {
        return Ok(out);
    }
    let now = Utc::now();

    let entries: Vec<entity::time_entry::Model> = TimeEntry::find()
        .filter(entity::time_entry::Column::TenantId.eq(tenant_id))
        .filter(
            sea_orm::Condition::any()
                .add(entity::time_entry::Column::MaintenanceTicketId.is_in(tickets.clone()))
                .add(entity::time_entry::Column::RehabProjectId.is_in(projects.clone())),
        )
        .all(db)
        .await?;
    let profiles = super::profiles_by_user(db, tenant_id).await?;
    let premium = premium_shares(db, tenant_id, &entries, &profiles, rules).await?;

    let bills: Vec<entity::vendor_bill::Model> = if tickets.is_empty() {
        vec![]
    } else {
        VendorBill::find()
            .filter(entity::vendor_bill::Column::TenantId.eq(tenant_id))
            .filter(entity::vendor_bill::Column::MaintenanceTicketId.is_in(tickets.clone()))
            .all(db)
            .await?
    };
    let live_bill_ids: HashSet<Uuid> = bills
        .iter()
        .filter(|b| bill_counts(b))
        .map(|b| b.id)
        .collect();
    let in_house = in_house_vendor_ids(db, tenant_id).await?;
    // In-house bills on rehab projects are found by their billed rows below.
    let billed_elsewhere: HashSet<Uuid> = entries.iter().filter_map(|e| e.billed_bill_id).collect();
    let extra_bills: Vec<entity::vendor_bill::Model> = if billed_elsewhere.is_empty() {
        vec![]
    } else {
        VendorBill::find()
            .filter(entity::vendor_bill::Column::TenantId.eq(tenant_id))
            .filter(entity::vendor_bill::Column::Id.is_in(billed_elsewhere))
            .all(db)
            .await?
    };
    let live: HashSet<Uuid> = live_bill_ids
        .iter()
        .copied()
        .chain(extra_bills.iter().filter(|b| bill_counts(b)).map(|b| b.id))
        .collect();
    let is_billed = |bill: Option<Uuid>| bill.is_some_and(|b| live.contains(&b));

    for e in &entries {
        let Some(w) = entry_work(e) else { continue };
        let Some(c) = out.get_mut(&w) else { continue };
        let m = entry_minutes(e, now);
        let prof = profiles.get(&e.user_id);
        let pay_rate = e
            .pay_rate_cents
            .or(prof.map(|p| p.pay_rate_cents))
            .unwrap_or(0);
        let bill_rate = e
            .bill_rate_cents
            .or(prof.map(|p| p.bill_rate_cents))
            .unwrap_or(0);
        let pay = minutes_cost(m, pay_rate);
        let extra = premium.get(&e.id).copied().unwrap_or(0);
        c.minutes += m;
        c.labor_pay_cents += pay;
        c.overtime_premium_cents += extra;
        let contractor = prof.is_some_and(|p| p.employment_type == "contractor");
        if !contractor {
            c.burden_cents += div_round((pay + extra) * rules.burden_bps, 10_000);
        }
        *c.minutes_by.entry(e.user_id).or_default() += m;
        let labor_bill = minutes_cost(m, bill_rate);
        *c.labor_by.entry(e.user_id).or_default() += labor_bill;
        if !is_billed(e.billed_bill_id) {
            c.unbilled_cents += labor_bill;
        }
    }

    if !tickets.is_empty() {
        for l in TicketLine::find()
            .filter(entity::ticket_line::Column::TenantId.eq(tenant_id))
            .filter(entity::ticket_line::Column::TicketId.is_in(tickets.clone()))
            .all(db)
            .await?
        {
            if let Some(c) = out.get_mut(&Work::Ticket(l.ticket_id)) {
                if l.kind == "part" {
                    c.parts_cents += l.total_cents;
                } else {
                    c.other_lines_cents += l.total_cents;
                }
            }
        }
    }

    for x in Expense::find()
        .filter(entity::expense::Column::TenantId.eq(tenant_id))
        .filter(
            sea_orm::Condition::any()
                .add(entity::expense::Column::MaintenanceTicketId.is_in(tickets.clone()))
                .add(entity::expense::Column::RehabProjectId.is_in(projects.clone())),
        )
        .all(db)
        .await?
    {
        let Some(w) = expense_work(&x) else { continue };
        let Some(c) = out.get_mut(&w) else { continue };
        if x.category == "mileage" {
            c.mileage_cents += x.amount_cents;
        } else {
            c.expenses_cents += x.amount_cents;
        }
        if x.billable_to_owner && !is_billed(x.billed_bill_id) {
            c.unbilled_cents += with_markup(x.amount_cents, rules.markup_bps);
        }
    }

    // A bill can be reached both from its work order and from billed rows.
    let mut seen_bills = HashSet::new();
    for b in bills
        .iter()
        .chain(extra_bills.iter())
        .filter(|b| bill_counts(b) && seen_bills.insert(b.id))
    {
        let key = match b.maintenance_ticket_id {
            Some(t) => Work::Ticket(t),
            None => match entries
                .iter()
                .find(|e| e.billed_bill_id == Some(b.id))
                .and_then(entry_work)
            {
                Some(w) => w,
                None => continue,
            },
        };
        let Some(c) = out.get_mut(&key) else { continue };
        if in_house.contains(&b.counterparty_id) {
            c.billed_cents += b.amount_cents;
        } else {
            c.vendor_bills_cents += b.amount_cents;
        }
    }

    // Parts and other lines are billed once, with the first in-house bill.
    for (w, c) in out.iter_mut() {
        if let Work::Ticket(t) = w {
            let parts_billed = bills.iter().any(|b| {
                bill_counts(b)
                    && in_house.contains(&b.counterparty_id)
                    && b.maintenance_ticket_id == Some(*t)
                    && bill_has_parts(b)
            });
            if !parts_billed {
                c.unbilled_cents +=
                    with_markup(c.parts_cents, rules.markup_bps) + c.other_lines_cents;
            }
        }
        c.finish(rules);
    }
    Ok(out)
}

/// Parts plus the markup.
pub fn with_markup(cents: i64, markup_bps: i64) -> i64 {
    cents + div_round(cents * markup_bps, 10_000)
}

/// Whether an in-house bill carried the work order's parts / line items.
fn bill_has_parts(b: &entity::vendor_bill::Model) -> bool {
    b.line_items
        .as_array()
        .map(|items| {
            items.iter().any(|i| {
                i.get("description")
                    .and_then(|d| d.as_str())
                    .is_some_and(|d| {
                        d.starts_with(PARTS_LINE_PREFIX) || d.starts_with("Line item — ")
                    })
            })
        })
        .unwrap_or(false)
}

/// The description prefix of the parts line on an in-house bill.
pub const PARTS_LINE_PREFIX: &str = "Parts & materials";

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(target: i64) -> Rules {
        Rules {
            tz: chrono_tz::America::Los_Angeles,
            overtime: super::super::overtime::Rule::Weekly,
            burden_bps: 0,
            overhead_per_hour_cents: 1_000,
            target_margin_bps: target,
            mileage_rate_mills: 700,
            markup_bps: 1_000,
            missed_punch_hours: 12,
            clock_location: false,
            clock_radius_m: 400,
        }
    }

    #[test]
    fn margins_and_the_bill_rate_that_reaches_target() {
        // 2h at $25 pay, billed at $60; $40 of parts billed at +10%.
        let mut c = Cost {
            minutes: 120,
            labor_pay_cents: 5_000,
            parts_cents: 4_000,
            unbilled_cents: 12_000 + 4_400,
            ..Default::default()
        };
        c.labor_by.insert(Uuid::nil(), 12_000);
        c.finish(&rules(5_000));
        assert_eq!(c.costs_cents, 9_000);
        assert_eq!(c.revenue_cents, 16_400);
        assert_eq!(c.gross_cents, 7_400);
        assert_eq!(c.gross_bps, 4_512);
        assert_eq!(c.overhead_cents, 2_000);
        assert_eq!(c.net_cents, 5_400);
        // Revenue for 50% = $180; parts bring $44 → labor $136 over 2h = $68/h.
        assert_eq!(c.bill_rate_for_target_cents, 6_800);
    }

    #[test]
    fn markup_and_bps() {
        assert_eq!(with_markup(4_000, 1_000), 4_400);
        assert_eq!(with_markup(333, 1_500), 383);
        assert_eq!(bps(1, 3), 3_333);
        assert_eq!(bps(5, 0), 0);
    }
}
