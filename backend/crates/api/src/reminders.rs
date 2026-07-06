//! The **calendar / reminders engine** (issue #54) — the cross-cutting
//! scheduling substrate that lease renewals, license / insurance expirations,
//! tours, and inspections all ride.
//!
//! One durable, self-rescheduling `reminder_scan` job per tenant (the
//! `billing_cycle` pattern) runs on an interval and, idempotently:
//!
//! 1. **syncs lease renewals** — every active lease with an end date gets an
//!    active `reminder` (subject `lease`), kept in step if the end date
//!    moves; and
//! 2. **fires due reminders** — for each active reminder, every configured
//!    lead time whose window has opened notifies once: staff holding
//!    `calendar:read` get the in-app/push fan-out, and each external
//!    recipient gets an `auto_email`. Fired leads are recorded on the row
//!    (and the notification layer's idempotency key backstops it), so a
//!    reminder never double-sends.

use crate::modules::JobOutcome;
use chrono::{NaiveDate, Utc};
use entity::prelude::{BackgroundJob, Lease, Reminder, Tenant};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    Set,
};
use serde_json::json;
use uuid::Uuid;

pub const SCAN_KIND: &str = "reminder_scan";

/// Subject types a reminder can carry.
pub const SUBJECT_TYPES: &[&str] = &[
    "lease",
    "license",
    "insurance",
    "tour",
    "inspection",
    "custom",
];

/// Reminder statuses.
pub const STATUSES: &[&str] = &["active", "done", "cancelled"];

// ---------------------------------------------------------------------------
// Recurring scan bootstrap (the billing-cycle pattern)
// ---------------------------------------------------------------------------

/// Ensure every tenant has exactly one live `reminder_scan` job. Called at
/// boot (and after tenant provisioning); idempotent.
pub async fn ensure_recurring_jobs(db: &DatabaseConnection) {
    let tenants = match Tenant::find().all(db).await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("reminders: tenant scan failed: {e}");
            return;
        }
    };
    for tenant in tenants {
        if let Err(e) = ensure_scan_for_tenant(db, tenant.id).await {
            tracing::error!("reminders: ensure scan for {} failed: {e}", tenant.id);
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
        crate::scheduler::enqueue(db, tenant_id, SCAN_KIND, json!({}), 10).await?;
        tracing::info!(tenant = %tenant_id, "reminder scan scheduled");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Pure helpers (unit-tested)
// ---------------------------------------------------------------------------

/// Parse a comma-separated lead-days setting (`"30,7,1"`) into a sorted,
/// deduplicated list. Invalid fragments are skipped; an unusable value falls
/// back to `[7, 1]`.
pub fn parse_lead_days(raw: &str) -> Vec<i64> {
    let mut days: Vec<i64> = raw
        .split(',')
        .filter_map(|s| s.trim().parse::<i64>().ok())
        .filter(|d| (0..=365).contains(d))
        .collect();
    days.sort_unstable();
    days.dedup();
    days.reverse(); // largest lead first
    if days.is_empty() {
        vec![7, 1]
    } else {
        days
    }
}

/// Days from `today` until `due` (negative = overdue), when both parse.
pub fn days_until(due: &str, today: NaiveDate) -> Option<i64> {
    let due = NaiveDate::parse_from_str(due, "%Y-%m-%d").ok()?;
    Some((due - today).num_days())
}

/// Which lead times have entered their window and not yet fired. A reminder
/// created late (or a scan that was down) yields several at once — the
/// caller notifies once for the most urgent and marks them all fired.
pub fn due_leads(lead_days: &[i64], fired: &[i64], days_left: i64) -> Vec<i64> {
    lead_days
        .iter()
        .copied()
        .filter(|lead| days_left <= *lead && !fired.contains(lead))
        .collect()
}

fn json_i64_vec(v: &serde_json::Value) -> Vec<i64> {
    v.as_array()
        .map(|a| a.iter().filter_map(|x| x.as_i64()).collect())
        .unwrap_or_default()
}

fn json_string_vec(v: &serde_json::Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// The scan job
// ---------------------------------------------------------------------------

/// Advance one `reminder_scan` job, then go back to sleep.
pub async fn handle_scan_job(
    db: &DatabaseConnection,
    job: &entity::background_job::Model,
) -> JobOutcome {
    let tenant_id = job.tenant_id;
    let today = Utc::now().date_naive();

    let mut summary = json!({});
    match sync_lease_renewals(db, tenant_id).await {
        Ok(n) => summary["lease_reminders_synced"] = json!(n),
        Err(e) => tracing::error!("reminders: lease sync failed: {e}"),
    }
    match fire_due(db, tenant_id, today).await {
        Ok(n) => summary["reminders_fired"] = json!(n),
        Err(e) => tracing::error!("reminders: firing failed: {e}"),
    }

    tracing::info!(tenant = %tenant_id, ?summary, "reminder scan ran");
    let interval =
        crate::settings::get_i64(db, tenant_id, crate::settings::CALENDAR_SCAN_INTERVAL_SECS)
            .await
            .clamp(60, 24 * 3600);
    let mut outcome = JobOutcome::reschedule("pending", interval);
    outcome.result = Some(summary);
    outcome
}

/// Every active lease with an end date keeps one active `lease` reminder,
/// created (or re-dated) idempotently. Gated by the
/// `calendar.lease_renewal_sync` setting.
async fn sync_lease_renewals(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
) -> Result<u32, sea_orm::DbErr> {
    if !crate::settings::get_bool(db, tenant_id, crate::settings::CALENDAR_LEASE_RENEWAL_SYNC).await
    {
        return Ok(0);
    }
    let default_leads = parse_lead_days(
        &crate::settings::get_string(db, tenant_id, crate::settings::CALENDAR_DEFAULT_LEAD_DAYS)
            .await,
    );
    let leases = Lease::find()
        .filter(entity::lease::Column::TenantId.eq(tenant_id))
        .filter(entity::lease::Column::Status.eq("active"))
        .filter(entity::lease::Column::EndDate.is_not_null())
        .all(db)
        .await?;
    let mut synced = 0u32;
    let now = Utc::now();
    for lease in leases {
        let Some(end_date) = lease.end_date.clone().filter(|d| !d.is_empty()) else {
            continue;
        };
        let existing = Reminder::find()
            .filter(entity::reminder::Column::TenantId.eq(tenant_id))
            .filter(entity::reminder::Column::SubjectType.eq("lease"))
            .filter(entity::reminder::Column::SubjectId.eq(lease.id))
            .filter(entity::reminder::Column::Status.eq("active"))
            .one(db)
            .await?;
        match existing {
            Some(r) if r.due_date == end_date => {}
            Some(r) => {
                // The lease end moved — re-date the reminder and let the new
                // window's leads fire fresh.
                let mut am: entity::reminder::ActiveModel = r.into();
                am.due_date = Set(end_date);
                am.fired = Set(json!([]));
                am.updated_at = Set(now.into());
                am.update(db).await?;
                synced += 1;
            }
            None => {
                let title = format!("Lease renewal — {}", lease.tenant_name);
                entity::reminder::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    tenant_id: Set(tenant_id),
                    subject_type: Set("lease".into()),
                    subject_id: Set(Some(lease.id)),
                    title: Set(title),
                    description: Set(Some(
                        "Lease term ends — start the renewal conversation or plan turnover.".into(),
                    )),
                    due_date: Set(end_date),
                    lead_days: Set(json!(default_leads)),
                    recipients: Set(json!([])),
                    fired: Set(json!([])),
                    status: Set("active".into()),
                    completed_at: Set(None),
                    created_by: Set(None),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(db)
                .await?;
                synced += 1;
            }
        }
    }
    Ok(synced)
}

/// Fire every active reminder whose next lead window has opened.
async fn fire_due(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    today: NaiveDate,
) -> Result<u32, sea_orm::DbErr> {
    let reminders = Reminder::find()
        .filter(entity::reminder::Column::TenantId.eq(tenant_id))
        .filter(entity::reminder::Column::Status.eq("active"))
        .all(db)
        .await?;
    let mut fired_count = 0u32;
    let now = Utc::now();
    for reminder in reminders {
        let Some(days_left) = days_until(&reminder.due_date, today) else {
            continue;
        };
        // Long past due — stop nagging (the console still shows it overdue).
        if days_left < -30 {
            continue;
        }
        let leads = json_i64_vec(&reminder.lead_days);
        let already = json_i64_vec(&reminder.fired);
        let due_now = due_leads(&leads, &already, days_left);
        if due_now.is_empty() {
            continue;
        }
        // Notify once, at the most urgent newly-opened lead.
        let lead = *due_now.iter().min().unwrap_or(&0);
        fire_reminder(db, tenant_id, &reminder, lead, days_left).await;

        let mut all_fired = already;
        all_fired.extend(due_now.iter().copied());
        all_fired.sort_unstable();
        all_fired.dedup();
        let mut am: entity::reminder::ActiveModel = reminder.into();
        am.fired = Set(json!(all_fired));
        am.updated_at = Set(now.into());
        am.update(db).await?;
        fired_count += 1;
    }
    Ok(fired_count)
}

/// One reminder notification: staff fan-out + an email per external recipient.
async fn fire_reminder(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    reminder: &entity::reminder::Model,
    lead: i64,
    days_left: i64,
) {
    let vars = json!({
        "title": reminder.title,
        "due_date": reminder.due_date,
        "days_left": days_left.max(0),
        "description": reminder
            .description
            .clone()
            .unwrap_or_else(|| format!("{} reminder", reminder.subject_type)),
    });
    let trigger = format!("lead_{lead}");

    crate::notify::notify_staff(
        db,
        tenant_id,
        "calendar:read",
        "reminder_due",
        vars.clone(),
        Some(("reminder", reminder.id)),
        &trigger,
        None,
    )
    .await;

    for email in json_string_vec(&reminder.recipients) {
        let payload = json!({
            "template": "reminder_due",
            "to": email,
            "owner_type": "reminder",
            "owner_id": reminder.id,
            "trigger": trigger,
            "vars": vars,
        });
        if let Err(e) = crate::scheduler::enqueue(db, tenant_id, "auto_email", payload, 0).await {
            tracing::error!("failed to enqueue reminder email: {e}");
        }
    }

    crate::audit::record(
        db,
        None,
        crate::audit::actions::REMINDER_FIRE,
        Some("reminder"),
        Some(reminder.id.to_string()),
        Some(tenant_id),
        Some(json!({
            "subject_type": reminder.subject_type,
            "due_date": reminder.due_date,
            "lead_days": lead,
            "days_left": days_left,
        })),
    )
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lead_days_parse_sorted_deduped() {
        assert_eq!(parse_lead_days("30,7,1"), vec![30, 7, 1]);
        assert_eq!(parse_lead_days("1, 7,7 ,30"), vec![30, 7, 1]);
        // Junk and out-of-range values are skipped…
        assert_eq!(parse_lead_days("x,7,-2,9999"), vec![7]);
        // …and a fully unusable setting falls back rather than going silent.
        assert_eq!(parse_lead_days(""), vec![7, 1]);
        assert_eq!(parse_lead_days("nope"), vec![7, 1]);
        // Day-of (0) is a valid lead.
        assert_eq!(parse_lead_days("0"), vec![0]);
    }

    #[test]
    fn days_until_handles_bounds() {
        let today = NaiveDate::from_ymd_opt(2026, 7, 6).unwrap();
        assert_eq!(days_until("2026-07-13", today), Some(7));
        assert_eq!(days_until("2026-07-06", today), Some(0));
        assert_eq!(days_until("2026-07-01", today), Some(-5));
        assert_eq!(days_until("not-a-date", today), None);
    }

    #[test]
    fn leads_fire_once_per_window() {
        // 5 days out with [30, 7, 1]: 30 and 7 have opened, 1 has not.
        assert_eq!(due_leads(&[30, 7, 1], &[], 5), vec![30, 7]);
        // After marking them fired, nothing re-fires until day 1's window.
        assert_eq!(due_leads(&[30, 7, 1], &[30, 7], 5), Vec::<i64>::new());
        assert_eq!(due_leads(&[30, 7, 1], &[30, 7], 1), vec![1]);
        // Overdue keeps only unfired leads (all windows are open).
        assert_eq!(due_leads(&[7, 1], &[7, 1], -3), Vec::<i64>::new());
        assert_eq!(due_leads(&[7, 1, 0], &[7, 1], -3), vec![0]);
    }
}
