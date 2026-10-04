//! **System settings** — a per-tenant, code-defined configuration catalog.
//!
//! Like the RBAC and workflow catalogs, the *set* of settings is defined in code
//! ([`CATALOG`]) — each with a key, type, default, and human label/group — while
//! the *values* are stored per tenant in the `setting` table. Absence of a row
//! means "use the default", so a fresh tenant is fully configured out of the box
//! and adding a new setting never needs a data backfill.
//!
//! Handlers read settings with the typed helpers ([`get_bool`], [`get_i64`]),
//! which validate the key against the catalog and fall back to its default. The
//! `routes::settings` endpoints expose the merged catalog+values and let a tenant
//! admin (`tenant:manage`) override them.

use crate::error::{ApiError, ApiResult};
use chrono::Utc;
use entity::prelude::Setting;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set as ActiveSet,
};
use serde_json::{json, Value};
use uuid::Uuid;

// ---- Known setting keys ----------------------------------------------------

/// Allow reusing a recent application for any property in the firm.
pub const APPLICATION_REUSE_ENABLED: &str = "application_reuse.enabled";
/// How many days a prior application stays reusable.
pub const APPLICATION_REUSE_WINDOW_DAYS: &str = "application_reuse.window_days";
/// Auto-approve an application the moment its background screening clears.
pub const APPLICATION_AUTO_APPROVE: &str = "applications.auto_approve";
/// Auto-generate the lease agreement when an application converts to a lease.
pub const APPLICATION_GENERATE_DOC_ON_CONVERT: &str = "applications.generate_document_on_convert";
/// Minimum credit score for screening to clear (0 = no floor).
pub const SCREENING_MIN_CREDIT_SCORE: &str = "screening.min_credit_score";
/// Minimum monthly-income-to-rent multiple for screening to clear (0 = off).
pub const SCREENING_MIN_INCOME_RENT_RATIO: &str = "screening.min_income_rent_ratio";
/// Seconds the simulated screening provider takes to call back.
pub const SCREENING_CALLBACK_DELAY_SECS: &str = "screening.callback_delay_secs";
/// Name of the consumer-reporting agency cited on adverse-action notices.
pub const SCREENING_CRA_NAME: &str = "screening.cra_name";
/// Contact details (address/phone/email) for the CRA on adverse-action notices.
pub const SCREENING_CRA_CONTACT: &str = "screening.cra_contact";
/// Auto-send the adverse-action notice when declining a flagged applicant.
pub const SCREENING_AUTO_ADVERSE_ACTION: &str = "screening.auto_adverse_action";
/// Days a signing link stays valid after the envelope is sent (0 = no expiry).
pub const ESIGN_LINK_EXPIRY_DAYS: &str = "esign.link_expiry_days";
/// Maximum signers allowed on one envelope.
pub const ESIGN_MAX_SIGNERS: &str = "esign.max_signers";
/// Days to retain the stored signed-lease PDF (0 = keep forever).
pub const ESIGN_SIGNED_DOC_RETENTION_DAYS: &str = "esign.signed_doc_retention_days";
/// Title stamped on generated lease documents (and the envelopes sent for them).
pub const LEASE_DOC_TITLE: &str = "lease_documents.title";
pub const LEASE_RENEWAL_DOC_TITLE: &str = "lease_documents.renewal_title";
/// Let residents enroll a saved method in autopay.
pub const PAYMENTS_AUTOPAY_ENABLED: &str = "payments.autopay_enabled";
/// Day of month rent falls due (clamped 1–28).
pub const PAYMENTS_RENT_DUE_DAY: &str = "payments.rent_due_day";
/// Seconds the simulated processor takes to confirm a charge.
pub const PAYMENTS_CALLBACK_DELAY_SECS: &str = "payments.callback_delay_secs";
/// Days past the due date before a late fee applies (0 = never).
pub const LATE_FEE_GRACE_DAYS: &str = "payments.late_fee_grace_days";
/// Flat late-fee amount, in cents.
pub const LATE_FEE_FLAT_CENTS: &str = "payments.late_fee_flat_cents";
/// Percentage late fee, in basis points of the overdue amount.
pub const LATE_FEE_PERCENT_BPS: &str = "payments.late_fee_percent_bps";
/// Late-fee recurrence: `one_time` or `daily`.
pub const LATE_FEE_RECURRENCE: &str = "payments.late_fee_recurrence";
/// Cap on total late fees per billing period, in cents (0 = no cap).
pub const LATE_FEE_MAX_CENTS: &str = "payments.late_fee_max_cents";
/// Management fee withheld from owner payouts, in basis points of rent collected.
pub const PAYOUT_MGMT_FEE_BPS: &str = "payments.mgmt_fee_bps";
/// Default lead times (days before due, comma-separated) for new reminders.
pub const CALENDAR_DEFAULT_LEAD_DAYS: &str = "calendar.default_lead_days";
/// Seconds the per-tenant reminder scan sleeps between runs.
pub const CALENDAR_SCAN_INTERVAL_SECS: &str = "calendar.scan_interval_secs";
/// Auto-create a renewal reminder for every active lease with an end date.
pub const CALENDAR_LEASE_RENEWAL_SYNC: &str = "calendar.lease_renewal_sync";
/// Automatic notices to residents (rent, inspections) and staff (lease expiry, warranties).
pub const REMINDERS_ENABLED: &str = "reminders.enabled";
/// Only send a vendor out with current liability insurance (or a reason).
pub const COMPLIANCE_REQUIRE_COI: &str = "compliance.require_coi";
/// Hours before a confirmed visit to remind everyone (comma-separated).
pub const APPOINTMENT_REMINDER_HOURS: &str = "appointments.reminder_hours";
/// How long an offered window is when staff give only a start time.
pub const APPOINTMENT_WINDOW_MINUTES: &str = "appointments.window_minutes";
/// Days before a routine's due date it shows as "to schedule".
pub const HELPDESK_PLAN_LEAD_DAYS: &str = "helpdesk.plan_lead_days";
/// Hours after a work order is finished to ask the resident how it went (0 = off).
pub const FOLLOWUPS_RATING_HOURS: &str = "followups.rating_hours";
/// Days after a work order is finished to ask the resident if it's still fixed (0 = off).
pub const FOLLOWUPS_CHECKIN_DAYS: &str = "followups.checkin_days";
/// Hours after sending tasks to a vendor by email to nudge them if they haven't answered (0 = off).
pub const FOLLOWUPS_VENDOR_HOURS: &str = "followups.vendor_hours";
/// Hours after offering visit times to remind the person if they haven't picked (0 = off).
pub const FOLLOWUPS_OFFER_HOURS: &str = "followups.offer_hours";
/// Days after a showing to nudge a prospect who hasn't applied (0 = off).
pub const FOLLOWUPS_PROSPECT_DAYS: &str = "followups.prospect_days";
/// Flag an appliance to replace once its repair spend passes this percent of its price.
pub const ANALYTICS_REPLACE_SHARE_PCT: &str = "analytics.replace_share_pct";
/// When a planned day starts, HH:MM in the workspace's zone.
pub const ROUTES_DAY_START: &str = "routes.day_start";
/// How long a planned day is, in minutes.
pub const ROUTES_DAY_MINUTES: &str = "routes.day_minutes";
/// Driving between stops when a property has no coordinates, in minutes.
pub const ROUTES_DRIVE_MINUTES: &str = "routes.drive_minutes";
/// The supply run at the start of a day with parts to buy, in minutes.
pub const ROUTES_STORE_MINUTES: &str = "routes.store_minutes";
/// Time on site for a work order whose tasks carry no estimate, in minutes.
pub const ROUTES_JOB_MINUTES: &str = "routes.job_minutes";
/// Work estimated at or over this many cents waits for the owner's approval
/// before it's sent out (0 = never ask).
pub const MAINTENANCE_OWNER_APPROVAL_CENTS: &str = "maintenance.owner_approval_cents";
/// Ask the owner to sign off on finished billable work.
pub const MAINTENANCE_OWNER_SIGNOFF: &str = "maintenance.owner_signoff";
/// Day of the month the owner statement goes out (0 = don't send).
pub const OWNERS_STATEMENT_DAY: &str = "owners.statement_day";
/// `fbi` (the FBI Crime Data Explorer) or `off`.
pub const PROPERTY_DATA_CRIME_PROVIDER: &str = "property_data.crime_provider";
/// `simulated` or `rentcast` for parcel, tax and valuation records.
pub const PROPERTY_DATA_RECORDS_PROVIDER: &str = "property_data.records_provider";
/// Re-fetch public records older than this many days (0 = never).
pub const PROPERTY_DATA_REFRESH_DAYS: &str = "property_data.refresh_days";
/// Where a vendor signs up for Alpha when we invite them.
pub const PARTNERS_ALPHA_JOIN_URL: &str = "partners.alpha_join_url";
/// Text (or email) the resident for a 1–5 rating when their repair resolves.
pub const MAINTENANCE_ASK_RATING: &str = "maintenance.ask_rating";
/// In-house labor rate for work-order estimates, cents per hour.
pub const MAINTENANCE_LABOR_RATE: &str = "maintenance.labor_rate_cents";
/// Contractor labor rate for work-order estimates, cents per hour.
pub const MAINTENANCE_CONTRACTOR_RATE: &str = "maintenance.contractor_rate_cents";
/// Answer a resident's repair-sounding text with a prefilled request link.
pub const TEXTS_REPAIR_LINKS: &str = "texts.repair_links";
/// Hold automatic texts overnight.
pub const TEXTS_QUIET_HOURS: &str = "texts.quiet_hours";
/// Quiet hours start at this hour (0–23, local).
pub const TEXTS_QUIET_START: &str = "texts.quiet_start_hour";
/// Quiet hours end at this hour (0–23, local).
pub const TEXTS_QUIET_END: &str = "texts.quiet_end_hour";
/// The time zone quiet hours are kept in.
pub const TEXTS_TIMEZONE: &str = "texts.timezone";
/// Text back a caller nobody answered.
pub const TEXTS_MISSED_CALL_REPLY_ON: &str = "texts.missed_call_reply_on";
/// What the missed-call text says; `{company}` is filled in.
pub const TEXTS_MISSED_CALL_REPLY: &str = "texts.missed_call_reply";
/// Text the same caller back at most once in this many hours.
pub const TEXTS_MISSED_CALL_HOURS: &str = "texts.missed_call_hours";
/// Ring this number first when someone calls the texting number (blank: don't ring).
pub const TEXTS_FORWARD_NUMBER: &str = "texts.forward_number";
/// What a caller hears before the call rings through or ends.
pub const TEXTS_VOICE_GREETING: &str = "texts.voice_greeting";
/// Days before the rent day that residents hear rent is due (0 = off).
pub const REMINDERS_RENT_DUE_DAYS: &str = "reminders.rent_due_days";
/// Tell residents the day after rent was due and is still unpaid.
pub const REMINDERS_RENT_PAST_DUE: &str = "reminders.rent_past_due";
/// Days before a lease ends that staff hear about it ("90,60,30").
pub const REMINDERS_LEASE_EXPIRY_DAYS: &str = "reminders.lease_expiry_days";
/// At the first lease-expiry notice, draft a renewal for a manager to review.
pub const REMINDERS_DRAFT_RENEWAL: &str = "reminders.draft_renewal";
/// Days before an inspection that the resident is reminded ("2,1").
pub const REMINDERS_INSPECTION_DAYS: &str = "reminders.inspection_days";
/// Days before a warranty ends that staff hear about it (0 = off).
pub const REMINDERS_WARRANTY_DAYS: &str = "reminders.warranty_days";
/// Send managers a morning summary email.
pub const REMINDERS_MANAGER_DIGEST: &str = "reminders.manager_digest";
/// Hour of the day (UTC, 0–23) the morning summary goes out.
pub const REMINDERS_DIGEST_HOUR_UTC: &str = "reminders.digest_hour_utc";
/// SLA first-response targets per priority (`urgent:4,high:8,…`, hours).
pub const HELPDESK_SLA_RESPONSE_HOURS: &str = "helpdesk.sla_response_hours";
/// SLA resolution targets per priority (`urgent:24,high:72,…`, hours).
pub const HELPDESK_SLA_RESOLVE_HOURS: &str = "helpdesk.sla_resolve_hours";
/// Seconds the per-tenant helpdesk scan sleeps between runs.
pub const HELPDESK_SCAN_INTERVAL_SECS: &str = "helpdesk.scan_interval_secs";
/// Auto-open a make-ready ticket when a move-out inspection completes.
pub const HELPDESK_AUTO_TURNOVER: &str = "helpdesk.auto_turnover";

// ---- Team, time & costing (the back office) ----
/// IANA time zone that decides which workday an hour belongs to.
pub const WORKFORCE_TIMEZONE: &str = "workforce.timezone";
/// `weekly` (FLSA, 1.5× over 40) or `california` (daily 8/12 + seventh day).
pub const WORKFORCE_OVERTIME_RULE: &str = "workforce.overtime_rule";
/// Employer payroll taxes / workers' comp / benefits, basis points of pay.
pub const WORKFORCE_LABOR_BURDEN_BPS: &str = "workforce.labor_burden_bps";
/// Overhead spread per labor hour, in cents (0 = gross only).
pub const WORKFORCE_OVERHEAD_PER_HOUR_CENTS: &str = "workforce.overhead_per_hour_cents";
/// Gross margin target, basis points; work under it is flagged.
pub const WORKFORCE_TARGET_MARGIN_BPS: &str = "workforce.target_margin_bps";
/// Mileage rate in mills ($0.001) per mile — the IRS standard rate.
pub const WORKFORCE_MILEAGE_RATE_MILLS: &str = "workforce.mileage_rate_mills";
/// Markup on parts and billable expenses charged to owners, basis points.
pub const WORKFORCE_MAINTENANCE_MARKUP_BPS: &str = "workforce.maintenance_markup_bps";
/// Hours after which a clock-in still running is closed as a missed punch.
pub const WORKFORCE_MISSED_PUNCH_HOURS: &str = "workforce.missed_punch_hours";
/// Record where the phone is at clock-in / clock-out (never in between).
pub const WORKFORCE_CLOCK_LOCATION: &str = "workforce.clock_location";
/// The Gusto company payroll hours are pushed to.
pub const PAYROLL_GUSTO_COMPANY_UUID: &str = "payroll.gusto_company_uuid";
/// How far from the property counts as "away", in metres.
pub const WORKFORCE_CLOCK_RADIUS_M: &str = "workforce.clock_location_radius_m";

/// The value type of a setting (drives validation + the UI control).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingKind {
    Bool,
    Int,
    /// A free-text setting.
    Text,
}

impl SettingKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SettingKind::Bool => "bool",
            SettingKind::Int => "int",
            SettingKind::Text => "text",
        }
    }

    /// Whether `v` is a valid JSON value for this kind.
    fn validate(&self, v: &Value) -> bool {
        match self {
            SettingKind::Bool => v.is_boolean(),
            SettingKind::Int => v.is_i64() || v.is_u64(),
            SettingKind::Text => v.is_string(),
        }
    }
}

/// One entry in the settings catalog.
pub struct SettingDef {
    pub key: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub group: &'static str,
    pub kind: SettingKind,
    /// Default value when the tenant has no override row.
    pub default: fn() -> Value,
}

/// Every recognized setting. Add new tenant-configurable knobs here.
pub const CATALOG: &[SettingDef] = &[
    SettingDef {
        key: APPLICATION_REUSE_ENABLED,
        label: "Reusable applications",
        description: "Let a recent application be reused for any property in the \
                      workspace, so applicants don't re-apply per listing.",
        group: "Applications",
        kind: SettingKind::Bool,
        default: || json!(false),
    },
    SettingDef {
        key: APPLICATION_REUSE_WINDOW_DAYS,
        label: "Reuse window (days)",
        description: "How many days a prior application stays reusable.",
        group: "Applications",
        kind: SettingKind::Int,
        default: || json!(30),
    },
    SettingDef {
        key: APPLICATION_AUTO_APPROVE,
        label: "Auto-approve cleared screenings",
        description: "Approve an application automatically the moment its \
                      background screening clears (the applicant is emailed). \
                      Off = screening results wait for a staff decision.",
        group: "Applications",
        kind: SettingKind::Bool,
        default: || json!(false),
    },
    SettingDef {
        key: APPLICATION_GENERATE_DOC_ON_CONVERT,
        label: "Auto-generate lease document on conversion",
        description: "Draft the lease agreement automatically when an \
                      application converts to a lease. Turn off when the \
                      workspace uses external paperwork. (A conversion request \
                      can still override either way per call.)",
        group: "Applications",
        kind: SettingKind::Bool,
        default: || json!(true),
    },
    SettingDef {
        key: SCREENING_MIN_CREDIT_SCORE,
        label: "Minimum credit score",
        description: "Screening fails when the applicant's reported credit \
                      score is below this floor. 0 disables the check; an \
                      application without a score is never failed by it.",
        group: "Screening",
        kind: SettingKind::Int,
        default: || json!(0),
    },
    SettingDef {
        key: SCREENING_MIN_INCOME_RENT_RATIO,
        label: "Minimum income-to-rent multiple",
        description: "Screening fails when the applicant's stated monthly \
                      income is below this multiple of the listing's rent \
                      (e.g. 3 = income must be at least 3× rent). 0 disables \
                      the check; it only runs when the application targets a \
                      listing with a rent.",
        group: "Screening",
        kind: SettingKind::Int,
        default: || json!(0),
    },
    SettingDef {
        key: SCREENING_CALLBACK_DELAY_SECS,
        label: "Provider callback delay (seconds)",
        description: "How long the simulated screening provider takes to call \
                      back with a verdict. A live provider (roadmap Phase 4) \
                      ignores this.",
        group: "Screening",
        kind: SettingKind::Int,
        default: || json!(6),
    },
    SettingDef {
        key: SCREENING_CRA_NAME,
        label: "Consumer-reporting agency name",
        description: "The screening bureau named on FCRA adverse-action \
                      notices — the applicant's point of contact for report \
                      copies and disputes.",
        group: "Screening",
        kind: SettingKind::Text,
        default: || json!("Checkr, Inc. (consumer reporting agency)"),
    },
    SettingDef {
        key: SCREENING_CRA_CONTACT,
        label: "Consumer-reporting agency contact",
        description: "Address/phone/email printed under the agency name on \
                      adverse-action notices.",
        group: "Screening",
        kind: SettingKind::Text,
        default: || json!("1 Montgomery St, San Francisco, CA 94104 · (844) 824-3257 · checkr.com"),
    },
    SettingDef {
        key: SCREENING_AUTO_ADVERSE_ACTION,
        label: "Auto-send adverse-action notices",
        description: "When a declined application's screening report carried \
                      adverse information, send (and file) the FCRA §615(a) \
                      notice automatically. Off = staff send it from the \
                      application console.",
        group: "Screening",
        kind: SettingKind::Bool,
        default: || json!(true),
    },
    SettingDef {
        key: ESIGN_LINK_EXPIRY_DAYS,
        label: "Signing-link validity (days)",
        description: "Signing links stop working this many days after the \
                      envelope is sent (void + re-send to issue fresh ones). \
                      0 = links stay valid until the envelope completes or is \
                      voided.",
        group: "E-signature",
        kind: SettingKind::Int,
        default: || json!(0),
    },
    SettingDef {
        key: ESIGN_MAX_SIGNERS,
        label: "Maximum signers per envelope",
        description: "Upper bound on the number of signers one envelope can \
                      carry.",
        group: "E-signature",
        kind: SettingKind::Int,
        default: || json!(10),
    },
    SettingDef {
        key: ESIGN_SIGNED_DOC_RETENTION_DAYS,
        label: "Signed-lease retention (days)",
        description: "Retention window stamped on the stored signed-lease PDF \
                      (drives the document service's expiry). 0 = keep \
                      forever.",
        group: "E-signature",
        kind: SettingKind::Int,
        default: || json!(0),
    },
    SettingDef {
        key: LEASE_DOC_TITLE,
        label: "Lease document title",
        description: "Title given to generated lease agreements and the \
                      e-signature envelopes sent for them.",
        group: "Lease documents",
        kind: SettingKind::Text,
        default: || json!("Residential Lease Agreement"),
    },
    SettingDef {
        key: LEASE_RENEWAL_DOC_TITLE,
        label: "Renewal addendum title",
        description: "Title given to generated lease-renewal addenda and the \
                      e-signature envelopes sent for them.",
        group: "Lease documents",
        kind: SettingKind::Text,
        default: || json!("Lease Renewal Addendum"),
    },
    SettingDef {
        key: PAYMENTS_AUTOPAY_ENABLED,
        label: "Autopay",
        description: "Let residents enroll a saved payment method in autopay: \
                      rent is charged automatically on its due date.",
        group: "Payments",
        kind: SettingKind::Bool,
        default: || json!(true),
    },
    SettingDef {
        key: PAYMENTS_RENT_DUE_DAY,
        label: "Rent due day of month",
        description: "The day of the month rent falls due (1–28). The billing \
                      cycle raises each active lease's rent receivable on this \
                      day.",
        group: "Payments",
        kind: SettingKind::Int,
        default: || json!(1),
    },
    SettingDef {
        key: PAYMENTS_CALLBACK_DELAY_SECS,
        label: "Processor callback delay (seconds)",
        description: "How long the simulated payment processor takes to \
                      confirm a charge. A live processor (Stripe) ignores \
                      this — its webhook drives settlement.",
        group: "Payments",
        kind: SettingKind::Int,
        default: || json!(5),
    },
    SettingDef {
        key: LATE_FEE_GRACE_DAYS,
        label: "Late-fee grace period (days)",
        description: "Days past the due date before a late fee is assessed. \
                      0 disables automatic late fees.",
        group: "Payments",
        kind: SettingKind::Int,
        default: || json!(5),
    },
    SettingDef {
        key: LATE_FEE_FLAT_CENTS,
        label: "Late fee — flat amount (cents)",
        description: "Flat late-fee amount in cents (e.g. 7500 = $75). \
                      Combined with the percentage component when both are set.",
        group: "Payments",
        kind: SettingKind::Int,
        default: || json!(7500),
    },
    SettingDef {
        key: LATE_FEE_PERCENT_BPS,
        label: "Late fee — percentage (basis points)",
        description: "Percentage late fee in basis points of the overdue \
                      amount (e.g. 500 = 5%). 0 = flat fee only.",
        group: "Payments",
        kind: SettingKind::Int,
        default: || json!(0),
    },
    SettingDef {
        key: LATE_FEE_RECURRENCE,
        label: "Late-fee recurrence",
        description: "one_time = a single fee per overdue period; daily = the \
                      fee re-applies each day the balance stays overdue \
                      (subject to the cap).",
        group: "Payments",
        kind: SettingKind::Text,
        default: || json!("one_time"),
    },
    SettingDef {
        key: LATE_FEE_MAX_CENTS,
        label: "Late-fee cap per period (cents)",
        description: "Ceiling on the total late fees assessed against one \
                      billing period. 0 = no cap.",
        group: "Payments",
        kind: SettingKind::Int,
        default: || json!(0),
    },
    SettingDef {
        key: PAYOUT_MGMT_FEE_BPS,
        label: "Management fee (basis points)",
        description: "The management fee withheld from owner payouts, in \
                      basis points of rent collected for the period (e.g. \
                      800 = 8%).",
        group: "Payments",
        kind: SettingKind::Int,
        default: || json!(800),
    },
    SettingDef {
        key: APPOINTMENT_REMINDER_HOURS,
        label: "Appointment reminders (hours before)",
        description: "Comma-separated hours before a confirmed visit at which the \
                      resident (and the person going) are reminded, e.g. \"24,2\".",
        group: "Calendar",
        kind: SettingKind::Text,
        default: || json!("24,2"),
    },
    SettingDef {
        key: APPOINTMENT_WINDOW_MINUTES,
        label: "Default visit window (minutes)",
        description: "How long an offered time window is when only a start time is given.",
        group: "Calendar",
        kind: SettingKind::Int,
        default: || json!(120),
    },
    SettingDef {
        key: CALENDAR_DEFAULT_LEAD_DAYS,
        label: "Default reminder lead times (days)",
        description: "Comma-separated days before a due date at which new \
                      reminders notify (e.g. \"30,7,1\"; 0 = the day itself). \
                      Existing reminders keep their own lead times.",
        group: "Calendar",
        kind: SettingKind::Text,
        default: || json!("30,7,1"),
    },
    SettingDef {
        key: CALENDAR_SCAN_INTERVAL_SECS,
        label: "Reminder scan interval (seconds)",
        description: "How often the reminder engine scans for due dates and \
                      fires notifications.",
        group: "Calendar",
        kind: SettingKind::Int,
        default: || json!(3600),
    },
    SettingDef {
        key: CALENDAR_LEASE_RENEWAL_SYNC,
        label: "Lease renewal reminders",
        description: "Automatically keep a renewal reminder on every active \
                      lease's end date.",
        group: "Calendar",
        kind: SettingKind::Bool,
        default: || json!(true),
    },
    SettingDef {
        key: HELPDESK_PLAN_LEAD_DAYS,
        label: "Schedule routines this many days ahead",
        description: "A routine (filters, inspections, code-required checks) shows under \
                      \"To schedule\" this many days before it's due, so the visit can be \
                      booked before the work order opens on the day.",
        group: "Helpdesk",
        kind: SettingKind::Int,
        default: || json!(14),
    },
    SettingDef {
        key: ANALYTICS_REPLACE_SHARE_PCT,
        label: "Replace instead of repair at (percent of price)",
        description: "The operations dashboard flags an appliance once repairs in the window add up to this share of what it cost.",
        group: "Maintenance",
        kind: SettingKind::Int,
        default: || json!(50),
    },
    SettingDef {
        key: FOLLOWUPS_RATING_HOURS,
        label: "Ask the resident to rate finished work after (hours)",
        description: "Email and text once the work order has been finished this long and nobody has rated it. 0 turns it off.",
        group: "Follow-ups",
        kind: SettingKind::Int,
        default: || json!(24),
    },
    SettingDef {
        key: FOLLOWUPS_CHECKIN_DAYS,
        label: "Check in on finished work after (days)",
        description: "Ask the resident whether it's still fixed, once. 0 turns it off.",
        group: "Follow-ups",
        kind: SettingKind::Int,
        default: || json!(7),
    },
    SettingDef {
        key: FOLLOWUPS_VENDOR_HOURS,
        label: "Nudge a quiet vendor after (hours)",
        description: "Tasks sent by email with no answer from the vendor's link get one reminder with a fresh link. 0 turns it off.",
        group: "Follow-ups",
        kind: SettingKind::Int,
        default: || json!(48),
    },
    SettingDef {
        key: FOLLOWUPS_OFFER_HOURS,
        label: "Remind about offered visit times after (hours)",
        description: "The person offered times gets the link again once, while the times are still ahead. 0 turns it off.",
        group: "Follow-ups",
        kind: SettingKind::Int,
        default: || json!(24),
    },
    SettingDef {
        key: FOLLOWUPS_PROSPECT_DAYS,
        label: "Nudge a prospect after a showing (days)",
        description: "A prospect who toured and hasn't applied gets the application link once. 0 turns it off.",
        group: "Follow-ups",
        kind: SettingKind::Int,
        default: || json!(2),
    },
    SettingDef {
        key: ROUTES_DAY_START,
        label: "Planned day starts at",
        description:
            "When a technician's planned day starts (HH:MM, in the workspace's time zone).",
        group: "Helpdesk",
        kind: SettingKind::Text,
        default: || json!("08:00"),
    },
    SettingDef {
        key: ROUTES_DAY_MINUTES,
        label: "Planned day length (minutes)",
        description: "Work past this goes to the next day when a route is proposed.",
        group: "Helpdesk",
        kind: SettingKind::Int,
        default: || json!(480),
    },
    SettingDef {
        key: ROUTES_DRIVE_MINUTES,
        label: "Driving between stops (minutes)",
        description: "Used when a property has no coordinates; otherwise the distance decides.",
        group: "Helpdesk",
        kind: SettingKind::Int,
        default: || json!(20),
    },
    SettingDef {
        key: ROUTES_STORE_MINUTES,
        label: "Supply run (minutes)",
        description: "The stop at the store at the start of a day with parts to buy.",
        group: "Helpdesk",
        kind: SettingKind::Int,
        default: || json!(30),
    },
    SettingDef {
        key: ROUTES_JOB_MINUTES,
        label: "Time on site without an estimate (minutes)",
        description: "A work order whose tasks carry no minutes is planned at this length.",
        group: "Helpdesk",
        kind: SettingKind::Int,
        default: || json!(60),
    },
    SettingDef {
        key: MAINTENANCE_OWNER_APPROVAL_CENTS,
        label: "Owner approval over (cents)",
        description: "Work estimated at or over this amount waits for the property owner's \
                      approval before it goes to a vendor. 50000 = $500. An owner can have \
                      their own limit. Staff can go ahead with a reason, which is recorded. \
                      0 never asks.",
        group: "Owners",
        kind: SettingKind::Int,
        default: || json!(50000),
    },
    SettingDef {
        key: MAINTENANCE_OWNER_SIGNOFF,
        label: "Owner sign-off on finished work",
        description: "When billable work is marked done, the owner gets a summary with the \
                      cost and photos and signs off or disputes it from the link.",
        group: "Owners",
        kind: SettingKind::Bool,
        default: || json!(true),
    },
    SettingDef {
        key: OWNERS_STATEMENT_DAY,
        label: "Monthly statement day",
        description: "Day of the month each owner is emailed last month's statement: rent \
                      collected, expenses, management fee, net, and the work done. 0 turns \
                      it off; owners can still read statements in their portal.",
        group: "Owners",
        kind: SettingKind::Int,
        default: || json!(3),
    },
    SettingDef {
        key: PROPERTY_DATA_CRIME_PROVIDER,
        label: "Crime statistics",
        description: "\"fbi\" reads the FBI Crime Data Explorer (free; put a data.gov key \
                      in the vault as fbi.api_key, or the shared demo key is used) for the \
                      nearest reporting agency's rates against the state and the country. \
                      \"off\" keeps the simulated figures.",
        group: "Property data",
        kind: SettingKind::Text,
        default: || json!("fbi"),
    },
    SettingDef {
        key: PROPERTY_DATA_RECORDS_PROVIDER,
        label: "Public records provider",
        description: "\"rentcast\" fetches the parcel, tax years and value estimate from \
                      RentCast with the key in the vault as rentcast.api_key. \"simulated\" \
                      uses deterministic stand-ins.",
        group: "Property data",
        kind: SettingKind::Text,
        default: || json!("simulated"),
    },
    SettingDef {
        key: PROPERTY_DATA_REFRESH_DAYS,
        label: "Refresh public records every (days)",
        description: "Properties whose records are older than this are re-fetched, a few \
                      a night. 0 turns the nightly refresh off.",
        group: "Property data",
        kind: SettingKind::Int,
        default: || json!(30),
    },
    SettingDef {
        key: PARTNERS_ALPHA_JOIN_URL,
        label: "Alpha sign-up link for vendors",
        description: "The page a vendor is sent to when you invite them to Alpha. \
                      Leave blank to use Alpha's public sign-up.",
        group: "Vendors",
        kind: SettingKind::Text,
        default: || json!("https://alphapowerwash.com/partners/join"),
    },
    SettingDef {
        key: COMPLIANCE_REQUIRE_COI,
        label: "Require vendor insurance to dispatch",
        description: "A vendor without current general liability insurance on \
                      file can only be assigned a work order with a reason, which \
                      is recorded in the audit trail.",
        group: "Vendors",
        kind: SettingKind::Bool,
        default: || json!(false),
    },
    SettingDef {
        key: MAINTENANCE_LABOR_RATE,
        label: "In-house labor rate (estimates)",
        description: "Cents per hour used to estimate tasks your team does, \
                      e.g. 7500 for $75/hr.",
        group: "Maintenance",
        kind: SettingKind::Int,
        default: || json!(7_500),
    },
    SettingDef {
        key: MAINTENANCE_CONTRACTOR_RATE,
        label: "Contractor labor rate (estimates)",
        description: "Cents per hour used to estimate tasks that need a \
                      contractor, e.g. 12500 for $125/hr.",
        group: "Maintenance",
        kind: SettingKind::Int,
        default: || json!(12_500),
    },
    SettingDef {
        key: MAINTENANCE_ASK_RATING,
        label: "Ask residents to rate repairs",
        description: "When a resident's work order is resolved, text them \
                      \"How did we do? Reply 1-5\" (or email a link when there's no \
                      number). A reply of 1 to 5 within a week becomes the review.",
        group: "Texts",
        kind: SettingKind::Bool,
        default: || json!(true),
    },
    SettingDef {
        key: TEXTS_REPAIR_LINKS,
        label: "Answer repair texts with a request link",
        description: "When a resident texts about something broken, reply with a \
                      link to a maintenance request already filled in. At most \
                      once a day per conversation.",
        group: "Texts",
        kind: SettingKind::Bool,
        default: || json!(true),
    },
    SettingDef {
        key: TEXTS_QUIET_HOURS,
        label: "Quiet hours",
        description: "Automatic texts (reminders, updates) wait until morning. \
                      Replies you type, and answers to someone who just texted, \
                      still go at once.",
        group: "Texts",
        kind: SettingKind::Bool,
        default: || json!(true),
    },
    SettingDef {
        key: TEXTS_QUIET_START,
        label: "Quiet hours start",
        description: "Hour of the day (0-23) automatic texts stop.",
        group: "Texts",
        kind: SettingKind::Int,
        default: || json!(21),
    },
    SettingDef {
        key: TEXTS_QUIET_END,
        label: "Quiet hours end",
        description: "Hour of the day (0-23) automatic texts start again.",
        group: "Texts",
        kind: SettingKind::Int,
        default: || json!(8),
    },
    SettingDef {
        key: TEXTS_MISSED_CALL_REPLY_ON,
        label: "Text back missed calls",
        description: "When someone calls the texting number and nobody answers, \
                      text them so the conversation can carry on in the inbox.",
        group: "Texts",
        kind: SettingKind::Bool,
        default: || json!(true),
    },
    SettingDef {
        key: TEXTS_MISSED_CALL_REPLY,
        label: "Missed-call text",
        description: "{company} is replaced with the workspace name.",
        group: "Texts",
        kind: SettingKind::Text,
        default: || json!("Sorry we missed your call. This is {company}. Text us here and we'll get right back to you."),
    },
    SettingDef {
        key: TEXTS_MISSED_CALL_HOURS,
        label: "Text the same caller back at most every (hours)",
        description: "Someone who calls three times in a row gets one text.",
        group: "Texts",
        kind: SettingKind::Int,
        default: || json!(4),
    },
    SettingDef {
        key: TEXTS_FORWARD_NUMBER,
        label: "Ring this phone first",
        description: "Calls to the texting number ring here for 20 seconds; if \
                      nobody answers, the caller gets the text. Leave blank to \
                      skip ringing.",
        group: "Texts",
        kind: SettingKind::Text,
        default: || json!(""),
    },
    SettingDef {
        key: TEXTS_VOICE_GREETING,
        label: "What callers hear",
        description: "Said before the call rings through, or before it ends \
                      when nothing is set to ring. {company} is filled in.",
        group: "Texts",
        kind: SettingKind::Text,
        default: || json!("Thanks for calling {company}."),
    },
    SettingDef {
        key: TEXTS_TIMEZONE,
        label: "Texting time zone",
        description: "Quiet hours are kept in this time zone, e.g. \
                      America/Los_Angeles.",
        group: "Texts",
        kind: SettingKind::Text,
        default: || json!("America/Los_Angeles"),
    },
    SettingDef {
        key: REMINDERS_ENABLED,
        label: "Automatic notices",
        description: "Send the notices below on their own: rent due and past due, \
                      inspection reminders, lease expiry and warranty notices, and \
                      the managers' morning summary.",
        group: "Reminders",
        kind: SettingKind::Bool,
        default: || json!(true),
    },
    SettingDef {
        key: REMINDERS_RENT_DUE_DAYS,
        label: "Rent due notice (days before)",
        description: "Tell residents rent is due this many days before the rent \
                      day. Residents on autopay are skipped. 0 turns it off.",
        group: "Reminders",
        kind: SettingKind::Int,
        default: || json!(3),
    },
    SettingDef {
        key: REMINDERS_RENT_PAST_DUE,
        label: "Rent past due notice",
        description: "Tell residents the day after rent was due if it is still \
                      unpaid, before any late fee.",
        group: "Reminders",
        kind: SettingKind::Bool,
        default: || json!(true),
    },
    SettingDef {
        key: REMINDERS_LEASE_EXPIRY_DAYS,
        label: "Lease expiry notices (days before)",
        description: "Comma-separated days before a lease ends that the office \
                      hears about it, e.g. \"90,60,30\".",
        group: "Reminders",
        kind: SettingKind::Text,
        default: || json!("90,60,30"),
    },
    SettingDef {
        key: REMINDERS_DRAFT_RENEWAL,
        label: "Draft renewals automatically",
        description: "At the first lease expiry notice, draft a renewal at the \
                      current rent for a manager to review and send. Nothing \
                      reaches the resident until a person sends it.",
        group: "Reminders",
        kind: SettingKind::Bool,
        default: || json!(true),
    },
    SettingDef {
        key: REMINDERS_INSPECTION_DAYS,
        label: "Inspection reminders (days before)",
        description: "Comma-separated days before a scheduled inspection that the \
                      resident is reminded, e.g. \"2,1\".",
        group: "Reminders",
        kind: SettingKind::Text,
        default: || json!("2,1"),
    },
    SettingDef {
        key: REMINDERS_WARRANTY_DAYS,
        label: "Warranty ending notice (days before)",
        description: "Tell the office this many days before an appliance's \
                      warranty ends. 0 turns it off.",
        group: "Reminders",
        kind: SettingKind::Int,
        default: || json!(30),
    },
    SettingDef {
        key: REMINDERS_MANAGER_DIGEST,
        label: "Managers' morning summary",
        description: "One email each morning to staff who can see properties: \
                      rent late, leases ending soon, work orders past SLA, turns \
                      past target and new tour requests. Skipped when empty.",
        group: "Reminders",
        kind: SettingKind::Bool,
        default: || json!(true),
    },
    SettingDef {
        key: REMINDERS_DIGEST_HOUR_UTC,
        label: "Morning summary hour (UTC)",
        description: "The hour of the day, in UTC, the morning summary is sent \
                      (14 is 7 AM Pacific in summer).",
        group: "Reminders",
        kind: SettingKind::Int,
        default: || json!(14),
    },
    SettingDef {
        key: HELPDESK_SLA_RESPONSE_HOURS,
        label: "SLA: first-response hours",
        description: "Target hours to first staff response per priority, as \
                      `priority:hours` pairs (0 disables a priority's target).",
        group: "Helpdesk",
        kind: SettingKind::Text,
        default: || json!("urgent:4,high:8,normal:24,low:72"),
    },
    SettingDef {
        key: HELPDESK_SLA_RESOLVE_HOURS,
        label: "SLA: resolution hours",
        description: "Target hours to resolution per priority, as \
                      `priority:hours` pairs (0 disables a priority's target).",
        group: "Helpdesk",
        kind: SettingKind::Text,
        default: || json!("urgent:24,high:72,normal:168,low:336"),
    },
    SettingDef {
        key: HELPDESK_SCAN_INTERVAL_SECS,
        label: "Helpdesk scan interval (seconds)",
        description: "How often the helpdesk scan checks for SLA breaches and \
                      due preventive-maintenance plans.",
        group: "Helpdesk",
        kind: SettingKind::Int,
        default: || json!(3600),
    },
    SettingDef {
        key: HELPDESK_AUTO_TURNOVER,
        label: "Auto make-ready on move-out",
        description: "Completing a move-out inspection opens a turnover ticket \
                      and flags the unit make-ready.",
        group: "Helpdesk",
        kind: SettingKind::Bool,
        default: || json!(true),
    },
    SettingDef {
        key: WORKFORCE_TIMEZONE,
        label: "Time zone",
        description: "The workday an hour belongs to (and so daily overtime) is \
                      decided in this time zone, e.g. America/Los_Angeles.",
        group: "Team & payroll",
        kind: SettingKind::Text,
        default: || json!("America/Los_Angeles"),
    },
    SettingDef {
        key: WORKFORCE_OVERTIME_RULE,
        label: "Overtime rule",
        description: "\"weekly\": 1.5× over 40 hours in a Monday–Sunday week. \
                      \"california\": also 1.5× over 8 and 2× over 12 hours in a day, \
                      plus the seventh-consecutive-day rule (Labor Code §510).",
        group: "Team & payroll",
        kind: SettingKind::Text,
        default: || json!("weekly"),
    },
    SettingDef {
        key: WORKFORCE_LABOR_BURDEN_BPS,
        label: "Labor burden (basis points)",
        description: "Added to employees' pay in work-order costs: employer payroll \
                      taxes, workers' comp, benefits. 1500 = 15%. Not applied to 1099 \
                      contractors.",
        group: "Team & payroll",
        kind: SettingKind::Int,
        default: || json!(0),
    },
    SettingDef {
        key: WORKFORCE_OVERHEAD_PER_HOUR_CENTS,
        label: "Overhead per labor hour (cents)",
        description: "Office, insurance, trucks and software spread over labor hours, \
                      for a net figure. 0 = gross only.",
        group: "Team & payroll",
        kind: SettingKind::Int,
        default: || json!(0),
    },
    SettingDef {
        key: WORKFORCE_TARGET_MARGIN_BPS,
        label: "Target margin (basis points)",
        description: "In-house work whose gross margin falls under this is flagged, \
                      with the bill rate that would reach it. 5000 = 50%.",
        group: "Team & payroll",
        kind: SettingKind::Int,
        default: || json!(5000),
    },
    SettingDef {
        key: WORKFORCE_MILEAGE_RATE_MILLS,
        label: "Mileage rate (tenths of a cent per mile)",
        description: "Prices mileage trips and own-vehicle reimbursements — the IRS \
                      standard rate. 700 = $0.70 a mile.",
        group: "Team & payroll",
        kind: SettingKind::Int,
        default: || json!(700),
    },
    SettingDef {
        key: WORKFORCE_MAINTENANCE_MARKUP_BPS,
        label: "Markup on parts & expenses billed to owners (basis points)",
        description: "Added to parts and billable expenses when in-house work is \
                      billed to the owner. 1000 = 10%. Labor bills at each person's \
                      bill rate.",
        group: "Team & payroll",
        kind: SettingKind::Int,
        default: || json!(0),
    },
    SettingDef {
        key: WORKFORCE_MISSED_PUNCH_HOURS,
        label: "Missed punch after (hours)",
        description: "A clock-in still running this long is closed at the best guess \
                      and held for the office to confirm before payroll (4–24).",
        group: "Team & payroll",
        kind: SettingKind::Int,
        default: || json!(12),
    },
    SettingDef {
        key: WORKFORCE_CLOCK_LOCATION,
        label: "Record clock location",
        description: "Note where the phone is when someone clocks in or out — only \
                      then, never in between. Nothing is blocked; far-away punches \
                      are flagged.",
        group: "Team & payroll",
        kind: SettingKind::Bool,
        default: || json!(false),
    },
    SettingDef {
        key: WORKFORCE_CLOCK_RADIUS_M,
        label: "Clock location radius (metres)",
        description: "How far from the property counts as away (50–5000).",
        group: "Team & payroll",
        kind: SettingKind::Int,
        default: || json!(400),
    },
    SettingDef {
        key: PAYROLL_GUSTO_COMPANY_UUID,
        label: "Gusto company ID",
        description: "Where approved hours are pushed for payroll. The access token \
                      goes in Integrations → credentials as gusto.access_token.",
        group: "Team & payroll",
        kind: SettingKind::Text,
        default: || json!(""),
    },
];

/// Look up a catalog entry by key.
pub fn def(key: &str) -> Option<&'static SettingDef> {
    CATALOG.iter().find(|d| d.key == key)
}

/// The effective JSON value for `key` in `tenant_id` (override row or default).
pub async fn get_value(db: &impl ConnectionTrait, tenant_id: Uuid, key: &str) -> Value {
    let default = def(key).map(|d| (d.default)()).unwrap_or(Value::Null);
    match Setting::find()
        .filter(entity::setting::Column::TenantId.eq(tenant_id))
        .filter(entity::setting::Column::Key.eq(key))
        .one(db)
        .await
    {
        Ok(Some(row)) => row.value,
        Ok(None) => default,
        Err(e) => {
            tracing::error!("setting lookup failed for {key}: {e}");
            default
        }
    }
}

/// Typed accessor: a boolean setting (false if missing/mistyped).
pub async fn get_bool(db: &impl ConnectionTrait, tenant_id: Uuid, key: &str) -> bool {
    get_value(db, tenant_id, key)
        .await
        .as_bool()
        .unwrap_or(false)
}

/// Typed accessor: an integer setting (0 if missing/mistyped).
pub async fn get_i64(db: &impl ConnectionTrait, tenant_id: Uuid, key: &str) -> i64 {
    get_value(db, tenant_id, key).await.as_i64().unwrap_or(0)
}

/// Typed accessor: a text setting (the catalog default if missing/mistyped).
pub async fn get_string(db: &impl ConnectionTrait, tenant_id: Uuid, key: &str) -> String {
    match get_value(db, tenant_id, key).await.as_str() {
        Some(s) if !s.trim().is_empty() => s.to_string(),
        _ => def(key)
            .map(|d| (d.default)())
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default(),
    }
}

/// Validate + upsert a setting override. Rejects unknown keys and type mismatches.
pub async fn set_value(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    key: &str,
    value: Value,
) -> ApiResult<Value> {
    let d = def(key).ok_or_else(|| ApiError::BadRequest(format!("unknown setting: {key}")))?;
    if !d.kind.validate(&value) {
        return Err(ApiError::BadRequest(format!(
            "setting '{key}' expects a {} value",
            d.kind.as_str()
        )));
    }
    let now = Utc::now();
    match Setting::find()
        .filter(entity::setting::Column::TenantId.eq(tenant_id))
        .filter(entity::setting::Column::Key.eq(key))
        .one(db)
        .await?
    {
        Some(row) => {
            let mut am: entity::setting::ActiveModel = row.into();
            am.value = ActiveSet(value.clone());
            am.updated_at = ActiveSet(now.into());
            am.update(db).await?;
        }
        None => {
            entity::setting::ActiveModel {
                id: ActiveSet(Uuid::new_v4()),
                tenant_id: ActiveSet(tenant_id),
                key: ActiveSet(key.to_string()),
                value: ActiveSet(value.clone()),
                updated_at: ActiveSet(now.into()),
            }
            .insert(db)
            .await?;
        }
    }
    Ok(value)
}
