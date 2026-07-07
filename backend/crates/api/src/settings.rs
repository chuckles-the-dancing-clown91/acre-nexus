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
/// SLA first-response targets per priority (`urgent:4,high:8,…`, hours).
pub const HELPDESK_SLA_RESPONSE_HOURS: &str = "helpdesk.sla_response_hours";
/// SLA resolution targets per priority (`urgent:24,high:72,…`, hours).
pub const HELPDESK_SLA_RESOLVE_HOURS: &str = "helpdesk.sla_resolve_hours";
/// Seconds the per-tenant helpdesk scan sleeps between runs.
pub const HELPDESK_SCAN_INTERVAL_SECS: &str = "helpdesk.scan_interval_secs";
/// Auto-open a make-ready ticket when a move-out inspection completes.
pub const HELPDESK_AUTO_TURNOVER: &str = "helpdesk.auto_turnover";

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
