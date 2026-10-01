//! **Reminders that run themselves** (roadmap area 10, fixes F4–F9). One
//! daily-ish `resident_reminders` job per workspace sends:
//!
//! * residents: rent due (N days before, skipped on autopay), rent past due
//!   (the day after, while unpaid), and inspection reminders with a calendar
//!   file;
//! * staff: lease expiry at 90/60/30 days (and, at the first, a renewal
//!   proposal at the current rent for a manager to review and send), and
//!   warranties about to end;
//!
//! and a `manager_digest` job sends the morning summary. Every notice claims a
//! key in the notice log first ([`crate::notices`]), so a job that runs every
//! few hours sends each one once. Each rule has its own setting.

use crate::modules::JobOutcome;
use crate::notices;
use crate::settings as cfg;
use chrono::{Datelike, Duration, NaiveDate, Timelike, Utc};
use entity::prelude::{
    Asset, BackgroundJob, Inspection, Lease, LeaseCharge, LeasePayment, LeaseRenewal,
    MaintenanceTicket, PaymentMethod, Process, Property, Tenant, TourRequest, Unit,
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
};
use serde_json::{json, Value};
use uuid::Uuid;

pub const KIND: &str = "resident_reminders";
pub const DIGEST_KIND: &str = "manager_digest";
/// How often the reminder job wakes. Rules are date-based and deduplicated,
/// so waking more often only makes notices land earlier in the day.
const INTERVAL_SECS: i64 = 4 * 3600;

// ---------------------------------------------------------------------------
// Pure helpers (unit-tested)
// ---------------------------------------------------------------------------

/// The next rent day on or after `today` for a workspace whose rent is due on
/// `due_day` (clamped 1–28).
pub fn next_due_date(today: NaiveDate, due_day: i64) -> NaiveDate {
    let this = crate::billing::due_date_for_month(today.year(), today.month(), due_day);
    if today <= this {
        return this;
    }
    let (y, m) = if today.month() == 12 {
        (today.year() + 1, 1)
    } else {
        (today.year(), today.month() + 1)
    };
    crate::billing::due_date_for_month(y, m, due_day)
}

/// Which of the lead times (days before) have been reached `days_left` before
/// the date. A late start reaches several at once; the caller claims them all
/// and notifies once.
pub fn reached(leads: &[i64], days_left: i64) -> Vec<i64> {
    if days_left < 0 {
        return vec![];
    }
    leads.iter().copied().filter(|l| days_left <= *l).collect()
}

/// "today", "tomorrow" or "in N days".
pub fn when_words(days: i64) -> String {
    match days {
        0 => "today".into(),
        1 => "tomorrow".into(),
        n => format!("in {n} days"),
    }
}

/// Escape a text value for iCalendar (RFC 5545 §3.3.11).
fn ics_text(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(';', r"\;")
        .replace(',', "\\,")
        .replace("\r\n", "\\n")
        .replace('\n', "\\n")
}

/// Fold a content line at 75 octets (RFC 5545 §3.1), on character boundaries.
fn ics_fold(line: &str) -> String {
    let mut out = String::new();
    let mut count = 0;
    for ch in line.chars() {
        let len = ch.len_utf8();
        if count + len > 75 {
            out.push_str("\r\n ");
            count = 1;
        }
        out.push(ch);
        count += len;
    }
    out
}

/// One all-day calendar event as an `.ics` file.
pub fn ics_event(
    uid: &str,
    date: NaiveDate,
    summary: &str,
    location: &str,
    description: &str,
) -> String {
    let stamp = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let next = date + Duration::days(1);
    let lines = [
        "BEGIN:VCALENDAR".to_string(),
        "VERSION:2.0".to_string(),
        "PRODID:-//Vantedge//Reminders//EN".to_string(),
        "CALSCALE:GREGORIAN".to_string(),
        "METHOD:PUBLISH".to_string(),
        "BEGIN:VEVENT".to_string(),
        format!("UID:{}", ics_text(uid)),
        format!("DTSTAMP:{stamp}"),
        format!("DTSTART;VALUE=DATE:{}", date.format("%Y%m%d")),
        format!("DTEND;VALUE=DATE:{}", next.format("%Y%m%d")),
        format!("SUMMARY:{}", ics_text(summary)),
        format!("LOCATION:{}", ics_text(location)),
        format!("DESCRIPTION:{}", ics_text(description)),
        "END:VEVENT".to_string(),
        "END:VCALENDAR".to_string(),
    ];
    let mut out = String::new();
    for l in lines {
        out.push_str(&ics_fold(&l));
        out.push_str("\r\n");
    }
    out
}

fn web_url() -> String {
    std::env::var("PUBLIC_WEB_URL")
        .ok()
        .map(|s| s.trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "http://localhost:3000".into())
}

fn api_url() -> String {
    crate::partner::public_api_url()
}

/// The signature on an inspection's calendar link, so the button in an email
/// works without signing in and nobody can guess another inspection's link.
pub fn calendar_sig(inspection_id: Uuid) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let key = crate::config::Config::global().jwt_secret.clone();
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key.as_bytes()).expect("any key length");
    mac.update(format!("ics:{inspection_id}").as_bytes());
    hex_of(&mac.finalize().into_bytes())[..32].to_string()
}

/// Constant-time check of a calendar link signature.
pub fn calendar_sig_ok(inspection_id: Uuid, sig: &str) -> bool {
    let want = calendar_sig(inspection_id);
    want.len() == sig.len()
        && want
            .bytes()
            .zip(sig.bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

fn hex_of(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The `.ics` for an inspection.
pub async fn inspection_ics(
    db: &impl ConnectionTrait,
    i: &entity::inspection::Model,
) -> Option<String> {
    let date = NaiveDate::parse_from_str(i.scheduled_date.as_deref()?, "%Y-%m-%d").ok()?;
    let place = place_of(db, i.property_id, i.unit_id).await;
    let kind = i.kind.replace('_', "-");
    Some(ics_event(
        &format!("inspection-{}@vantedge", i.id),
        date,
        &format!("{} inspection", capitalize(&kind)),
        &place,
        "Scheduled through your property manager. Reply to their email or message them in your portal to change the time.",
    ))
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

// ---------------------------------------------------------------------------
// Recurring jobs
// ---------------------------------------------------------------------------

/// Ensure every workspace has one live reminder job and one live digest job.
pub async fn ensure_recurring_jobs(db: &DatabaseConnection) {
    let Ok(tenants) = Tenant::find().all(db).await else {
        return;
    };
    for t in tenants {
        for kind in [KIND, DIGEST_KIND] {
            if let Err(e) = ensure_job(db, t.id, kind).await {
                tracing::error!("reminders: ensure {kind} for {} failed: {e}", t.id);
            }
        }
    }
}

pub async fn ensure_job(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    kind: &str,
) -> Result<(), sea_orm::DbErr> {
    let live = BackgroundJob::find()
        .filter(entity::background_job::Column::TenantId.eq(tenant_id))
        .filter(entity::background_job::Column::Kind.eq(kind))
        .filter(entity::background_job::Column::Status.is_in(["pending", "running"]))
        .count(db)
        .await?;
    if live == 0 {
        crate::scheduler::enqueue(db, tenant_id, kind, json!({}), 60).await?;
    }
    Ok(())
}

/// Advance one `resident_reminders` job.
pub async fn handle_job(
    db: &DatabaseConnection,
    job: &entity::background_job::Model,
) -> JobOutcome {
    let tenant_id = job.tenant_id;
    let mut summary = json!({});
    if cfg::get_bool(db, tenant_id, cfg::REMINDERS_ENABLED).await {
        let today = Utc::now().date_naive();
        macro_rules! step {
            ($name:literal, $f:expr) => {
                match $f.await {
                    Ok(n) => summary[$name] = json!(n),
                    Err(e) => {
                        tracing::error!("reminders: {} failed: {e}", $name);
                        summary[$name] = json!(format!("error: {e}"));
                    }
                }
            };
        }
        step!("rent_due", rent_due(db, tenant_id, today));
        step!("rent_past_due", rent_past_due(db, tenant_id, today));
        step!("lease_expiry", lease_expiry(db, tenant_id, today));
        step!("inspections", inspections(db, tenant_id, today));
        step!("warranties", warranties(db, tenant_id, today));
        step!("vendor_insurance", vendor_insurance(db, tenant_id, today));
    } else {
        summary["skipped"] = json!("automatic notices are off");
    }
    let mut out = JobOutcome::reschedule("pending", INTERVAL_SECS);
    out.result = Some(summary);
    out
}

// ---------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------

async fn place_of(db: &impl ConnectionTrait, property_id: Uuid, unit_id: Option<Uuid>) -> String {
    let p = Property::find_by_id(property_id)
        .one(db)
        .await
        .ok()
        .flatten()
        .map(|p| p.name)
        .unwrap_or_default();
    let u = match unit_id {
        Some(id) => Unit::find_by_id(id)
            .one(db)
            .await
            .ok()
            .flatten()
            .map(|u| u.unit_number),
        None => None,
    };
    match u {
        Some(u) => format!("{p}, unit {u}"),
        None => p,
    }
}

fn email_of(lease: &entity::lease::Model) -> Option<&str> {
    lease
        .tenant_email
        .as_deref()
        .map(str::trim)
        .filter(|e| !e.is_empty())
}

/// F4: rent due in N days, for leases not on autopay.
pub(crate) async fn rent_due(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    today: NaiveDate,
) -> Result<u32, sea_orm::DbErr> {
    let days = cfg::get_i64(db, tenant_id, cfg::REMINDERS_RENT_DUE_DAYS).await;
    if days <= 0 {
        return Ok(0);
    }
    let due_day = cfg::get_i64(db, tenant_id, cfg::PAYMENTS_RENT_DUE_DAY)
        .await
        .clamp(1, 28);
    let due = next_due_date(today, due_day);
    let left = (due - today).num_days();
    if left <= 0 || left > days {
        return Ok(0);
    }
    let leases = Lease::find()
        .filter(entity::lease::Column::TenantId.eq(tenant_id))
        .filter(entity::lease::Column::Status.eq("active"))
        .all(db)
        .await?;
    let autopay: Vec<Uuid> = PaymentMethod::find()
        .filter(entity::payment_method::Column::TenantId.eq(tenant_id))
        .filter(entity::payment_method::Column::Autopay.eq(true))
        .filter(entity::payment_method::Column::Status.eq("active"))
        .all(db)
        .await?
        .into_iter()
        .filter_map(|m| m.lease_id)
        .collect();
    let mut sent = 0;
    for lease in leases {
        let Some(email) = email_of(&lease) else {
            continue;
        };
        if autopay.contains(&lease.id) || lease.start_date.as_str() > due.to_string().as_str() {
            continue;
        }
        let charges = LeaseCharge::find()
            .filter(entity::lease_charge::Column::TenantId.eq(tenant_id))
            .filter(entity::lease_charge::Column::LeaseId.eq(lease.id))
            .all(db)
            .await?;
        let amount = crate::billing::monthly_amount(lease.rent_cents, &charges);
        if amount <= 0 {
            continue;
        }
        if !notices::claim(db, tenant_id, &format!("rent_due:{}:{due}", lease.id)).await? {
            continue;
        }
        crate::notify::notify_person(
            db,
            tenant_id,
            email,
            "rent_due",
            json!({
                "amount": crate::dto::usd(amount),
                "due_date": due.format("%B %-d").to_string(),
                "pay_url": format!("{}/account/payments", web_url()),
            }),
            Some(("lease", lease.id)),
            &format!("rent_due:{due}"),
        )
        .await;
        sent += 1;
    }
    Ok(sent)
}

/// F4: rent still unpaid the day after it was due.
pub(crate) async fn rent_past_due(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    today: NaiveDate,
) -> Result<u32, sea_orm::DbErr> {
    if !cfg::get_bool(db, tenant_id, cfg::REMINDERS_RENT_PAST_DUE).await {
        return Ok(0);
    }
    let open = LeasePayment::find()
        .filter(entity::lease_payment::Column::TenantId.eq(tenant_id))
        .filter(entity::lease_payment::Column::Kind.eq(crate::payments::KIND_RENT))
        .filter(entity::lease_payment::Column::Status.is_in(["due", "late", "failed"]))
        .all(db)
        .await?;
    let mut sent = 0;
    for p in open {
        let Ok(due) = NaiveDate::parse_from_str(&p.due_date, "%Y-%m-%d") else {
            continue;
        };
        let late = (today - due).num_days();
        // Only around the day after: an old receivable is the office's to chase.
        if !(1..=7).contains(&late) {
            continue;
        }
        let Some(lease) = Lease::find_by_id(p.lease_id)
            .filter(entity::lease::Column::TenantId.eq(tenant_id))
            .one(db)
            .await?
        else {
            continue;
        };
        let Some(email) = email_of(&lease) else {
            continue;
        };
        if !notices::claim(db, tenant_id, &format!("rent_past_due:{}", p.id)).await? {
            continue;
        }
        crate::notify::notify_person(
            db,
            tenant_id,
            email,
            "rent_past_due",
            json!({
                "amount": crate::dto::usd(p.amount_cents),
                "due_date": due.format("%B %-d").to_string(),
                "pay_url": format!("{}/account/payments", web_url()),
            }),
            Some(("lease_payment", p.id)),
            "rent_past_due",
        )
        .await;
        sent += 1;
    }
    Ok(sent)
}

/// F6: staff hear before a lease ends; at the first notice a renewal is
/// proposed at the current rent for a manager to review (never sent from here).
pub(crate) async fn lease_expiry(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    today: NaiveDate,
) -> Result<u32, sea_orm::DbErr> {
    let leads = crate::reminders::parse_lead_days(
        &cfg::get_string(db, tenant_id, cfg::REMINDERS_LEASE_EXPIRY_DAYS).await,
    );
    let draft = cfg::get_bool(db, tenant_id, cfg::REMINDERS_DRAFT_RENEWAL).await;
    let max = leads.iter().copied().max().unwrap_or(0);
    let leases = Lease::find()
        .filter(entity::lease::Column::TenantId.eq(tenant_id))
        .filter(entity::lease::Column::Status.eq("active"))
        .all(db)
        .await?;
    let mut sent = 0;
    for lease in leases {
        let Some(end) = lease
            .end_date
            .as_deref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        else {
            continue;
        };
        let left = (end - today).num_days();
        if left < 0 || left > max {
            continue;
        }
        let hit = reached(&leads, left);
        let mut fresh = false;
        for lead in &hit {
            if notices::claim(
                db,
                tenant_id,
                &format!("lease_expiry:{}:{end}:{lead}", lease.id),
            )
            .await?
            {
                fresh = true;
            }
        }
        if !fresh {
            continue;
        }
        let mut note = String::new();
        if draft {
            let open = LeaseRenewal::find()
                .filter(entity::lease_renewal::Column::TenantId.eq(tenant_id))
                .filter(entity::lease_renewal::Column::LeaseId.eq(lease.id))
                .filter(
                    entity::lease_renewal::Column::Status
                        .is_in(crate::renewals::OPEN_STATUSES.to_vec()),
                )
                .count(db)
                .await?;
            if open == 0 {
                let start = crate::renewals::day_after(&end.to_string()).unwrap_or_default();
                let new_end = crate::renewals::add_months_str(&start, 12);
                match crate::renewals::create_proposal(
                    db,
                    tenant_id,
                    &lease,
                    crate::renewals::Terms {
                        new_rent_cents: lease.rent_cents,
                        new_start_date: start,
                        new_end_date: new_end,
                        term_months: Some(12),
                        notes: Some(format!(
                            "Drafted automatically {left} days before the lease ends, at the \
                             current rent. Review the rent and term before sending."
                        )),
                    },
                    None,
                )
                .await
                {
                    Ok((r, _, _)) => {
                        note = "A renewal at the current rent has been drafted for you to review and send.".into();
                        crate::audit::record(
                            db,
                            None,
                            crate::audit::actions::LEASE_RENEWAL_PROPOSE,
                            Some("lease_renewal"),
                            Some(r.id.to_string()),
                            Some(tenant_id),
                            Some(json!({ "lease_id": lease.id, "automatic": true, "days_left": left })),
                        )
                        .await;
                    }
                    Err(e) => tracing::warn!("reminders: renewal draft failed: {e}"),
                }
            } else {
                note = "A renewal is already in progress.".into();
            }
        }
        let place = place_of(db, lease.property_id, lease.unit_id).await;
        crate::notify::notify_staff(
            db,
            tenant_id,
            "lease:manage",
            "lease_expiring",
            json!({
                "tenant": lease.tenant_name,
                "place": place,
                "end_date": end.format("%B %-d, %Y").to_string(),
                "days": left,
                "renewal_note": note,
            }),
            Some(("lease", lease.id)),
            &format!("lease_expiring:{left}"),
            None,
        )
        .await;
        sent += 1;
    }
    Ok(sent)
}

/// F7: the resident hears before an inspection, with a calendar file.
pub(crate) async fn inspections(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    today: NaiveDate,
) -> Result<u32, sea_orm::DbErr> {
    let leads = crate::reminders::parse_lead_days(
        &cfg::get_string(db, tenant_id, cfg::REMINDERS_INSPECTION_DAYS).await,
    );
    let max = leads.iter().copied().max().unwrap_or(0);
    let rows = Inspection::find()
        .filter(entity::inspection::Column::TenantId.eq(tenant_id))
        .filter(entity::inspection::Column::Status.eq("draft"))
        .filter(entity::inspection::Column::ScheduledDate.is_not_null())
        .all(db)
        .await?;
    let mut sent = 0;
    for i in rows {
        let Some(date) = i
            .scheduled_date
            .as_deref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        else {
            continue;
        };
        let left = (date - today).num_days();
        if left < 0 || left > max {
            continue;
        }
        let hit = reached(&leads, left);
        let mut fresh = false;
        for lead in &hit {
            if notices::claim(db, tenant_id, &format!("inspection:{}:{date}:{lead}", i.id)).await? {
                fresh = true;
            }
        }
        if !fresh {
            continue;
        }
        let Some(lease) = Lease::find_by_id(i.lease_id)
            .filter(entity::lease::Column::TenantId.eq(tenant_id))
            .one(db)
            .await?
        else {
            continue;
        };
        let Some(email) = email_of(&lease) else {
            continue;
        };
        let place = place_of(db, i.property_id, i.unit_id).await;
        crate::notify::notify_person(
            db,
            tenant_id,
            email,
            "inspection_reminder",
            json!({
                "kind": i.kind.replace('_', "-"),
                "when": when_words(left),
                "place": place,
                "date": date.format("%A, %B %-d").to_string(),
                "calendar_url": format!(
                    "{}/public/inspections/{}/calendar.ics?sig={}",
                    api_url(),
                    i.id,
                    calendar_sig(i.id)
                ),
            }),
            Some(("inspection", i.id)),
            &format!("inspection_reminder:{left}"),
        )
        .await;
        sent += 1;
    }
    Ok(sent)
}

/// F8: staff hear before an appliance's warranty ends.
pub(crate) async fn warranties(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    today: NaiveDate,
) -> Result<u32, sea_orm::DbErr> {
    let days = cfg::get_i64(db, tenant_id, cfg::REMINDERS_WARRANTY_DAYS).await;
    if days <= 0 {
        return Ok(0);
    }
    let assets = Asset::find()
        .filter(entity::asset::Column::TenantId.eq(tenant_id))
        .filter(entity::asset::Column::Status.eq("active"))
        .filter(entity::asset::Column::WarrantyExpires.is_not_null())
        .all(db)
        .await?;
    let mut sent = 0;
    for a in assets {
        let Some(end) = a
            .warranty_expires
            .as_deref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        else {
            continue;
        };
        let left = (end - today).num_days();
        if left < 0 || left > days {
            continue;
        }
        if !notices::claim(db, tenant_id, &format!("warranty:{}:{end}", a.id)).await? {
            continue;
        }
        let place = place_of(db, a.property_id, a.unit_id).await;
        crate::notify::notify_staff(
            db,
            tenant_id,
            "maintenance:read",
            "warranty_expiring",
            json!({
                "asset": a.name,
                "place": place,
                "date": end.format("%B %-d, %Y").to_string(),
            }),
            Some(("asset", a.id)),
            "warranty_expiring",
            None,
        )
        .await;
        sent += 1;
    }
    Ok(sent)
}

/// Which reminder a policy ending in `left` days is due for: 30 days out,
/// then 7. `None` once it has lapsed or while it is further off.
pub fn coi_lead(left: i64) -> Option<i64> {
    match left {
        0..=7 => Some(7),
        8..=30 => Some(30),
        _ => None,
    }
}

/// F12: a vendor's insurance ending in 30 and in 7 days. The vendor is asked
/// for the renewed certificate; staff are told. A policy already replaced by
/// a later one of the same kind is left alone.
pub(crate) async fn vendor_insurance(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    today: NaiveDate,
) -> Result<u32, sea_orm::DbErr> {
    let policies = entity::prelude::VendorInsurance::find()
        .filter(entity::vendor_insurance::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?;
    let mut sent = 0;
    for p in &policies {
        let Ok(end) = NaiveDate::parse_from_str(&p.expires_on, "%Y-%m-%d") else {
            continue;
        };
        let Some(lead) = coi_lead((end - today).num_days()) else {
            continue;
        };
        let replaced = policies.iter().any(|o| {
            o.id != p.id
                && o.counterparty_id == p.counterparty_id
                && o.kind == p.kind
                && o.expires_on > p.expires_on
        });
        if replaced {
            continue;
        }
        if !notices::claim(db, tenant_id, &format!("coi:{}:{lead}", p.id)).await? {
            continue;
        }
        let Some(vendor) = entity::prelude::Counterparty::find_by_id(p.counterparty_id)
            .one(db)
            .await?
        else {
            continue;
        };
        let vars = json!({
            "vendor": vendor.name,
            "kind": p.kind.replace('_', " "),
            "carrier": p.carrier,
            "date": end.format("%B %-d, %Y").to_string(),
        });
        if let Some(email) = vendor.email.as_deref().filter(|e| !e.trim().is_empty()) {
            crate::notify::notify_person(
                db,
                tenant_id,
                email,
                "coi_expiring",
                vars.clone(),
                Some(("counterparty", vendor.id)),
                &format!("coi_expiring:{lead}"),
            )
            .await;
        }
        crate::notify::notify_staff(
            db,
            tenant_id,
            "entity:read",
            "vendor_coi_expiring",
            vars,
            Some(("counterparty", vendor.id)),
            "vendor_coi_expiring",
            None,
        )
        .await;
        sent += 1;
    }
    Ok(sent)
}

// ---------------------------------------------------------------------------
// F9: the managers' morning summary
// ---------------------------------------------------------------------------

#[derive(Debug, Default, PartialEq)]
pub struct Digest {
    pub rent_late: u64,
    pub leases_ending: u64,
    pub tickets_past_sla: u64,
    pub turns_past_target: u64,
    pub new_tours: u64,
}

impl Digest {
    pub fn is_empty(&self) -> bool {
        *self == Digest::default()
    }

    /// The lines of the email, only for what has something in it.
    pub fn lines(&self) -> Vec<String> {
        let mut out = vec![];
        let mut add = |n: u64, one: &str, many: &str| {
            if n > 0 {
                out.push(format!("- {n} {}", if n == 1 { one } else { many }));
            }
        };
        add(self.rent_late, "rent payment late", "rent payments late");
        add(
            self.leases_ending,
            "lease ending in the next 30 days",
            "leases ending in the next 30 days",
        );
        add(
            self.tickets_past_sla,
            "work order past its SLA",
            "work orders past their SLA",
        );
        add(
            self.turns_past_target,
            "turn past its target date",
            "turns past their target date",
        );
        add(self.new_tours, "new tour request", "new tour requests");
        out
    }

    pub fn headline(&self) -> String {
        let n = self.rent_late
            + self.leases_ending
            + self.tickets_past_sla
            + self.turns_past_target
            + self.new_tours;
        if n == 1 {
            "1 thing needs you".into()
        } else {
            format!("{n} things need you")
        }
    }
}

pub async fn compute_digest(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    today: NaiveDate,
) -> Result<Digest, sea_orm::DbErr> {
    let now = Utc::now();
    let in30 = (today + Duration::days(30)).to_string();
    let today_s = today.to_string();
    let rent_late = LeasePayment::find()
        .filter(entity::lease_payment::Column::TenantId.eq(tenant_id))
        .filter(entity::lease_payment::Column::Kind.eq(crate::payments::KIND_RENT))
        .filter(entity::lease_payment::Column::Status.eq("late"))
        .count(db)
        .await?;
    let leases_ending = Lease::find()
        .filter(entity::lease::Column::TenantId.eq(tenant_id))
        .filter(entity::lease::Column::Status.eq("active"))
        .filter(entity::lease::Column::EndDate.gte(today_s.clone()))
        .filter(entity::lease::Column::EndDate.lte(in30))
        .count(db)
        .await?;
    let tickets_past_sla = MaintenanceTicket::find()
        .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
        .filter(
            entity::maintenance_ticket::Column::Status
                .is_in(crate::routes::maintenance::OPEN_STATUSES.to_vec()),
        )
        .filter(entity::maintenance_ticket::Column::SlaResolveDueAt.lt(now))
        .count(db)
        .await?;
    let turns_past_target = Process::find()
        .filter(entity::process::Column::TenantId.eq(tenant_id))
        .filter(entity::process::Column::Status.eq("active"))
        .filter(entity::process::Column::TargetDate.lt(today_s))
        .count(db)
        .await?;
    let new_tours = TourRequest::find()
        .filter(entity::tour_request::Column::TenantId.eq(tenant_id))
        .filter(entity::tour_request::Column::CreatedAt.gte(now - Duration::days(1)))
        .count(db)
        .await?;
    Ok(Digest {
        rent_late,
        leases_ending,
        tickets_past_sla,
        turns_past_target,
        new_tours,
    })
}

/// Advance one `manager_digest` job: send once a day at the chosen hour.
pub async fn handle_digest_job(
    db: &DatabaseConnection,
    job: &entity::background_job::Model,
) -> JobOutcome {
    let tenant_id = job.tenant_id;
    let mut result: Value = json!({});
    let now = Utc::now();
    let hour = cfg::get_i64(db, tenant_id, cfg::REMINDERS_DIGEST_HOUR_UTC)
        .await
        .clamp(0, 23) as u32;
    let on = cfg::get_bool(db, tenant_id, cfg::REMINDERS_ENABLED).await
        && cfg::get_bool(db, tenant_id, cfg::REMINDERS_MANAGER_DIGEST).await;
    if on && now.hour() >= hour {
        match send_digest(db, tenant_id, now.date_naive()).await {
            Ok(r) => result = r,
            Err(e) => result = json!({ "error": e.to_string() }),
        }
    } else {
        result["waiting"] = json!(if on { "for the hour" } else { "off" });
    }
    let mut out = JobOutcome::reschedule("pending", 3600);
    out.result = Some(result);
    out
}

pub async fn send_digest(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    today: NaiveDate,
) -> Result<Value, sea_orm::DbErr> {
    if !notices::claim(db, tenant_id, &format!("digest:{today}")).await? {
        return Ok(json!({ "sent": false, "why": "already sent today" }));
    }
    let d = compute_digest(db, tenant_id, today).await?;
    if d.is_empty() {
        return Ok(json!({ "sent": false, "why": "nothing to report" }));
    }
    let staff = crate::notify::staff_with_permission(db, tenant_id, "property:read")
        .await
        .unwrap_or_default();
    let summary = d.lines().join("\n");
    for u in &staff {
        crate::notify::notify_person(
            db,
            tenant_id,
            &u.email,
            DIGEST_KIND,
            json!({
                "headline": d.headline(),
                "summary": summary,
                "console_url": format!("{}/console", web_url()),
            }),
            None,
            &format!("digest:{today}"),
        )
        .await;
    }
    Ok(json!({ "sent": true, "to": staff.len(), "lines": d.lines() }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn next_rent_day() {
        assert_eq!(next_due_date(d("2026-10-01"), 1), d("2026-10-01"));
        assert_eq!(next_due_date(d("2026-10-02"), 1), d("2026-11-01"));
        assert_eq!(next_due_date(d("2026-12-15"), 5), d("2027-01-05"));
        assert_eq!(next_due_date(d("2026-02-10"), 28), d("2026-02-28"));
    }

    #[test]
    fn coi_reminder_leads() {
        assert_eq!(coi_lead(45), None);
        assert_eq!(coi_lead(30), Some(30));
        assert_eq!(coi_lead(8), Some(30));
        assert_eq!(coi_lead(7), Some(7));
        assert_eq!(coi_lead(0), Some(7));
        assert_eq!(coi_lead(-1), None);
    }

    #[test]
    fn lead_times_reached() {
        assert_eq!(reached(&[90, 60, 30], 95), Vec::<i64>::new());
        assert_eq!(reached(&[90, 60, 30], 85), vec![90]);
        assert_eq!(reached(&[90, 60, 30], 25), vec![90, 60, 30]);
        assert!(reached(&[90], -1).is_empty());
    }

    #[test]
    fn words() {
        assert_eq!(when_words(0), "today");
        assert_eq!(when_words(1), "tomorrow");
        assert_eq!(when_words(3), "in 3 days");
    }

    #[test]
    fn calendar_file_is_valid_and_escaped() {
        let ics = ics_event(
            "abc@vantedge",
            d("2026-10-20"),
            "Move-out inspection",
            "Maple Ct, unit 2",
            "Bring keys; meet at the office, 9am",
        );
        assert!(ics.starts_with("BEGIN:VCALENDAR\r\n"));
        assert!(ics.contains("DTSTART;VALUE=DATE:20261020\r\n"));
        assert!(ics.contains("DTEND;VALUE=DATE:20261021\r\n"));
        assert!(ics.contains("LOCATION:Maple Ct\\, unit 2"));
        assert!(ics.contains(r"Bring keys\; meet at the office\, 9am"));
        assert_eq!(ics.matches("BEGIN:VEVENT").count(), 1);
        assert!(ics.ends_with("END:VCALENDAR\r\n"));
        for line in ics.split("\r\n") {
            assert!(line.len() <= 75, "line too long: {line}");
        }
    }

    #[test]
    fn long_lines_fold() {
        let long = "x".repeat(200);
        let ics = ics_event("u", d("2026-10-20"), &long, "", "");
        assert!(ics.contains("\r\n x"));
    }

    #[test]
    fn digest_lines_only_for_what_has_something() {
        let empty = Digest::default();
        assert!(empty.is_empty());
        assert!(empty.lines().is_empty());
        let one = Digest {
            rent_late: 2,
            new_tours: 1,
            ..Default::default()
        };
        assert_eq!(
            one.lines(),
            vec!["- 2 rent payments late", "- 1 new tour request"]
        );
        assert_eq!(one.headline(), "3 things need you");
    }
}
