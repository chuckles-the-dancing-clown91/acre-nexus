//! **Helpdesk & maintenance operations** (roadmap Phase 6, issue #10) — the
//! support-desk layer on top of the maintenance module.
//!
//! * **SLA policy**: per-priority first-response and resolution targets
//!   (tenant settings, `priority:hours` pairs) stamped onto every ticket at
//!   create and re-stamped on priority change while the target is still
//!   open. The DTO derives a state per target: `met` / `on_track` /
//!   `breached` (or `none` when the policy disables it).
//! * **The helpdesk scan**: one durable, self-rescheduling `helpdesk_scan`
//!   job per tenant (the billing-cycle/reminder-scan pattern) that notifies
//!   maintenance staff of newly breached tickets and opens tickets for due
//!   **preventive-maintenance plans**, advancing each plan's next-due date.
//! * **Turnover**: completing a move-out inspection auto-opens a make-ready
//!   ticket and flags the unit (setting-gated), so the turn starts itself.

use crate::error::ApiResult;
use crate::modules::JobOutcome;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use entity::prelude::{
    BackgroundJob, InventoryItem, MaintenancePlan, MaintenanceTicket, Tenant, Unit,
};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, EntityTrait,
    QueryFilter, Set,
};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

/// The per-tenant helpdesk scan job kind (owned by the maintenance module).
pub const SCAN_KIND: &str = "helpdesk_scan";

// ---------------------------------------------------------------------------
// Pure helpers (unit-tested)
// ---------------------------------------------------------------------------

/// Parse a `priority:hours` list (`"urgent:4,high:8"`) into a map. Invalid
/// fragments are skipped; `0` (or a missing priority) means "no target".
pub fn parse_sla_hours(raw: &str) -> HashMap<String, i64> {
    raw.split(',')
        .filter_map(|pair| {
            let (p, h) = pair.split_once(':')?;
            let hours = h.trim().parse::<i64>().ok()?;
            if !(1..=24 * 365).contains(&hours) {
                return None;
            }
            Some((p.trim().to_lowercase(), hours))
        })
        .collect()
}

/// The SLA state of one target: `none` (no policy), `met` (done in time),
/// `breached` (past due — whether or not it eventually completed late), or
/// `on_track`.
pub fn sla_state(
    due: Option<DateTime<Utc>>,
    done: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> &'static str {
    match (due, done) {
        (None, _) => "none",
        (Some(due), Some(done)) if done <= due => "met",
        (Some(_), Some(_)) => "breached",
        (Some(due), None) if now > due => "breached",
        (Some(_), None) => "on_track",
    }
}

/// Advance a plan's due date past `today` by whole cadences — a scan that was
/// down for weeks generates one ticket, not a backlog.
pub fn advance_due(next_due: &str, cadence_days: i32, today: NaiveDate) -> String {
    let cadence = i64::from(cadence_days.max(1));
    let mut due = NaiveDate::parse_from_str(next_due, "%Y-%m-%d").unwrap_or(today);
    while due <= today {
        due += Duration::days(cadence);
    }
    due.to_string()
}

// ---------------------------------------------------------------------------
// SLA stamping
// ---------------------------------------------------------------------------

/// The tenant's SLA targets for a priority, measured from `from`:
/// `(response_due, resolve_due)`.
pub async fn sla_targets(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    priority: &str,
    from: DateTime<Utc>,
) -> (Option<DateTime<Utc>>, Option<DateTime<Utc>>) {
    let response = parse_sla_hours(
        &crate::settings::get_string(db, tenant_id, crate::settings::HELPDESK_SLA_RESPONSE_HOURS)
            .await,
    );
    let resolve = parse_sla_hours(
        &crate::settings::get_string(db, tenant_id, crate::settings::HELPDESK_SLA_RESOLVE_HOURS)
            .await,
    );
    let p = priority.to_lowercase();
    (
        response.get(&p).map(|h| from + Duration::hours(*h)),
        resolve.get(&p).map(|h| from + Duration::hours(*h)),
    )
}

// ---------------------------------------------------------------------------
// Recurring scan
// ---------------------------------------------------------------------------

/// Ensure every tenant has exactly one live `helpdesk_scan` job. Called at
/// boot after migrations.
pub async fn ensure_recurring_jobs(db: &DatabaseConnection) {
    let tenants = match Tenant::find().all(db).await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("helpdesk: tenant scan failed: {e}");
            return;
        }
    };
    for tenant in tenants {
        if let Err(e) = ensure_scan_for_tenant(db, tenant.id).await {
            tracing::error!("helpdesk: ensure scan for {} failed: {e}", tenant.id);
        }
    }
}

/// Ensure one live scan job for a single tenant (used by provisioning too).
pub async fn ensure_scan_for_tenant(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
) -> Result<(), sea_orm::DbErr> {
    let existing = BackgroundJob::find()
        .filter(entity::background_job::Column::TenantId.eq(tenant_id))
        .filter(entity::background_job::Column::Kind.eq(SCAN_KIND))
        .filter(entity::background_job::Column::Status.is_in([
            "pending",
            "running",
            "awaiting_callback",
        ]))
        .one(db)
        .await?;
    if existing.is_none() {
        crate::scheduler::enqueue(db, tenant_id, SCAN_KIND, json!({}), 15).await?;
        tracing::info!(tenant = %tenant_id, "helpdesk scan scheduled");
    }
    Ok(())
}

/// One scan pass: breach notifications + preventive-plan ticket generation,
/// then sleep until the next interval.
pub async fn handle_scan_job(
    db: &DatabaseConnection,
    job: &entity::background_job::Model,
) -> JobOutcome {
    let tenant_id = job.tenant_id;

    let mut summary = json!({});
    match notify_breaches(db, tenant_id).await {
        Ok(n) => summary["sla_breaches_notified"] = json!(n),
        Err(e) => tracing::error!("helpdesk: breach scan failed: {e}"),
    }
    match run_due_plans(db, tenant_id).await {
        Ok(n) => summary["plan_tickets_opened"] = json!(n),
        Err(e) => tracing::error!("helpdesk: plan scan failed: {e}"),
    }
    match notify_follow_ups(db, tenant_id).await {
        Ok(n) => summary["follow_ups_notified"] = json!(n),
        Err(e) => tracing::error!("helpdesk: follow-up scan failed: {e}"),
    }
    match notify_low_stock(db, tenant_id).await {
        Ok(n) => summary["low_stock_notified"] = json!(n),
        Err(e) => tracing::error!("helpdesk: low-stock scan failed: {e}"),
    }

    tracing::info!(tenant = %tenant_id, ?summary, "helpdesk scan ran");
    let interval =
        crate::settings::get_i64(db, tenant_id, crate::settings::HELPDESK_SCAN_INTERVAL_SECS)
            .await
            .clamp(60, 24 * 3600);
    let mut outcome = JobOutcome::reschedule("pending", interval);
    outcome.result = Some(summary);
    outcome
}

/// Notify maintenance staff of open tickets past an SLA target. The
/// notification substrate's idempotency key (owner + trigger) makes each
/// breach fire once per ticket per target.
async fn notify_breaches(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
) -> Result<u32, sea_orm::DbErr> {
    let now = Utc::now();
    let open = MaintenanceTicket::find()
        .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
        .filter(
            entity::maintenance_ticket::Column::Status
                .is_in(crate::routes::maintenance::OPEN_STATUSES.to_vec()),
        )
        .all(db)
        .await?;

    let mut notified = 0u32;
    for ticket in open {
        let breaches = [
            (
                "response",
                ticket.sla_response_due_at,
                ticket.first_response_at,
            ),
            ("resolution", ticket.sla_resolve_due_at, ticket.resolved_at),
        ];
        for (kind, due, done) in breaches {
            let breached_open = matches!(
                (due, done),
                (Some(due), None) if now > due.with_timezone(&Utc)
            );
            if !breached_open {
                continue;
            }
            crate::notify::notify_staff(
                db,
                tenant_id,
                "maintenance:read",
                "ticket_sla_breached",
                json!({
                    "title": ticket.title,
                    "priority": ticket.priority,
                    "kind": kind,
                }),
                Some(("maintenance_ticket", ticket.id)),
                &format!("sla_breach:{kind}"),
                None,
            )
            .await;
            notified += 1;
        }
    }
    Ok(notified)
}

/// Open tickets for every active plan whose due date arrived, advancing the
/// plan past today.
async fn run_due_plans(db: &impl ConnectionTrait, tenant_id: Uuid) -> ApiResult<u32> {
    let today = Utc::now().date_naive();
    let plans = MaintenancePlan::find()
        .filter(entity::maintenance_plan::Column::TenantId.eq(tenant_id))
        .filter(entity::maintenance_plan::Column::Active.eq(true))
        .all(db)
        .await?;

    let mut opened = 0u32;
    for plan in plans {
        let due = NaiveDate::parse_from_str(&plan.next_due_date, "%Y-%m-%d").unwrap_or(today);
        if due > today {
            continue;
        }
        let ticket = open_ticket(
            db,
            tenant_id,
            OpenTicket {
                property_id: plan.property_id,
                unit_id: plan.unit_id,
                lease_id: None,
                title: plan.title.clone(),
                description: plan.description.clone(),
                category: plan.category.clone(),
                priority: plan.priority.clone(),
                reporter: Some("Preventive maintenance".into()),
                due_date: Some(plan.next_due_date.clone()),
            },
            None,
        )
        .await?;

        let next = advance_due(&plan.next_due_date, plan.cadence_days, today);
        let plan_id = plan.id;
        let mut am: entity::maintenance_plan::ActiveModel = plan.into();
        am.next_due_date = Set(next);
        am.last_ticket_id = Set(Some(ticket.id));
        am.updated_at = Set(Utc::now().into());
        am.update(db).await?;

        crate::audit::record(
            db,
            None,
            crate::audit::actions::MAINTENANCE_PLAN_RUN,
            Some("maintenance_plan"),
            Some(plan_id.to_string()),
            Some(tenant_id),
            Some(json!({ "ticket_id": ticket.id })),
        )
        .await;
        opened += 1;
    }
    Ok(opened)
}

/// Chase waiting-on tickets whose follow-up date arrived: notify maintenance
/// staff once per ticket per follow-up date (the trigger carries the date, so
/// re-dating the follow-up re-arms the reminder).
async fn notify_follow_ups(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
) -> Result<u32, sea_orm::DbErr> {
    let today = Utc::now().date_naive().to_string();
    let waiting = MaintenanceTicket::find()
        .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
        .filter(entity::maintenance_ticket::Column::WaitingOn.is_not_null())
        .filter(entity::maintenance_ticket::Column::FollowUpDate.lte(today.clone()))
        .filter(
            entity::maintenance_ticket::Column::Status
                .is_in(crate::routes::maintenance::OPEN_STATUSES.to_vec()),
        )
        .all(db)
        .await?;
    let mut notified = 0u32;
    for ticket in waiting {
        let (Some(waiting_on), Some(date)) = (&ticket.waiting_on, &ticket.follow_up_date) else {
            continue;
        };
        crate::notify::notify_staff(
            db,
            tenant_id,
            "maintenance:read",
            "ticket_follow_up",
            json!({
                "title": ticket.title,
                "waiting_on": waiting_on,
                "date": date,
            }),
            Some(("maintenance_ticket", ticket.id)),
            &format!("follow_up:{date}"),
            None,
        )
        .await;
        notified += 1;
    }
    Ok(notified)
}

/// Flag stock that fell to its reorder level — once per episode.
/// `low_stock_alerted_at` marks an alert as out; restocking above the level
/// clears it, so the next drop alerts again (a permanent dedupe key like the
/// quantity alone would silence any repeat of a previously-seen quantity
/// forever). Only rows with something to do are fetched: newly-low items
/// plus items with an outstanding alert to re-arm.
async fn notify_low_stock(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
) -> Result<u32, sea_orm::DbErr> {
    use entity::inventory_item::Column as Inv;
    let is_low = Expr::col(Inv::ReorderLevel)
        .gt(0)
        .and(Expr::col(Inv::Quantity).lte(Expr::col(Inv::ReorderLevel)));
    let items = InventoryItem::find()
        .filter(Inv::TenantId.eq(tenant_id))
        .filter(Inv::Status.eq("active"))
        .filter(
            Condition::any()
                .add(Inv::LowStockAlertedAt.is_not_null())
                .add(is_low),
        )
        .all(db)
        .await?;
    let now = Utc::now();
    let mut notified = 0u32;
    for item in items {
        let low = item.reorder_level > 0 && item.quantity <= item.reorder_level;
        if low && item.low_stock_alerted_at.is_none() {
            crate::notify::notify_staff(
                db,
                tenant_id,
                "maintenance:read",
                "inventory_low",
                json!({
                    "name": item.name,
                    "quantity": item.quantity,
                    "reorder_level": item.reorder_level,
                }),
                Some(("inventory_item", item.id)),
                &format!("low_stock:{}", now.timestamp()),
                None,
            )
            .await;
            let mut am: entity::inventory_item::ActiveModel = item.into();
            am.low_stock_alerted_at = Set(Some(now.into()));
            am.update(db).await?;
            notified += 1;
        } else if !low && item.low_stock_alerted_at.is_some() {
            // Restocked — re-arm the alert for the next episode.
            let mut am: entity::inventory_item::ActiveModel = item.into();
            am.low_stock_alerted_at = Set(None);
            am.update(db).await?;
        }
    }
    Ok(notified)
}

// ---------------------------------------------------------------------------
// Ticket creation shared by the scan + turnover hook
// ---------------------------------------------------------------------------

/// Everything needed to open a system-generated work order.
pub struct OpenTicket {
    pub property_id: Uuid,
    pub unit_id: Option<Uuid>,
    pub lease_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub category: String,
    pub priority: String,
    pub reporter: Option<String>,
    pub due_date: Option<String>,
}

/// Insert a ticket with SLA targets stamped, audit it, and notify
/// maintenance staff — the shared path for plan-generated and turnover
/// tickets.
pub async fn open_ticket(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    spec: OpenTicket,
    created_by: Option<Uuid>,
) -> ApiResult<entity::maintenance_ticket::Model> {
    let now = Utc::now();
    let (response_due, resolve_due) = sla_targets(db, tenant_id, &spec.priority, now).await;
    let saved = entity::maintenance_ticket::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        property_id: Set(spec.property_id),
        unit_id: Set(spec.unit_id),
        lease_id: Set(spec.lease_id),
        title: Set(spec.title),
        description: Set(spec.description),
        category: Set(spec.category),
        priority: Set(spec.priority),
        status: Set("open".into()),
        assignee_user_id: Set(None),
        assignee_entity_id: Set(None),
        reporter: Set(spec.reporter),
        location: Set(None),
        access_notes: Set(None),
        permission_to_enter: Set(false),
        asset_id: Set(None),
        waiting_on: Set(None),
        follow_up_date: Set(None),
        rating: Set(None),
        review_comment: Set(None),
        reviewed_at: Set(None),
        due_date: Set(spec.due_date),
        cost_cents: Set(None),
        first_response_at: Set(None),
        resolved_at: Set(None),
        sla_response_due_at: Set(response_due.map(Into::into)),
        sla_resolve_due_at: Set(resolve_due.map(Into::into)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;

    crate::audit::record(
        db,
        created_by,
        crate::audit::actions::TICKET_CREATE,
        Some("maintenance_ticket"),
        Some(saved.id.to_string()),
        Some(tenant_id),
        Some(json!({
            "property_id": saved.property_id,
            "category": saved.category,
            "priority": saved.priority,
            "generated": true,
        })),
    )
    .await;

    crate::notify::notify_staff(
        db,
        tenant_id,
        "maintenance:read",
        "ticket_created",
        json!({ "title": saved.title, "priority": saved.priority }),
        Some(("maintenance_ticket", saved.id)),
        "created",
        created_by,
    )
    .await;

    Ok(saved)
}

/// The turnover hook: a completed move-out inspection opens a make-ready
/// ticket and flags the unit (gated by `helpdesk.auto_turnover`).
pub async fn open_turnover_ticket(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    inspection: &entity::inspection::Model,
    completed_by: Uuid,
) -> ApiResult<Option<entity::maintenance_ticket::Model>> {
    if !crate::settings::get_bool(db, tenant_id, crate::settings::HELPDESK_AUTO_TURNOVER).await {
        return Ok(None);
    }
    let unit_label = match inspection.unit_id {
        Some(uid) => Unit::find_by_id(uid)
            .filter(entity::unit::Column::TenantId.eq(tenant_id))
            .one(db)
            .await?
            .map(|u| format!(" — Unit {}", u.unit_number))
            .unwrap_or_default(),
        None => String::new(),
    };
    let ticket = open_ticket(
        db,
        tenant_id,
        OpenTicket {
            property_id: inspection.property_id,
            unit_id: inspection.unit_id,
            lease_id: Some(inspection.lease_id),
            title: format!("Turnover / make-ready{unit_label}"),
            description: Some(
                "Auto-opened by the completed move-out inspection: turn the unit \
                 (repairs from the inspection report, cleaning, rekey) and list it."
                    .into(),
            ),
            category: "general".into(),
            priority: "high".into(),
            reporter: Some("Move-out inspection".into()),
            due_date: None,
        },
        Some(completed_by),
    )
    .await?;

    // Flag the unit make-ready so the rentals board reflects the turn.
    if let Some(uid) = inspection.unit_id {
        if let Some(unit) = Unit::find_by_id(uid)
            .filter(entity::unit::Column::TenantId.eq(tenant_id))
            .one(db)
            .await?
        {
            let mut am: entity::unit::ActiveModel = unit.into();
            am.status = Set("make_ready".into());
            am.update(db).await?;
        }
    }

    Ok(Some(ticket))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn sla_hours_parse_and_skip_junk() {
        let map = parse_sla_hours("urgent:4, high:8, normal:24, low:72");
        assert_eq!(map.get("urgent"), Some(&4));
        assert_eq!(map.get("low"), Some(&72));
        // Zero, negatives, and junk fragments disable rather than panic.
        let map = parse_sla_hours("urgent:0,high:-3,weird,normal:24:7,low:48");
        assert_eq!(map.get("urgent"), None);
        assert_eq!(map.get("high"), None);
        assert_eq!(map.get("normal"), None);
        assert_eq!(map.get("low"), Some(&48));
        assert!(parse_sla_hours("").is_empty());
    }

    #[test]
    fn sla_states_cover_the_matrix() {
        let t0 = Utc.with_ymd_and_hms(2026, 7, 1, 12, 0, 0).unwrap();
        let due = Some(t0);
        let before = t0 - Duration::hours(1);
        let after = t0 + Duration::hours(1);
        assert_eq!(sla_state(None, None, after), "none");
        assert_eq!(sla_state(due, Some(before), after), "met");
        assert_eq!(sla_state(due, Some(after), after), "breached"); // done late
        assert_eq!(sla_state(due, None, before), "on_track");
        assert_eq!(sla_state(due, None, after), "breached");
    }

    #[test]
    fn plan_due_dates_advance_past_today_without_backlog() {
        let today = NaiveDate::from_ymd_opt(2026, 7, 7).unwrap();
        // Due today → next cadence.
        assert_eq!(advance_due("2026-07-07", 30, today), "2026-08-06");
        // Long-overdue plan skips the missed occurrences.
        assert_eq!(advance_due("2026-01-01", 30, today), "2026-07-30");
        // Future dates are left alone.
        assert_eq!(advance_due("2026-09-01", 30, today), "2026-09-01");
        // Garbage dates restart from today.
        assert_eq!(advance_due("not-a-date", 7, today), "2026-07-14");
    }
}
