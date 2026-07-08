//! **SaaS platform billing** (roadmap Phase 8) — the revenue side of the
//! platform: Acre HQ metering and billing each client workspace for its
//! subscription. This is deliberately *not* a pluggable module (a tenant can't
//! switch off being billed) — it is core infrastructure wired directly into the
//! boot sequence and the scheduler, alongside [`crate::billing`] (which is the
//! unrelated *resident* rent-billing cycle).
//!
//! Pricing is a per-plan **base fee** plus a **metered overage** on units under
//! management beyond the plan's included allowance. Each billing month a
//! per-tenant `platform_billing` job assembles one [`entity::platform_invoice`]
//! (with frozen [`entity::platform_invoice_line`]s), so the bill is reproducible
//! even if plan pricing later changes. A workspace sees its own subscription +
//! invoices via `billing:read`; Acre staff run and settle billing across every
//! workspace on the platform plane.

use crate::modules::JobOutcome;
use chrono::{Datelike, NaiveDate, Utc};
use entity::prelude::{PlatformInvoice, Property, Tenant};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    Set,
};
use serde_json::json;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Plan catalog
// ---------------------------------------------------------------------------

/// A subscription plan: a base monthly fee, an included unit allowance, and the
/// per-unit price charged on units beyond it.
#[derive(Clone, Copy, Debug)]
pub struct Plan {
    /// Stable key, persisted on `tenant.plan`.
    pub key: &'static str,
    pub name: &'static str,
    /// Monthly base fee (charged even at zero units).
    pub base_cents: i64,
    /// Units under management included in the base fee.
    pub included_units: i32,
    /// Price per unit beyond `included_units`.
    pub overage_cents: i64,
    /// One-line positioning blurb for the pricing UI.
    pub blurb: &'static str,
    /// Headline capabilities, for the plan card.
    pub features: &'static [&'static str],
}

/// The published plans. Prices are illustrative but realistic for per-door PM
/// SaaS: a low base with a generous allowance, scaling to a lower marginal
/// per-unit price on the larger plans.
pub const PLANS: &[Plan] = &[
    Plan {
        key: "starter",
        name: "Starter",
        base_cents: 4_900,
        included_units: 25,
        overage_cents: 250,
        blurb: "For new managers getting their first doors online.",
        features: &[
            "Up to 25 units included",
            "Property, leasing & maintenance",
            "Resident portal & online payments",
            "Email support",
        ],
    },
    Plan {
        key: "growth",
        name: "Growth",
        base_cents: 19_900,
        included_units: 100,
        overage_cents: 200,
        blurb: "For growing firms that need reporting and automation.",
        features: &[
            "100 units included",
            "Everything in Starter",
            "Standard PM reports + exports",
            "Global search & screening",
            "Priority support",
        ],
    },
    Plan {
        key: "enterprise",
        name: "Enterprise",
        base_cents: 79_900,
        included_units: 500,
        overage_cents: 150,
        blurb: "For portfolios at scale with custom needs.",
        features: &[
            "500 units included",
            "Everything in Growth",
            "Custom domains & white-label",
            "Acquisitions, rehab & investor tools",
            "Dedicated account manager",
        ],
    },
];

/// The plan for a key, defaulting to `starter` for unknown / legacy values.
pub fn plan_for(key: &str) -> &'static Plan {
    PLANS.iter().find(|p| p.key == key).unwrap_or(&PLANS[0])
}

// ---------------------------------------------------------------------------
// Pure pricing (unit-tested)
// ---------------------------------------------------------------------------

/// One assembled line: description + quantity × unit price = amount.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineDraft {
    pub description: String,
    pub quantity: i32,
    pub unit_price_cents: i64,
    pub amount_cents: i64,
}

/// A priced-out invoice before persistence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assembled {
    pub base_cents: i64,
    pub overage_cents: i64,
    pub total_cents: i64,
    pub lines: Vec<LineDraft>,
}

/// Price a plan for a metered `unit_count`. Pure: base fee always applies; each
/// unit past the allowance is charged at the plan's overage rate.
pub fn assemble(plan: &Plan, unit_count: i32) -> Assembled {
    let overage_units = (unit_count - plan.included_units).max(0);
    let overage_cents = overage_units as i64 * plan.overage_cents;

    let mut lines = vec![LineDraft {
        description: format!("{} plan — monthly platform fee", plan.name),
        quantity: 1,
        unit_price_cents: plan.base_cents,
        amount_cents: plan.base_cents,
    }];
    if overage_units > 0 {
        lines.push(LineDraft {
            description: format!(
                "Metered units — {overage_units} over {} included",
                plan.included_units
            ),
            quantity: overage_units,
            unit_price_cents: plan.overage_cents,
            amount_cents: overage_cents,
        });
    }

    Assembled {
        base_cents: plan.base_cents,
        overage_cents,
        total_cents: plan.base_cents + overage_cents,
        lines,
    }
}

// ---------------------------------------------------------------------------
// Metering
// ---------------------------------------------------------------------------

/// A workspace's current, billable footprint.
#[derive(Clone, Copy, Debug, Default)]
pub struct Meter {
    pub properties: i32,
    pub units: i32,
}

/// Meter a tenant's units under management (the billable quantity) and property
/// count (shown alongside for context).
pub async fn meter(db: &impl ConnectionTrait, tenant_id: Uuid) -> Result<Meter, sea_orm::DbErr> {
    let properties = Property::find()
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?;
    let units: i32 = properties.iter().map(|p| p.units.max(0)).sum();
    Ok(Meter {
        properties: properties.len() as i32,
        units,
    })
}

// ---------------------------------------------------------------------------
// Period helpers
// ---------------------------------------------------------------------------

/// The `YYYY-MM` label of the month before `today`.
pub fn previous_month(today: NaiveDate) -> String {
    let (y, m) = if today.month() == 1 {
        (today.year() - 1, 12)
    } else {
        (today.year(), today.month() - 1)
    };
    format!("{y:04}-{m:02}")
}

/// The `YYYY-MM` label of a date's own month.
fn month_of(date: NaiveDate) -> String {
    format!("{:04}-{:02}", date.year(), date.month())
}

// ---------------------------------------------------------------------------
// Generation + settlement
// ---------------------------------------------------------------------------

/// Generate (or fetch, if it already exists) the `period` invoice for a tenant.
/// Idempotent on `(tenant_id, period)`; issued immediately (status `open`) with
/// a net-15 due date. Priced from the tenant's current plan + live unit meter.
pub async fn generate_invoice(
    db: &impl ConnectionTrait,
    tenant: &entity::tenant::Model,
    period: &str,
) -> Result<entity::platform_invoice::Model, sea_orm::DbErr> {
    if let Some(existing) = PlatformInvoice::find()
        .filter(entity::platform_invoice::Column::TenantId.eq(tenant.id))
        .filter(entity::platform_invoice::Column::Period.eq(period))
        .one(db)
        .await?
    {
        return Ok(existing);
    }

    let plan = plan_for(&tenant.plan);
    let metered = meter(db, tenant.id).await?;
    let assembled = assemble(plan, metered.units);

    let now = Utc::now();
    let due = (now + chrono::Duration::days(15)).date_naive().to_string();
    let invoice = entity::platform_invoice::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant.id),
        period: Set(period.to_string()),
        plan: Set(plan.key.to_string()),
        unit_count: Set(metered.units),
        included_units: Set(plan.included_units),
        base_cents: Set(assembled.base_cents),
        overage_cents: Set(assembled.overage_cents),
        total_cents: Set(assembled.total_cents),
        status: Set("open".into()),
        issued_at: Set(Some(now.into())),
        due_date: Set(Some(due)),
        paid_at: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;

    for (i, line) in assembled.lines.iter().enumerate() {
        entity::platform_invoice_line::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            invoice_id: Set(invoice.id),
            description: Set(line.description.clone()),
            quantity: Set(line.quantity),
            unit_price_cents: Set(line.unit_price_cents),
            amount_cents: Set(line.amount_cents),
            sort_order: Set(i as i32),
            created_at: Set(now.into()),
        }
        .insert(db)
        .await?;
    }

    tracing::info!(
        tenant = %tenant.id, period, plan = plan.key, units = metered.units,
        total = assembled.total_cents, "platform invoice generated"
    );
    Ok(invoice)
}

/// Generate the `period` invoice for every tenant that already existed in that
/// month. Runs on the platform plane (null tenant GUC). Returns the number of
/// invoices that existed or were created.
pub async fn run_for_period(db: &DatabaseConnection, period: &str) -> Result<i64, sea_orm::DbErr> {
    let tenants = Tenant::find().all(db).await?;
    let mut count = 0;
    for tenant in tenants {
        // Don't bill a period before the workspace existed.
        if month_of(tenant.created_at.date_naive()).as_str() > period {
            continue;
        }
        match generate_invoice(db, &tenant, period).await {
            Ok(_) => count += 1,
            Err(e) => tracing::error!("saas: invoice for {} {period} failed: {e}", tenant.id),
        }
    }
    Ok(count)
}

// ---------------------------------------------------------------------------
// Recurring job (core; not a tenant-toggleable module)
// ---------------------------------------------------------------------------

pub const BILLING_KIND: &str = "platform_billing";
/// How long the billing job sleeps between runs (~daily).
const BILLING_INTERVAL_SECS: i64 = 24 * 3600;

/// Ensure every tenant has exactly one live `platform_billing` job. Called at
/// boot and after provisioning; idempotent.
pub async fn ensure_recurring_jobs(db: &DatabaseConnection) {
    let tenants = match Tenant::find().all(db).await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("saas: tenant scan failed: {e}");
            return;
        }
    };
    for tenant in tenants {
        if let Err(e) = ensure_job_for_tenant(db, tenant.id).await {
            tracing::error!("saas: ensure billing job for {} failed: {e}", tenant.id);
        }
    }
}

/// Ensure one live billing job for a single tenant (used by provisioning too).
pub async fn ensure_job_for_tenant(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
) -> Result<(), sea_orm::DbErr> {
    let existing = entity::prelude::BackgroundJob::find()
        .filter(entity::background_job::Column::TenantId.eq(tenant_id))
        .filter(entity::background_job::Column::Kind.eq(BILLING_KIND))
        .filter(entity::background_job::Column::Status.is_in(["pending", "running"]))
        .one(db)
        .await?;
    if existing.is_none() {
        crate::scheduler::enqueue(db, tenant_id, BILLING_KIND, json!({}), 5).await?;
        tracing::info!(tenant = %tenant_id, "platform billing job scheduled");
    }
    Ok(())
}

/// Advance one `platform_billing` job: ensure last month's invoice exists for
/// this tenant, then sleep. Dispatched directly by the scheduler (see
/// [`crate::scheduler`]), so it is never parked by module enablement.
pub async fn handle_billing_job(
    db: &DatabaseConnection,
    job: &entity::background_job::Model,
) -> JobOutcome {
    let tenant_id = job.tenant_id;
    let today = Utc::now().date_naive();
    let period = previous_month(today);

    let mut summary = json!({ "period": period });
    match Tenant::find_by_id(tenant_id).one(db).await {
        Ok(Some(tenant)) => {
            // Skip billing a period that predates the workspace.
            if month_of(tenant.created_at.date_naive()).as_str() <= period.as_str() {
                match generate_invoice(db, &tenant, &period).await {
                    Ok(inv) => {
                        summary["invoice_id"] = json!(inv.id);
                        summary["total_cents"] = json!(inv.total_cents);
                    }
                    Err(e) => tracing::error!("saas: billing job generate failed: {e}"),
                }
            }
        }
        Ok(None) => tracing::warn!("saas: billing job for missing tenant {tenant_id}"),
        Err(e) => tracing::error!("saas: billing job tenant lookup failed: {e}"),
    }

    let mut outcome = JobOutcome::reschedule("pending", BILLING_INTERVAL_SECS);
    outcome.result = Some(summary);
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(key: &str) -> &'static Plan {
        plan_for(key)
    }

    #[test]
    fn base_only_when_under_allowance() {
        // 10 units on Starter (25 included) → base fee, no overage line.
        let a = assemble(plan("starter"), 10);
        assert_eq!(a.base_cents, 4_900);
        assert_eq!(a.overage_cents, 0);
        assert_eq!(a.total_cents, 4_900);
        assert_eq!(a.lines.len(), 1);
    }

    #[test]
    fn overage_charged_beyond_allowance() {
        // 40 units on Starter → 15 over × $2.50 = $37.50 overage.
        let a = assemble(plan("starter"), 40);
        assert_eq!(a.overage_cents, 15 * 250);
        assert_eq!(a.total_cents, 4_900 + 3_750);
        assert_eq!(a.lines.len(), 2);
        assert_eq!(a.lines[1].quantity, 15);
    }

    #[test]
    fn exactly_at_allowance_has_no_overage() {
        let a = assemble(plan("growth"), 100);
        assert_eq!(a.overage_cents, 0);
        assert_eq!(a.total_cents, 19_900);
    }

    #[test]
    fn unknown_plan_falls_back_to_starter() {
        assert_eq!(plan_for("legacy_freebie").key, "starter");
    }

    #[test]
    fn previous_month_wraps_year() {
        let jan = NaiveDate::from_ymd_opt(2026, 1, 9).unwrap();
        assert_eq!(previous_month(jan), "2025-12");
        let jul = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
        assert_eq!(previous_month(jul), "2026-06");
    }
}
