//! The **billing cycle** — the recurring per-tenant automation that keeps the
//! books current (roadmap Phase 3, issues #33/#35/#37/#39). One durable,
//! self-rescheduling `billing_cycle` job per tenant runs every few hours and,
//! idempotently:
//!
//! 1. **raises rent receivables** — on the tenant's rent-due day each active
//!    lease gets its month's `lease_payment` (rent + recurring charges),
//!    accrued to the ledger (`Dr AR / Cr Rental Income`);
//! 2. **assesses late fees** — receivables past the grace period flip to
//!    `late`, the lease drops to late standing, and the configured fee
//!    (flat + percentage, one-time or daily, capped) lands as a
//!    `lease_charge` + payable receivable + ledger entry;
//! 3. **runs autopay** — leases with an enrolled method charge their due rent
//!    through the payment pipeline (a previously failed attempt is never
//!    hammered — the resident pays manually);
//! 4. **syncs bank feeds** — linked accounts refresh roughly daily;
//! 5. **captures the monthly snapshot** — occupancy, delinquency, portfolio
//!    value etc. for the dashboards' history.
//!
//! Every step is guarded by existence checks, so the cycle can run as often
//! as it likes without double-billing anyone.

use crate::modules::JobOutcome;
use chrono::{Datelike, NaiveDate, Utc};
use entity::prelude::{
    BackgroundJob, BankAccount, FinancialSnapshot, Lease, LeaseCharge, LeasePayment, PaymentMethod,
    Property, PropertyValuation, Tenant,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, Set,
};
use serde_json::json;
use uuid::Uuid;

pub const CYCLE_KIND: &str = "billing_cycle";
/// How long the cycle sleeps between runs.
const CYCLE_INTERVAL_SECS: i64 = 6 * 3600;

/// Ensure every tenant has exactly one live `billing_cycle` job. Called at
/// boot (and after tenant provisioning); idempotent.
pub async fn ensure_recurring_jobs(db: &DatabaseConnection) {
    let tenants = match Tenant::find().all(db).await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("billing: tenant scan failed: {e}");
            return;
        }
    };
    for tenant in tenants {
        if let Err(e) = ensure_cycle_for_tenant(db, tenant.id).await {
            tracing::error!("billing: ensure cycle for {} failed: {e}", tenant.id);
        }
    }
}

/// Ensure one live cycle job for a single tenant (used by provisioning too).
pub async fn ensure_cycle_for_tenant(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
) -> Result<(), sea_orm::DbErr> {
    let existing = BackgroundJob::find()
        .filter(entity::background_job::Column::TenantId.eq(tenant_id))
        .filter(entity::background_job::Column::Kind.eq(CYCLE_KIND))
        .filter(entity::background_job::Column::Status.is_in([
            "pending",
            "running",
            "awaiting_callback",
        ]))
        .one(db)
        .await?;
    if existing.is_none() {
        crate::scheduler::enqueue(db, tenant_id, CYCLE_KIND, json!({}), 5).await?;
        tracing::info!(tenant = %tenant_id, "billing cycle scheduled");
    }
    Ok(())
}

/// Advance one `billing_cycle` job, then go back to sleep.
pub async fn handle_cycle_job(
    db: &DatabaseConnection,
    job: &entity::background_job::Model,
) -> JobOutcome {
    let tenant_id = job.tenant_id;
    let today = Utc::now().date_naive();

    let mut summary = json!({});
    match raise_rent_receivables(db, tenant_id, today).await {
        Ok(n) => summary["receivables_raised"] = json!(n),
        Err(e) => tracing::error!("billing: receivables failed: {e}"),
    }
    match assess_late_fees(db, tenant_id, today).await {
        Ok(n) => summary["late_fees_assessed"] = json!(n),
        Err(e) => tracing::error!("billing: late fees failed: {e}"),
    }
    match run_autopay(db, tenant_id, today).await {
        Ok(n) => summary["autopay_started"] = json!(n),
        Err(e) => tracing::error!("billing: autopay failed: {e}"),
    }
    match sync_bank_feeds(db, tenant_id).await {
        Ok(n) => summary["bank_syncs_enqueued"] = json!(n),
        Err(e) => tracing::error!("billing: bank feed scan failed: {e}"),
    }
    if let Err(e) = capture_snapshot(db, tenant_id, today).await {
        tracing::error!("billing: snapshot failed: {e}");
    }

    tracing::info!(tenant = %tenant_id, ?summary, "billing cycle ran");
    // Persist the run summary and sleep. `reschedule` is a state step, not a
    // retry, so the cycle never exhausts a budget.
    let mut outcome = JobOutcome::reschedule("pending", CYCLE_INTERVAL_SECS);
    outcome.result = Some(summary);
    outcome
}

/// The date rent falls due in `month` given the tenant's due-day setting.
fn due_date_for_month(year: i32, month: u32, due_day: i64) -> NaiveDate {
    let day = due_day.clamp(1, 28) as u32;
    NaiveDate::from_ymd_opt(year, month, day).expect("day clamped to 28 is always valid")
}

/// The month's recurring amount for a lease: base rent plus signed recurring
/// charges (garage fees, discounts …), never below zero.
fn monthly_amount(rent_cents: i64, charges: &[entity::lease_charge::Model]) -> i64 {
    let extras: i64 = charges
        .iter()
        .filter(|c| c.recurring)
        .map(|c| crate::routes::lease_charges::signed_amount(&c.kind, c.amount_cents))
        .sum();
    (rent_cents + extras).max(0)
}

/// Step 1: raise this month's rent receivable for every active lease.
async fn raise_rent_receivables(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    today: NaiveDate,
) -> Result<i64, sea_orm::DbErr> {
    let due_day = crate::settings::get_i64(db, tenant_id, crate::settings::PAYMENTS_RENT_DUE_DAY)
        .await
        .clamp(1, 28);
    let due = due_date_for_month(today.year(), today.month(), due_day);
    if today < due {
        return Ok(0);
    }
    let due_str = due.to_string();

    let leases = Lease::find()
        .filter(entity::lease::Column::TenantId.eq(tenant_id))
        .filter(entity::lease::Column::Status.eq("active"))
        .all(db)
        .await?;
    let mut raised = 0;
    for lease in leases {
        // Leases starting later this month wait for next month's cycle.
        if lease.start_date.as_str() > due_str.as_str() {
            continue;
        }
        let existing = LeasePayment::find()
            .filter(entity::lease_payment::Column::TenantId.eq(tenant_id))
            .filter(entity::lease_payment::Column::LeaseId.eq(lease.id))
            .filter(entity::lease_payment::Column::Kind.eq(crate::payments::KIND_RENT))
            .filter(entity::lease_payment::Column::DueDate.eq(due_str.clone()))
            .one(db)
            .await?;
        if existing.is_some() {
            continue;
        }
        let charges = LeaseCharge::find()
            .filter(entity::lease_charge::Column::TenantId.eq(tenant_id))
            .filter(entity::lease_charge::Column::LeaseId.eq(lease.id))
            .all(db)
            .await?;
        let amount = monthly_amount(lease.rent_cents, &charges);
        if amount <= 0 {
            continue;
        }
        let now = Utc::now();
        let payment = entity::lease_payment::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            lease_id: Set(lease.id),
            due_date: Set(due_str.clone()),
            amount_cents: Set(amount),
            paid_date: Set(None),
            status: Set("due".into()),
            method: Set(None),
            created_at: Set(now.into()),
            kind: Set(crate::payments::KIND_RENT.into()),
            method_id: Set(None),
            provider: Set(None),
            external_id: Set(None),
            failure_reason: Set(None),
            receipt_number: Set(None),
            ledger_txn_id: Set(None),
        }
        .insert(db)
        .await?;

        // The receivable joins the lease's outstanding balance…
        let mut lam: entity::lease::ActiveModel = lease.clone().into();
        lam.balance_cents = Set(lease.balance_cents + amount);
        if lease.payment_status == "current" {
            lam.payment_status = Set("partial".into());
        }
        lam.updated_at = Set(now.into());
        lam.update(db).await?;

        // …and accrues on the entity's books.
        if let Some(entity_id) =
            crate::payments::entity_for_property(db, tenant_id, lease.property_id).await
        {
            if let Err(e) = crate::accounting::post_rent_due(
                db,
                tenant_id,
                entity_id,
                Some(lease.property_id),
                lease.id,
                &due_str,
                amount,
                payment.id,
            )
            .await
            {
                tracing::error!("billing: rent accrual post failed: {e}");
            }
        }
        raised += 1;
    }
    Ok(raised)
}

/// The late fee owed on an overdue amount under the tenant's policy. Pure.
pub fn late_fee_amount(overdue_cents: i64, flat_cents: i64, percent_bps: i64) -> i64 {
    if overdue_cents <= 0 {
        return 0;
    }
    flat_cents.max(0) + (overdue_cents * percent_bps.max(0)) / 10_000
}

/// Step 2: flip overdue receivables to `late` and assess the configured fee.
async fn assess_late_fees(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    today: NaiveDate,
) -> Result<i64, sea_orm::DbErr> {
    let grace = crate::settings::get_i64(db, tenant_id, crate::settings::LATE_FEE_GRACE_DAYS).await;
    let flat = crate::settings::get_i64(db, tenant_id, crate::settings::LATE_FEE_FLAT_CENTS).await;
    let bps = crate::settings::get_i64(db, tenant_id, crate::settings::LATE_FEE_PERCENT_BPS).await;
    let recurrence =
        crate::settings::get_string(db, tenant_id, crate::settings::LATE_FEE_RECURRENCE).await;
    let cap = crate::settings::get_i64(db, tenant_id, crate::settings::LATE_FEE_MAX_CENTS).await;

    let overdue = LeasePayment::find()
        .filter(entity::lease_payment::Column::TenantId.eq(tenant_id))
        .filter(entity::lease_payment::Column::Kind.eq(crate::payments::KIND_RENT))
        .filter(entity::lease_payment::Column::Status.is_in(["due", "late"]))
        .all(db)
        .await?;

    let mut assessed = 0;
    for payment in overdue {
        let Ok(due) = NaiveDate::parse_from_str(&payment.due_date, "%Y-%m-%d") else {
            continue;
        };
        let days_late = (today - due).num_days();
        if grace <= 0 || days_late <= grace {
            continue;
        }

        // Overdue past grace: the receivable and its lease go late.
        if payment.status != "late" {
            let mut am: entity::lease_payment::ActiveModel = payment.clone().into();
            am.status = Set("late".into());
            am.update(db).await?;
        }
        let Some(lease) = Lease::find_by_id(payment.lease_id)
            .filter(entity::lease::Column::TenantId.eq(tenant_id))
            .one(db)
            .await?
        else {
            continue;
        };
        if lease.payment_status != "late" {
            let mut lam: entity::lease::ActiveModel = lease.clone().into();
            lam.payment_status = Set("late".into());
            lam.updated_at = Set(Utc::now().into());
            lam.update(db).await?;
        }

        // Fee recurrence + cap, keyed on the overdue period (its due date).
        let period_tag = format!("period:{}", payment.due_date);
        let existing_fees: Vec<entity::lease_charge::Model> = LeaseCharge::find()
            .filter(entity::lease_charge::Column::TenantId.eq(tenant_id))
            .filter(entity::lease_charge::Column::LeaseId.eq(payment.lease_id))
            .filter(entity::lease_charge::Column::Code.eq("late_fee"))
            .all(db)
            .await?
            .into_iter()
            .filter(|c| {
                c.verbiage
                    .as_deref()
                    .map(|v| v.contains(&period_tag))
                    .unwrap_or(false)
            })
            .collect();
        let already_cents: i64 = existing_fees.iter().map(|c| c.amount_cents).sum();
        let today_str = today.to_string();
        let applied_today = existing_fees
            .iter()
            .any(|c| c.created_at.to_rfc3339().starts_with(&today_str));
        let should_apply = match recurrence.as_str() {
            "daily" => !applied_today,
            _ => existing_fees.is_empty(),
        };
        if !should_apply {
            continue;
        }
        let mut fee = late_fee_amount(payment.amount_cents, flat, bps);
        if cap > 0 {
            fee = fee.min(cap - already_cents);
        }
        if fee <= 0 {
            continue;
        }

        // The fee lands three ways: the lease_charge line item that documents
        // its origin, a payable receivable, and the ledger accrual.
        let now = Utc::now();
        let month_label = due.format("%B %Y").to_string();
        let charge = entity::lease_charge::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            lease_id: Set(payment.lease_id),
            kind: Set("fee".into()),
            code: Set(Some("late_fee".into())),
            label: Set(format!("Late fee — {month_label}")),
            amount_cents: Set(fee),
            recurring: Set(false),
            source: Set("auto".into()),
            verbiage: Set(Some(format!(
                "Assessed {today_str}: rent due {} unpaid {days_late} days \
                 (grace {grace}). {period_tag}",
                payment.due_date
            ))),
            created_at: Set(now.into()),
        }
        .insert(db)
        .await?;

        let fee_receivable = entity::lease_payment::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            lease_id: Set(payment.lease_id),
            due_date: Set(today_str.clone()),
            amount_cents: Set(fee),
            paid_date: Set(None),
            status: Set("due".into()),
            method: Set(None),
            created_at: Set(now.into()),
            kind: Set(crate::payments::KIND_FEE.into()),
            method_id: Set(None),
            provider: Set(None),
            external_id: Set(None),
            failure_reason: Set(None),
            receipt_number: Set(None),
            ledger_txn_id: Set(None),
        }
        .insert(db)
        .await?;

        let mut lam: entity::lease::ActiveModel = lease.clone().into();
        lam.balance_cents = Set(lease.balance_cents + fee);
        lam.updated_at = Set(now.into());
        lam.update(db).await?;

        if let Some(entity_id) =
            crate::payments::entity_for_property(db, tenant_id, lease.property_id).await
        {
            if let Err(e) = crate::accounting::post_late_fee(
                db,
                tenant_id,
                entity_id,
                Some(lease.property_id),
                lease.id,
                &today_str,
                fee,
                fee_receivable.id,
            )
            .await
            {
                tracing::error!("billing: late fee post failed: {e}");
            }
        }

        crate::audit::record(
            db,
            None,
            crate::audit::actions::LATE_FEE_APPLY,
            Some("lease_charge"),
            Some(charge.id.to_string()),
            Some(tenant_id),
            Some(json!({
                "lease_id": payment.lease_id,
                "amount_cents": fee,
                "overdue_payment_id": payment.id,
                "days_late": days_late,
                "recurrence": recurrence,
            })),
        )
        .await;

        if let Some(email) = lease.tenant_email.as_deref().filter(|e| !e.is_empty()) {
            let payload = json!({
                "template": "late_fee_applied",
                "to": email,
                "owner_type": "lease_charge",
                "owner_id": charge.id,
                "trigger": "assessed",
                "vars": {
                    "amount": crate::dto::usd(fee),
                    "month": month_label,
                },
            });
            let _ = crate::scheduler::enqueue(db, tenant_id, "auto_email", payload, 0).await;
        }
        assessed += 1;
    }
    Ok(assessed)
}

/// Step 3: charge due rent through each lease's enrolled autopay method.
async fn run_autopay(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    today: NaiveDate,
) -> Result<i64, sea_orm::DbErr> {
    if !crate::settings::get_bool(db, tenant_id, crate::settings::PAYMENTS_AUTOPAY_ENABLED).await {
        return Ok(0);
    }
    let methods = PaymentMethod::find()
        .filter(entity::payment_method::Column::TenantId.eq(tenant_id))
        .filter(entity::payment_method::Column::Autopay.eq(true))
        .filter(entity::payment_method::Column::Status.eq("active"))
        .all(db)
        .await?;

    let mut started = 0;
    for method in methods {
        let Some(lease_id) = method.lease_id else {
            continue;
        };
        // Autopay waits for its enrollment day (or the due date, whichever is
        // later) so residents control when the charge lands.
        if let Some(day) = method.autopay_day {
            if (today.day() as i32) < day.clamp(1, 28) {
                continue;
            }
        }
        let due_items = LeasePayment::find()
            .filter(entity::lease_payment::Column::TenantId.eq(tenant_id))
            .filter(entity::lease_payment::Column::LeaseId.eq(lease_id))
            .filter(entity::lease_payment::Column::Kind.eq(crate::payments::KIND_RENT))
            .filter(entity::lease_payment::Column::Status.is_in(["due", "late"]))
            .all(db)
            .await?;
        for item in due_items {
            if item.due_date > today.to_string() {
                continue;
            }
            // Never hammer a method that already failed this receivable.
            if item.failure_reason.is_some() {
                continue;
            }
            match crate::payments::start_charge(db, tenant_id, item, &method, None).await {
                Ok(_) => started += 1,
                Err(e) => tracing::warn!("billing: autopay start failed: {e}"),
            }
        }
    }
    Ok(started)
}

/// Step 4: refresh linked bank feeds roughly daily.
async fn sync_bank_feeds(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
) -> Result<i64, sea_orm::DbErr> {
    let accounts = BankAccount::find()
        .filter(entity::bank_account::Column::TenantId.eq(tenant_id))
        .filter(entity::bank_account::Column::ExternalId.is_not_null())
        .all(db)
        .await?;
    let mut enqueued = 0;
    let cutoff = Utc::now() - chrono::Duration::hours(20);
    for account in accounts {
        let stale = account
            .last_synced_at
            .map(|t| t.with_timezone(&Utc) < cutoff)
            .unwrap_or(true);
        if !stale {
            continue;
        }
        crate::scheduler::enqueue(
            db,
            tenant_id,
            "bank_feed_sync",
            json!({ "bank_account_id": account.id }),
            0,
        )
        .await?;
        enqueued += 1;
    }
    Ok(enqueued)
}

/// Step 5: upsert this month's [`entity::financial_snapshot`].
pub async fn capture_snapshot(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    today: NaiveDate,
) -> Result<(), sea_orm::DbErr> {
    let month = today.format("%Y-%m").to_string();
    let metrics = compute_point_in_time(db, tenant_id).await?;
    let (rent_due, rent_collected) = month_rent_figures(db, tenant_id, &month).await?;
    let noi = crate::finance::month_noi(db, tenant_id, &month)
        .await
        .unwrap_or(0);

    let now = Utc::now();
    match FinancialSnapshot::find()
        .filter(entity::financial_snapshot::Column::TenantId.eq(tenant_id))
        .filter(entity::financial_snapshot::Column::Month.eq(month.clone()))
        .one(db)
        .await?
    {
        Some(row) => {
            let mut am: entity::financial_snapshot::ActiveModel = row.into();
            am.occupancy_bps = Set(metrics.occupancy_bps);
            am.delinquency_bps = Set(metrics.delinquency_bps);
            am.portfolio_value_cents = Set(metrics.portfolio_value_cents);
            am.rent_due_cents = Set(rent_due);
            am.rent_collected_cents = Set(rent_collected);
            am.noi_cents = Set(noi);
            am.active_leases = Set(metrics.active_leases);
            am.updated_at = Set(now.into());
            am.update(db).await?;
        }
        None => {
            entity::financial_snapshot::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                month: Set(month),
                occupancy_bps: Set(metrics.occupancy_bps),
                delinquency_bps: Set(metrics.delinquency_bps),
                portfolio_value_cents: Set(metrics.portfolio_value_cents),
                rent_due_cents: Set(rent_due),
                rent_collected_cents: Set(rent_collected),
                noi_cents: Set(noi),
                active_leases: Set(metrics.active_leases),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }
            .insert(db)
            .await?;
        }
    }
    Ok(())
}

/// Point-in-time portfolio metrics (the parts history can't reconstruct).
pub struct PointInTime {
    pub occupancy_bps: i32,
    pub delinquency_bps: i32,
    pub portfolio_value_cents: i64,
    pub active_leases: i32,
}

pub async fn compute_point_in_time(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
) -> Result<PointInTime, sea_orm::DbErr> {
    let properties = Property::find()
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?;
    let units: i64 = properties.iter().map(|p| p.units as i64).sum();
    let occupied: i64 = properties.iter().map(|p| p.occupied_units as i64).sum();
    let occupancy_bps = if units > 0 {
        ((occupied * 10_000) / units) as i32
    } else {
        0
    };

    // Portfolio value: the latest AVM estimate per property, falling back to
    // purchase price.
    let mut portfolio_value: i64 = 0;
    for p in &properties {
        let valuation = PropertyValuation::find()
            .filter(entity::property_valuation::Column::TenantId.eq(tenant_id))
            .filter(entity::property_valuation::Column::PropertyId.eq(p.id))
            .order_by_desc(entity::property_valuation::Column::AsOf)
            .one(db)
            .await?
            .and_then(|v| v.estimated_value_cents);
        portfolio_value += valuation.or(p.purchase_price_cents).unwrap_or(0);
    }

    let leases = Lease::find()
        .filter(entity::lease::Column::TenantId.eq(tenant_id))
        .filter(entity::lease::Column::Status.eq("active"))
        .all(db)
        .await?;
    let active = leases.len() as i64;
    let late = leases.iter().filter(|l| l.payment_status == "late").count() as i64;
    let delinquency_bps = if active > 0 {
        ((late * 10_000) / active) as i32
    } else {
        0
    };

    Ok(PointInTime {
        occupancy_bps,
        delinquency_bps,
        portfolio_value_cents: portfolio_value,
        active_leases: active as i32,
    })
}

/// Rent due vs collected for one `YYYY-MM` month, from the payments table.
pub async fn month_rent_figures(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    month: &str,
) -> Result<(i64, i64), sea_orm::DbErr> {
    let prefix = format!("{month}-");
    let due: i64 = LeasePayment::find()
        .filter(entity::lease_payment::Column::TenantId.eq(tenant_id))
        .filter(entity::lease_payment::Column::Kind.ne(crate::payments::KIND_DEPOSIT))
        .filter(entity::lease_payment::Column::DueDate.starts_with(&prefix))
        .all(db)
        .await?
        .iter()
        .map(|p| p.amount_cents)
        .sum();
    let collected: i64 = LeasePayment::find()
        .filter(entity::lease_payment::Column::TenantId.eq(tenant_id))
        .filter(entity::lease_payment::Column::Kind.ne(crate::payments::KIND_DEPOSIT))
        .filter(entity::lease_payment::Column::Status.eq("paid"))
        .filter(entity::lease_payment::Column::PaidDate.starts_with(&prefix))
        .all(db)
        .await?
        .iter()
        .map(|p| p.amount_cents)
        .sum();
    Ok((due, collected))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn charge(kind: &str, amount: i64, recurring: bool) -> entity::lease_charge::Model {
        entity::lease_charge::Model {
            id: Uuid::from_u128(1),
            tenant_id: Uuid::from_u128(2),
            lease_id: Uuid::from_u128(3),
            kind: kind.into(),
            code: None,
            label: "x".into(),
            amount_cents: amount,
            recurring,
            source: "manual".into(),
            verbiage: None,
            created_at: chrono::Utc::now().into(),
        }
    }

    #[test]
    fn monthly_amount_adds_recurring_signed_charges() {
        // $1,850 rent + $150 garage − $100 military discount; a one-time fee
        // is excluded.
        let charges = vec![
            charge("amenity", 15_000, true),
            charge("discount", 10_000, true),
            charge("fee", 99_900, false),
        ];
        assert_eq!(monthly_amount(185_000, &charges), 190_000);
    }

    #[test]
    fn monthly_amount_never_negative() {
        let charges = vec![charge("discount", 999_999, true)];
        assert_eq!(monthly_amount(185_000, &charges), 0);
    }

    #[test]
    fn late_fee_combines_flat_and_percent() {
        // $75 flat + 5% of $1,620 = $75 + $81 = $156.
        assert_eq!(late_fee_amount(162_000, 7_500, 500), 15_600);
        // Flat only.
        assert_eq!(late_fee_amount(162_000, 7_500, 0), 7_500);
        // Percent only.
        assert_eq!(late_fee_amount(162_000, 0, 500), 8_100);
        // Nothing overdue, nothing owed.
        assert_eq!(late_fee_amount(0, 7_500, 500), 0);
        // Negative config never produces a negative fee.
        assert_eq!(late_fee_amount(162_000, -5, -5), 0);
    }

    #[test]
    fn due_date_clamps_to_safe_days() {
        let d = due_date_for_month(2026, 2, 31);
        assert_eq!(d.to_string(), "2026-02-28");
        let d = due_date_for_month(2026, 7, 1);
        assert_eq!(d.to_string(), "2026-07-01");
    }
}
