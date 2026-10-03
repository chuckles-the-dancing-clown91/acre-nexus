//! **Appointments**: a repair visit someone has to be home for, a showing,
//! an inspection. Staff offer up to a few windows; the resident, prospect or
//! vendor picks one from the portal or a one-time link (or asks for another
//! time); reminders go out before it; the work order follows along.
//!
//! The pure parts (parsing windows, which reminders are due, how a window
//! reads) are unit-tested; the database parts are covered by the
//! integration suite.

use crate::error::{ApiError, ApiResult};
use crate::modules::JobOutcome;
use chrono::{DateTime, Datelike, Duration, Timelike, Utc};
use entity::prelude::{Appointment, Lease, MaintenanceTicket, Property, Tenant, User};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

pub const KINDS: &[&str] = &["repair", "showing", "inspection", "other"];
pub const ROLES: &[&str] = &["resident", "prospect", "vendor", "owner"];
/// The reminder job, one per workspace, self-rescheduling.
pub const REMINDER_KIND: &str = "appointment_reminders";
const REMINDER_INTERVAL_SECS: i64 = 15 * 60;
pub const MAX_WINDOWS: usize = 4;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, schemars::JsonSchema)]
pub struct Window {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

/// Windows as offered: in order, each at least 15 minutes and at most a
/// day, none in the past, no more than [`MAX_WINDOWS`].
pub fn clean_windows(raw: Vec<Window>, now: DateTime<Utc>) -> Result<Vec<Window>, String> {
    if raw.is_empty() {
        return Err("offer at least one time window".into());
    }
    if raw.len() > MAX_WINDOWS {
        return Err(format!("offer at most {MAX_WINDOWS} windows"));
    }
    let mut out: Vec<Window> = Vec::new();
    for w in raw {
        let len = w.end - w.start;
        if len < Duration::minutes(15) {
            return Err("a window needs at least 15 minutes".into());
        }
        if len > Duration::hours(24) {
            return Err("a window can't be longer than a day".into());
        }
        if w.end < now {
            return Err("that window has already passed".into());
        }
        if out.iter().any(|o| o.start < w.end && w.start < o.end) {
            return Err("windows overlap".into());
        }
        out.push(w);
    }
    out.sort_by_key(|w| w.start);
    Ok(out)
}

pub fn windows_of(v: &Value) -> Vec<Window> {
    serde_json::from_value(v.clone()).unwrap_or_default()
}

/// "Tue, Oct 7, 9:00 AM to 11:00 AM" in the workspace's time zone.
pub fn window_words(w: &Window, tz: &chrono_tz::Tz) -> String {
    let s = w.start.with_timezone(tz);
    let e = w.end.with_timezone(tz);
    let day = s.format("%a, %b %-d");
    let hm = |t: chrono::DateTime<chrono_tz::Tz>| {
        let (pm, h12) = t.hour12();
        if t.minute() == 0 {
            format!("{h12} {}", if pm { "PM" } else { "AM" })
        } else {
            format!("{h12}:{:02} {}", t.minute(), if pm { "PM" } else { "AM" })
        }
    };
    if s.date_naive() == e.date_naive() {
        format!("{day}, {} to {}", hm(s), hm(e))
    } else {
        format!("{day}, {} to {}, {}", hm(s), e.format("%a, %b %-d"), hm(e))
    }
}

/// Reminder lead hours whose time has come: `starts_at - lead` is in the
/// past but the visit is still ahead, and that lead hasn't been sent.
pub fn reminders_due(
    leads: &[i64],
    already: &[i64],
    starts_at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Vec<i64> {
    if starts_at <= now {
        return vec![];
    }
    leads
        .iter()
        .copied()
        .filter(|h| !already.contains(h))
        .filter(|h| starts_at - Duration::hours(*h) <= now)
        .collect()
}

pub fn parse_hours(raw: &str) -> Vec<i64> {
    let mut v: Vec<i64> = raw
        .split(',')
        .filter_map(|s| s.trim().parse::<i64>().ok())
        .filter(|h| (1..=24 * 14).contains(h))
        .collect();
    v.sort_unstable();
    v.dedup();
    v.reverse();
    if v.is_empty() {
        vec![24, 2]
    } else {
        v
    }
}

pub fn ids_of(v: &Value) -> Vec<i64> {
    serde_json::from_value(v.clone()).unwrap_or_default()
}

pub async fn tz_for(db: &impl ConnectionTrait, tenant_id: Uuid) -> chrono_tz::Tz {
    crate::settings::get_string(db, tenant_id, crate::settings::TEXTS_TIMEZONE)
        .await
        .parse()
        .unwrap_or(chrono_tz::America::Los_Angeles)
}

/// The public link a token opens.
pub fn book_url(token: &str) -> String {
    format!("{}/book/{token}", crate::oauth::public_app_url())
}

// ---------------------------------------------------------------------------
// Creating and moving an appointment
// ---------------------------------------------------------------------------

pub struct Offer {
    pub property_id: Uuid,
    pub unit_id: Option<Uuid>,
    pub kind: String,
    pub subject_type: String,
    pub subject_id: Option<Uuid>,
    pub title: String,
    pub windows: Vec<Window>,
    pub with_name: Option<String>,
    pub with_email: Option<String>,
    pub with_phone: Option<String>,
    pub with_role: String,
    pub assignee_user_id: Option<Uuid>,
    pub vendor_entity_id: Option<Uuid>,
    pub note: Option<String>,
    pub access_notes: Option<String>,
    pub created_by: Option<Uuid>,
}

/// Offer windows and tell the person. One open appointment per subject: a
/// new offer on a work order that already has one replaces it.
pub async fn offer(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    o: Offer,
) -> ApiResult<(entity::appointment::Model, String)> {
    let now = Utc::now();
    if let Some(sid) = o.subject_id {
        let open = Appointment::find()
            .filter(entity::appointment::Column::TenantId.eq(tenant_id))
            .filter(entity::appointment::Column::SubjectType.eq(o.subject_type.as_str()))
            .filter(entity::appointment::Column::SubjectId.eq(sid))
            .filter(entity::appointment::Column::Status.is_in(["proposed", "confirmed"]))
            .all(db)
            .await?;
        for a in open {
            let mut am: entity::appointment::ActiveModel = a.into();
            am.status = Set("cancelled".into());
            am.outcome_note = Set(Some("Replaced by a new offer".into()));
            am.updated_at = Set(now.into());
            am.update(db).await?;
        }
    }
    let raw = crate::auth::random_secret(24);
    let saved = entity::appointment::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        property_id: Set(o.property_id),
        unit_id: Set(o.unit_id),
        kind: Set(o.kind),
        subject_type: Set(o.subject_type),
        subject_id: Set(o.subject_id),
        title: Set(o.title),
        status: Set("proposed".into()),
        windows: Set(json!(o.windows)),
        starts_at: Set(None),
        ends_at: Set(None),
        with_name: Set(o.with_name),
        with_email: Set(o.with_email.map(|e| e.trim().to_lowercase())),
        with_phone: Set(o.with_phone),
        with_role: Set(o.with_role),
        assignee_user_id: Set(o.assignee_user_id),
        vendor_entity_id: Set(o.vendor_entity_id),
        note: Set(o.note),
        access_notes: Set(o.access_notes),
        token_hash: Set(Some(crate::auth::hash_secret(&raw))),
        confirmed_by: Set(None),
        confirmed_at: Set(None),
        proposed_start: Set(None),
        proposed_end: Set(None),
        reminded: Set(json!([])),
        outcome_note: Set(None),
        created_by: Set(o.created_by),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    notify_offer(db, tenant_id, &saved, &raw).await;
    if saved.subject_type == "ticket" {
        if let Some(tid) = saved.subject_id {
            let words = offered_words(db, tenant_id, &saved).await;
            note_on_ticket(
                db,
                tenant_id,
                tid,
                "appointment_offered",
                &format!("Offered times: {words}"),
                "public",
            )
            .await;
        }
    }
    Ok((saved, raw))
}

async fn offered_words(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    a: &entity::appointment::Model,
) -> String {
    let tz = tz_for(db, tenant_id).await;
    windows_of(&a.windows)
        .iter()
        .map(|w| window_words(w, &tz))
        .collect::<Vec<_>>()
        .join("; ")
}

async fn person_vars(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    a: &entity::appointment::Model,
) -> Value {
    let property = Property::find_by_id(a.property_id)
        .one(db)
        .await
        .ok()
        .flatten();
    let company = Tenant::find_by_id(tenant_id)
        .one(db)
        .await
        .ok()
        .flatten()
        .map(|t| t.name)
        .unwrap_or_default();
    json!({
        "title": a.title,
        "property": property.as_ref().map(|p| format!("{}, {}", p.address, p.city)).unwrap_or_default(),
        "company": company,
        "name": a.with_name.clone().unwrap_or_else(|| "there".into()),
    })
}

async fn notify_offer(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    a: &entity::appointment::Model,
    raw: &str,
) {
    let mut vars = person_vars(db, tenant_id, a).await;
    vars["windows"] = json!(offered_words(db, tenant_id, a).await);
    vars["link"] = json!(book_url(raw));
    send_both(db, tenant_id, a, "appointment_offered", vars).await;
}

/// Email, and a text when there's a number, to the person the visit is with.
async fn send_both(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    a: &entity::appointment::Model,
    template: &str,
    vars: Value,
) {
    let trigger = format!("{template}:{}:{}", a.id, a.updated_at.timestamp());
    if let Some(email) = a.with_email.as_deref().filter(|e| !e.is_empty()) {
        crate::notify::notify_person(
            db,
            tenant_id,
            email,
            template,
            vars.clone(),
            Some(("appointment", a.id)),
            &trigger,
        )
        .await;
    }
    if let Some(phone) = a.with_phone.as_deref().filter(|p| !p.trim().is_empty()) {
        let sms = json!({
            "template": template,
            "to": phone,
            "owner_type": "appointment",
            "owner_id": a.id,
            "trigger": format!("{trigger}:sms"),
            "vars": vars,
        });
        if let Err(e) = crate::scheduler::enqueue(db, tenant_id, "auto_sms", sms, 0).await {
            tracing::error!("appointment sms: {e}");
        }
    }
}

/// A line on the work order's timeline.
pub async fn note_on_ticket(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    ticket_id: Uuid,
    action: &str,
    body: &str,
    visibility: &str,
) {
    let c = entity::ticket_comment::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        ticket_id: Set(ticket_id),
        author_user_id: Set(None),
        kind: Set("action".into()),
        visibility: Set(visibility.into()),
        author_name: Set(Some("Scheduling".into())),
        body: Set(body.to_string()),
        document_ids: Set(json!([])),
        action: Set(Some(action.into())),
        created_at: Set(Utc::now().into()),
    };
    if let Err(e) = c.insert(db).await {
        tracing::error!("appointment note: {e}");
    }
}

/// Who confirmed: `resident`, `prospect`, `vendor` (from a link or the
/// portal) or `staff`.
pub async fn confirm(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    a: entity::appointment::Model,
    w: Window,
    by: &str,
) -> ApiResult<entity::appointment::Model> {
    if !matches!(a.status.as_str(), "proposed" | "confirmed") {
        return Err(ApiError::Conflict(format!(
            "this appointment is {}",
            a.status
        )));
    }
    let now = Utc::now();
    let mut am: entity::appointment::ActiveModel = a.clone().into();
    am.status = Set("confirmed".into());
    am.starts_at = Set(Some(w.start.into()));
    am.ends_at = Set(Some(w.end.into()));
    am.confirmed_by = Set(Some(by.into()));
    am.confirmed_at = Set(Some(now.into()));
    am.proposed_start = Set(None);
    am.proposed_end = Set(None);
    am.reminded = Set(json!([]));
    am.updated_at = Set(now.into());
    let saved = am.update(db).await?;
    let tz = tz_for(db, tenant_id).await;
    let words = window_words(&w, &tz);
    // The work order follows: scheduled, with the day as its due date.
    if saved.subject_type == "ticket" {
        if let Some(tid) = saved.subject_id {
            if let Some(t) = MaintenanceTicket::find_by_id(tid).one(db).await? {
                if matches!(
                    t.status.as_str(),
                    "open" | "triage" | "on_hold" | "scheduled"
                ) {
                    let mut tm: entity::maintenance_ticket::ActiveModel = t.into();
                    tm.status = Set("scheduled".into());
                    tm.due_date = Set(Some(w.start.with_timezone(&tz).date_naive().to_string()));
                    tm.waiting_on = Set(None);
                    tm.updated_at = Set(now.into());
                    tm.update(db).await?;
                }
            }
            note_on_ticket(
                db,
                tenant_id,
                tid,
                "appointment_confirmed",
                &format!("Scheduled: {words}"),
                "public",
            )
            .await;
        }
    }
    let mut vars = person_vars(db, tenant_id, &saved).await;
    vars["when"] = json!(words);
    if by != "staff" {
        // Staff hear that the other side picked.
        if let Some(uid) = saved.assignee_user_id {
            if let Some(u) = User::find_by_id(uid).one(db).await? {
                crate::notify::in_app(
                    db,
                    tenant_id,
                    &u,
                    "appointment_confirmed_staff",
                    &vars,
                    Some(("appointment", saved.id)),
                    &format!("appt_confirmed_staff:{}:{}", saved.id, now.timestamp()),
                )
                .await;
            }
        }
    }
    send_both(db, tenant_id, &saved, "appointment_confirmed", vars).await;
    Ok(saved)
}

/// The other side can't make any of the windows: they ask for another time,
/// or just say no. Staff hear about it.
pub async fn decline(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    a: entity::appointment::Model,
    proposed: Option<Window>,
    reason: Option<String>,
) -> ApiResult<entity::appointment::Model> {
    if !matches!(a.status.as_str(), "proposed" | "confirmed") {
        return Err(ApiError::Conflict(format!(
            "this appointment is {}",
            a.status
        )));
    }
    let now = Utc::now();
    let mut am: entity::appointment::ActiveModel = a.clone().into();
    am.status = Set("declined".into());
    am.proposed_start = Set(proposed.as_ref().map(|w| w.start.into()));
    am.proposed_end = Set(proposed.as_ref().map(|w| w.end.into()));
    am.outcome_note = Set(reason.clone());
    am.updated_at = Set(now.into());
    let saved = am.update(db).await?;
    let tz = tz_for(db, tenant_id).await;
    let asked = proposed
        .as_ref()
        .map(|w| format!(" They asked for {}.", window_words(w, &tz)))
        .unwrap_or_default();
    if saved.subject_type == "ticket" {
        if let Some(tid) = saved.subject_id {
            note_on_ticket(
                db,
                tenant_id,
                tid,
                "appointment_declined",
                &format!(
                    "None of the offered times work.{asked}{}",
                    reason
                        .as_deref()
                        .map(|r| format!(" \"{r}\""))
                        .unwrap_or_default()
                ),
                "internal",
            )
            .await;
        }
    }
    let mut vars = person_vars(db, tenant_id, &saved).await;
    vars["asked"] = json!(asked.trim());
    vars["reason"] = json!(reason.unwrap_or_default());
    crate::notify::notify_staff(
        db,
        tenant_id,
        "maintenance:manage",
        "appointment_declined",
        vars,
        Some(("appointment", saved.id)),
        &format!("appt_declined:{}:{}", saved.id, now.timestamp()),
        None,
    )
    .await;
    Ok(saved)
}

/// The visit by its link token.
pub async fn by_token(
    db: &impl ConnectionTrait,
    token: &str,
) -> ApiResult<entity::appointment::Model> {
    let token = token.trim();
    if token.len() < 16 {
        return Err(ApiError::NotFound("that link isn't valid".into()));
    }
    Appointment::find()
        .filter(entity::appointment::Column::TokenHash.eq(crate::auth::hash_secret(token)))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("that link isn't valid".into()))
}

/// Who a work order's visit is with: the lease on it, or the property's
/// active lease on that unit.
pub async fn resident_for_ticket(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    t: &entity::maintenance_ticket::Model,
) -> Option<entity::lease::Model> {
    if let Some(lid) = t.lease_id {
        if let Ok(Some(l)) = Lease::find_by_id(lid).one(db).await {
            return Some(l);
        }
    }
    let mut q = Lease::find()
        .filter(entity::lease::Column::TenantId.eq(tenant_id))
        .filter(entity::lease::Column::PropertyId.eq(t.property_id))
        .filter(entity::lease::Column::Status.eq("active"));
    if let Some(u) = t.unit_id {
        q = q.filter(entity::lease::Column::UnitId.eq(u));
    }
    q.order_by_desc(entity::lease::Column::StartDate)
        .one(db)
        .await
        .ok()
        .flatten()
}

// ---------------------------------------------------------------------------
// Reminders
// ---------------------------------------------------------------------------

pub async fn ensure_recurring_jobs(db: &DatabaseConnection) {
    let tenants = match Tenant::find().all(db).await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("appointments: list tenants: {e}");
            return;
        }
    };
    for t in tenants {
        if let Err(e) = ensure_job_for_tenant(db, t.id).await {
            tracing::error!("appointments: ensure job for {}: {e}", t.id);
        }
    }
}

pub async fn ensure_job_for_tenant(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
) -> Result<(), sea_orm::DbErr> {
    let open = entity::prelude::BackgroundJob::find()
        .filter(entity::background_job::Column::TenantId.eq(tenant_id))
        .filter(entity::background_job::Column::Kind.eq(REMINDER_KIND))
        .filter(entity::background_job::Column::Status.is_in(["pending", "running"]))
        .one(db)
        .await?;
    if open.is_none() {
        crate::scheduler::enqueue(db, tenant_id, REMINDER_KIND, json!({}), 20).await?;
    }
    Ok(())
}

/// Send the reminders that are due, then sleep.
pub async fn handle_reminder_job(
    db: &DatabaseConnection,
    job: &entity::background_job::Model,
) -> JobOutcome {
    let tenant_id = job.tenant_id;
    let sent = match remind_due(db, tenant_id, Utc::now()).await {
        Ok(n) => n,
        Err(e) => {
            tracing::error!("appointment reminders: {e}");
            0
        }
    };
    let mut out = JobOutcome::reschedule("pending", REMINDER_INTERVAL_SECS);
    out.result = Some(json!({ "sent": sent }));
    out
}

pub async fn remind_due(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    now: DateTime<Utc>,
) -> ApiResult<u32> {
    let leads = parse_hours(
        &crate::settings::get_string(db, tenant_id, crate::settings::APPOINTMENT_REMINDER_HOURS)
            .await,
    );
    let horizon = now + Duration::hours(leads.iter().copied().max().unwrap_or(24) + 1);
    let rows = Appointment::find()
        .filter(entity::appointment::Column::TenantId.eq(tenant_id))
        .filter(entity::appointment::Column::Status.eq("confirmed"))
        .filter(entity::appointment::Column::StartsAt.gt(now))
        .filter(entity::appointment::Column::StartsAt.lte(horizon))
        .all(db)
        .await?;
    let tz = tz_for(db, tenant_id).await;
    let mut sent = 0;
    for a in rows {
        let Some(starts) = a.starts_at else { continue };
        let starts: DateTime<Utc> = starts.to_utc();
        let already = ids_of(&a.reminded);
        let due = reminders_due(&leads, &already, starts, now);
        if due.is_empty() {
            continue;
        }
        let w = Window {
            start: starts,
            end: a
                .ends_at
                .map(|e| e.to_utc())
                .unwrap_or(starts + Duration::hours(2)),
        };
        let mut vars = person_vars(db, tenant_id, &a).await;
        vars["when"] = json!(window_words(&w, &tz));
        let hours = due.iter().copied().min().unwrap_or(0);
        vars["in"] = json!(if hours >= 24 {
            format!(
                "in {} day{}",
                hours / 24,
                if hours >= 48 { "s" } else { "" }
            )
        } else {
            format!("in {hours} hour{}", if hours == 1 { "" } else { "s" })
        });
        send_both(db, tenant_id, &a, "appointment_reminder", vars.clone()).await;
        if let Some(uid) = a.assignee_user_id {
            if let Some(u) = User::find_by_id(uid).one(db).await? {
                crate::notify::in_app(
                    db,
                    tenant_id,
                    &u,
                    "appointment_reminder_staff",
                    &vars,
                    Some(("appointment", a.id)),
                    &format!("appt_reminder_staff:{}:{hours}", a.id),
                )
                .await;
            }
        }
        let mut all = already;
        all.extend(due);
        let mut am: entity::appointment::ActiveModel = a.into();
        am.reminded = Set(json!(all));
        am.update(db).await?;
        sent += 1;
    }
    Ok(sent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn t(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().to_utc()
    }

    #[test]
    fn windows_are_checked_and_sorted() {
        let now = t("2026-10-05T12:00:00Z");
        let ok = clean_windows(
            vec![
                Window {
                    start: t("2026-10-07T16:00:00Z"),
                    end: t("2026-10-07T18:00:00Z"),
                },
                Window {
                    start: t("2026-10-06T16:00:00Z"),
                    end: t("2026-10-06T18:00:00Z"),
                },
            ],
            now,
        )
        .unwrap();
        assert_eq!(ok[0].start, t("2026-10-06T16:00:00Z"));
        assert!(clean_windows(vec![], now).is_err());
        assert!(clean_windows(
            vec![Window {
                start: t("2026-10-06T16:00:00Z"),
                end: t("2026-10-06T16:10:00Z")
            }],
            now
        )
        .is_err());
        assert!(
            clean_windows(
                vec![Window {
                    start: t("2026-10-01T16:00:00Z"),
                    end: t("2026-10-01T18:00:00Z")
                }],
                now
            )
            .is_err(),
            "in the past"
        );
        assert!(
            clean_windows(
                vec![
                    Window {
                        start: t("2026-10-06T16:00:00Z"),
                        end: t("2026-10-06T18:00:00Z")
                    },
                    Window {
                        start: t("2026-10-06T17:00:00Z"),
                        end: t("2026-10-06T19:00:00Z")
                    },
                ],
                now
            )
            .is_err(),
            "overlap"
        );
    }

    #[test]
    fn a_window_reads_in_local_time() {
        let tz: chrono_tz::Tz = "America/Los_Angeles".parse().unwrap();
        let w = Window {
            start: t("2026-10-07T16:00:00Z"),
            end: t("2026-10-07T18:30:00Z"),
        };
        assert_eq!(window_words(&w, &tz), "Wed, Oct 7, 9 AM to 11:30 AM");
        let _ = tz.with_ymd_and_hms(2026, 10, 7, 9, 0, 0);
    }

    #[test]
    fn reminders_fire_once_per_lead_and_never_after() {
        let starts = t("2026-10-07T16:00:00Z");
        let leads = [24, 2];
        assert_eq!(
            reminders_due(&leads, &[], t("2026-10-05T16:00:00Z"), starts),
            Vec::<i64>::new()
        );
        assert_eq!(
            reminders_due(&leads, &[], starts, t("2026-10-06T16:30:00Z")),
            vec![24]
        );
        assert_eq!(
            reminders_due(&leads, &[24], starts, t("2026-10-07T14:30:00Z")),
            vec![2]
        );
        assert_eq!(
            reminders_due(&leads, &[24, 2], starts, t("2026-10-07T15:00:00Z")),
            Vec::<i64>::new()
        );
        // Started late: both at once, sent as one.
        assert_eq!(
            reminders_due(&leads, &[], starts, t("2026-10-07T15:00:00Z")),
            vec![24, 2]
        );
        assert!(reminders_due(&leads, &[], starts, t("2026-10-07T17:00:00Z")).is_empty());
    }

    #[test]
    fn hours_setting_is_forgiving() {
        assert_eq!(parse_hours("24, 2"), vec![24, 2]);
        assert_eq!(parse_hours("2,48,2,junk"), vec![48, 2]);
        assert_eq!(parse_hours(""), vec![24, 2]);
    }
}
