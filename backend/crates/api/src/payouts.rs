//! **Owner payouts / draws** (roadmap Phase 3, issue #38) — the loop from
//! "rent collected" to "owner got paid".
//!
//! A payout is computed from one entity's actual books for a period: rent
//! collected (settled payments on the entity's properties) minus operating
//! expenses posted to its ledger minus the configured management fee.
//! Executing it rides the payments provider as an ACH transfer (sandbox by
//! default), and settlement — webhook-driven live, immediate in simulation —
//! posts the draw to the ledger (`Dr Owner Draws + Dr Management Fees / Cr
//! Operating Bank`) and stores a generated statement PDF against the entity.

use crate::error::{ApiError, ApiResult};
use crate::modules::JobOutcome;
use crate::providers::payments::{PaymentsRequest, StripeProvider};
use crate::providers::ProviderCtx;
use crate::storage::ObjectStore;
use chrono::Utc;
use entity::prelude::{Lease, LeasePayment, Llc, OwnerPayout, Property};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    Set,
};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

/// The computed inputs for one payout period.
pub struct PayoutComputation {
    pub rent_collected_cents: i64,
    pub expenses_cents: i64,
    pub mgmt_fee_cents: i64,
    pub net_cents: i64,
}

/// Pure arithmetic: management fee from collected rent, net from the rest.
pub fn compute_amounts(
    rent_collected_cents: i64,
    expenses_cents: i64,
    mgmt_fee_bps: i64,
) -> PayoutComputation {
    let mgmt_fee_cents = (rent_collected_cents * mgmt_fee_bps.max(0)) / 10_000;
    PayoutComputation {
        rent_collected_cents,
        expenses_cents,
        mgmt_fee_cents,
        net_cents: rent_collected_cents - expenses_cents - mgmt_fee_cents,
    }
}

/// Compute a draft payout for `(entity, period)` from settled payments and
/// the entity's expense ledger.
pub async fn compute_payout(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    entity_id: Uuid,
    period_start: &str,
    period_end: &str,
    created_by: Option<Uuid>,
) -> ApiResult<entity::owner_payout::Model> {
    // The entity's properties → their leases → settled payments in-period.
    let properties = Property::find()
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .filter(entity::property::Column::LlcId.eq(entity_id))
        .all(db)
        .await?;
    let property_ids: Vec<Uuid> = properties.iter().map(|p| p.id).collect();
    let mut rent_collected: i64 = 0;
    if !property_ids.is_empty() {
        let leases = Lease::find()
            .filter(entity::lease::Column::TenantId.eq(tenant_id))
            .filter(entity::lease::Column::PropertyId.is_in(property_ids))
            .all(db)
            .await?;
        let lease_ids: Vec<Uuid> = leases.iter().map(|l| l.id).collect();
        if !lease_ids.is_empty() {
            rent_collected = LeasePayment::find()
                .filter(entity::lease_payment::Column::TenantId.eq(tenant_id))
                .filter(entity::lease_payment::Column::LeaseId.is_in(lease_ids))
                .filter(entity::lease_payment::Column::Status.eq("paid"))
                .filter(entity::lease_payment::Column::Kind.ne(crate::payments::KIND_DEPOSIT))
                .filter(entity::lease_payment::Column::PaidDate.gte(period_start))
                .filter(entity::lease_payment::Column::PaidDate.lte(period_end))
                .all(db)
                .await?
                .iter()
                .map(|p| p.amount_cents)
                .sum();
        }
    }

    // Operating expenses actually posted to the entity's books in-period —
    // excluding management fees (this payout is about to charge its own).
    let activity = crate::accounting::account_activity(
        db,
        tenant_id,
        entity_id,
        Some(period_start),
        Some(period_end),
    )
    .await?;
    let expenses: i64 = activity
        .iter()
        .filter(|a| {
            a.account.kind == "expense"
                && a.account.subtype.as_deref()
                    != Some(crate::accounting::subtypes::MANAGEMENT_FEES)
        })
        .map(|a| a.balance_cents())
        .sum();

    let mgmt_fee_bps =
        crate::settings::get_i64(db, tenant_id, crate::settings::PAYOUT_MGMT_FEE_BPS).await;
    let amounts = compute_amounts(rent_collected, expenses, mgmt_fee_bps);

    let now = Utc::now();
    let payout = entity::owner_payout::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        entity_id: Set(entity_id),
        period_start: Set(period_start.to_string()),
        period_end: Set(period_end.to_string()),
        rent_collected_cents: Set(amounts.rent_collected_cents),
        expenses_cents: Set(amounts.expenses_cents),
        mgmt_fee_cents: Set(amounts.mgmt_fee_cents),
        net_cents: Set(amounts.net_cents),
        status: Set("draft".into()),
        provider: Set(None),
        external_id: Set(None),
        statement_document_id: Set(None),
        ledger_txn_id: Set(None),
        failure_reason: Set(None),
        created_by: Set(created_by),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;

    crate::audit::record(
        db,
        created_by,
        crate::audit::actions::PAYOUT_CREATE,
        Some("owner_payout"),
        Some(payout.id.to_string()),
        Some(tenant_id),
        Some(json!({
            "entity_id": entity_id,
            "period_start": period_start,
            "period_end": period_end,
            "net_cents": payout.net_cents,
        })),
    )
    .await;

    Ok(payout)
}

/// Kick a draft payout into execution (route-level: caller already verified
/// permissions and ownership).
pub async fn execute_payout(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    payout: entity::owner_payout::Model,
    executed_by: Uuid,
) -> ApiResult<entity::owner_payout::Model> {
    if payout.status != "draft" && payout.status != "failed" {
        return Err(ApiError::BadRequest(format!(
            "payout is not executable (status: {})",
            payout.status
        )));
    }
    if payout.net_cents <= 0 {
        return Err(ApiError::BadRequest(
            "payout net amount must be positive to execute".into(),
        ));
    }
    let id = payout.id;
    let mut am: entity::owner_payout::ActiveModel = payout.into();
    am.status = Set("processing".into());
    am.failure_reason = Set(None);
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(db).await?;

    crate::scheduler::enqueue(
        db,
        tenant_id,
        "payout_execute",
        json!({ "payout_id": id }),
        0,
    )
    .await?;

    crate::audit::record(
        db,
        Some(executed_by),
        crate::audit::actions::PAYOUT_EXECUTE,
        Some("owner_payout"),
        Some(id.to_string()),
        Some(tenant_id),
        Some(json!({ "net_cents": saved.net_cents })),
    )
    .await;
    Ok(saved)
}

/// Advance one `payout_execute` job: transfer on the first pass, then settle
/// (simulated) or wait for the payout webhook (live).
pub async fn handle_payout_job(
    db: &DatabaseConnection,
    job: &entity::background_job::Model,
) -> JobOutcome {
    let Some(payout_id) = job
        .payload
        .get("payout_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    else {
        return JobOutcome::failed("payout_execute payload missing payout_id");
    };
    let payout = match OwnerPayout::find_by_id(payout_id)
        .filter(entity::owner_payout::Column::TenantId.eq(job.tenant_id))
        .one(db)
        .await
    {
        Ok(Some(p)) => p,
        Ok(None) => return JobOutcome::failed("payout not found"),
        Err(e) => {
            return JobOutcome::retry(
                crate::providers::backoff(job.attempts),
                format!("db error: {e}"),
            )
        }
    };
    if matches!(payout.status.as_str(), "paid" | "failed") {
        return JobOutcome::completed(json!({ "already_settled": payout.status }));
    }

    if payout.external_id.is_none() {
        let ctx = ProviderCtx::new(db, job.tenant_id);
        let req = PaymentsRequest::Payout {
            reference: payout.id,
            amount_cents: payout.net_cents,
            description: format!("Owner draw {} — {}", payout.period_start, payout.period_end),
        };
        let resp = match crate::providers::run(&StripeProvider, &ctx, job, &req).await {
            Ok(resp) => resp,
            Err(outcome) => return outcome,
        };
        let mut am: entity::owner_payout::ActiveModel = payout.clone().into();
        am.provider = Set(Some("stripe".into()));
        am.external_id = Set(Some(resp.external_id.clone()));
        am.updated_at = Set(Utc::now().into());
        if let Err(e) = am.update(db).await {
            return JobOutcome::retry(
                crate::providers::backoff(job.attempts),
                format!("db error: {e}"),
            );
        }
        match resp.status.as_str() {
            "succeeded" => {
                settle_payout(db, job.tenant_id, payout.id, true, None).await;
                return JobOutcome::completed(json!({ "settled": "paid" }));
            }
            "failed" => {
                settle_payout(db, job.tenant_id, payout.id, false, resp.failure_reason).await;
                return JobOutcome::completed(json!({ "settled": "failed" }));
            }
            _ => {
                if crate::providers::is_live("stripe") {
                    return JobOutcome::reschedule("awaiting_callback", 600);
                }
                let delay = crate::settings::get_i64(
                    db,
                    job.tenant_id,
                    crate::settings::PAYMENTS_CALLBACK_DELAY_SECS,
                )
                .await
                .clamp(1, 3600);
                return JobOutcome::reschedule("awaiting_callback", delay);
            }
        }
    }

    if crate::providers::is_live("stripe") {
        // The webhook settles; this job just stands watch briefly.
        return JobOutcome::completed(json!({ "handed_off": "webhook" }));
    }
    settle_payout(db, job.tenant_id, payout.id, true, None).await;
    JobOutcome::completed(json!({ "settled": "paid", "simulated": true }))
}

/// Settle a payout found by provider id (webhook path).
pub async fn settle_by_external_id(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    external_id: &str,
    reference: Option<Uuid>,
    success: bool,
    failure_reason: Option<String>,
) {
    let mut payout = None;
    if !external_id.is_empty() {
        payout = OwnerPayout::find()
            .filter(entity::owner_payout::Column::TenantId.eq(tenant_id))
            .filter(entity::owner_payout::Column::ExternalId.eq(external_id))
            .one(db)
            .await
            .ok()
            .flatten();
    }
    if payout.is_none() {
        if let Some(id) = reference {
            payout = OwnerPayout::find_by_id(id)
                .filter(entity::owner_payout::Column::TenantId.eq(tenant_id))
                .one(db)
                .await
                .ok()
                .flatten();
        }
    }
    match payout {
        Some(p) => settle_payout(db, tenant_id, p.id, success, failure_reason).await,
        None => tracing::warn!("payout webhook matched nothing (id {external_id})"),
    }
}

/// The single payout settlement path: terminal status, ledger posting,
/// statement PDF, audit, staff notification. Idempotent.
pub async fn settle_payout(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    payout_id: Uuid,
    success: bool,
    failure_reason: Option<String>,
) {
    let Ok(Some(payout)) = OwnerPayout::find_by_id(payout_id)
        .filter(entity::owner_payout::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
    else {
        tracing::error!("settle_payout: payout {payout_id} not found");
        return;
    };
    if matches!(payout.status.as_str(), "paid" | "failed") {
        return;
    }
    let now = Utc::now();

    if !success {
        let reason = failure_reason.unwrap_or_else(|| "payout failed".into());
        let mut am: entity::owner_payout::ActiveModel = payout.clone().into();
        am.status = Set("failed".into());
        am.failure_reason = Set(Some(reason.clone()));
        am.updated_at = Set(now.into());
        if let Err(e) = am.update(db).await {
            tracing::error!("settle_payout: update failed: {e}");
            return;
        }
        crate::audit::record(
            db,
            None,
            crate::audit::actions::PAYOUT_SETTLE,
            Some("owner_payout"),
            Some(payout.id.to_string()),
            Some(tenant_id),
            Some(json!({ "status": "failed", "reason": reason })),
        )
        .await;
        return;
    }

    // Ledger: the draw + management fee leave operating cash.
    let today = now.date_naive().to_string();
    let mut ledger_txn_id = None;
    match crate::accounting::post_payout(
        db,
        tenant_id,
        payout.entity_id,
        &today,
        payout.net_cents,
        payout.mgmt_fee_cents,
        payout.id,
    )
    .await
    {
        Ok(txn) => ledger_txn_id = Some(txn.id),
        Err(e) => tracing::error!("settle_payout: ledger post failed: {e}"),
    }

    // Statement PDF against the entity (best-effort).
    let statement_document_id = match store_statement(db, tenant_id, &payout, &today).await {
        Ok(id) => Some(id),
        Err(e) => {
            tracing::error!("settle_payout: statement store failed: {e}");
            None
        }
    };

    let mut am: entity::owner_payout::ActiveModel = payout.clone().into();
    am.status = Set("paid".into());
    am.ledger_txn_id = Set(ledger_txn_id);
    am.statement_document_id = Set(statement_document_id);
    am.updated_at = Set(now.into());
    if let Err(e) = am.update(db).await {
        tracing::error!("settle_payout: update failed: {e}");
        return;
    }

    crate::audit::record(
        db,
        None,
        crate::audit::actions::PAYOUT_SETTLE,
        Some("owner_payout"),
        Some(payout.id.to_string()),
        Some(tenant_id),
        Some(json!({
            "status": "paid",
            "net_cents": payout.net_cents,
            "ledger_txn_id": ledger_txn_id,
            "statement_document_id": statement_document_id,
        })),
    )
    .await;

    crate::notify::notify_staff(
        db,
        tenant_id,
        "payout:manage",
        "payout_paid",
        json!({ "amount": crate::dto::usd(payout.net_cents) }),
        Some(("owner_payout", payout.id)),
        "settled",
        None,
    )
    .await;
}

/// Render + store the owner statement PDF against the entity.
async fn store_statement(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    payout: &entity::owner_payout::Model,
    paid_date: &str,
) -> anyhow::Result<Uuid> {
    let llc = Llc::find_by_id(payout.entity_id)
        .filter(entity::llc::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?;
    let entity_name = llc.map(|l| l.name).unwrap_or_else(|| "Entity".into());
    let text = statement_text(&entity_name, payout, paid_date);
    let bytes = crate::pdf::text_to_pdf(&text);

    let id = Uuid::new_v4();
    let storage_key = format!("{tenant_id}/{id}");
    let store = ObjectStore::from_env()?;
    store.put_bytes(&storage_key, &bytes).await?;

    let now = Utc::now();
    entity::document::ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant_id),
        owner_type: Set("entity".into()),
        owner_id: Set(payout.entity_id),
        filename: Set(format!(
            "owner-statement-{}-{}.pdf",
            payout.period_start, payout.period_end
        )),
        category: Set(Some("statement".into())),
        requires_wet_ink: Set(false),
        physical_location: Set(None),
        mime_type: Set("application/pdf".into()),
        size_bytes: Set(bytes.len() as i64),
        checksum: Set(Some(crate::storage::sha256_hex(&bytes))),
        version: Set(1),
        previous_version_id: Set(None),
        storage_key: Set(storage_key),
        status: Set("stored".into()),
        retention_expires_at: Set(None),
        created_by: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    Ok(id)
}

/// The rendered owner-statement body.
pub fn statement_text(
    entity_name: &str,
    payout: &entity::owner_payout::Model,
    paid_date: &str,
) -> String {
    let usd = crate::dto::usd;
    format!(
        "OWNER STATEMENT\n\
         ================================\n\n\
         Entity:            {entity_name}\n\
         Period:            {} to {}\n\
         Statement date:    {paid_date}\n\n\
         SUMMARY\n\
         --------------------------------\n\
         Rent collected:    {}\n\
         Operating expenses: -{}\n\
         Management fee:    -{}\n\
         --------------------------------\n\
         Net owner draw:    {}\n\n\
         The net draw was transferred by ACH to the entity's operating owner \n\
         account. Figures are drawn from the entity's double-entry ledger; \n\
         the corresponding journal entry is referenced on the payout record.",
        payout.period_start,
        payout.period_end,
        usd(payout.rent_collected_cents),
        usd(payout.expenses_cents),
        usd(payout.mgmt_fee_cents),
        usd(payout.net_cents),
    )
}

/// Names for entities, for list endpoints.
pub async fn entity_names(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
) -> Result<HashMap<Uuid, String>, sea_orm::DbErr> {
    Ok(Llc::find()
        .filter(entity::llc::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?
        .into_iter()
        .map(|l| (l.id, l.name))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payout_math_is_exact() {
        // $12,000 collected, $2,400 expenses, 8% fee = $960 → net $8,640.
        let c = compute_amounts(1_200_000, 240_000, 800);
        assert_eq!(c.mgmt_fee_cents, 96_000);
        assert_eq!(c.net_cents, 864_000);
    }

    #[test]
    fn payout_math_handles_zero_and_negative_margins() {
        let c = compute_amounts(0, 0, 800);
        assert_eq!(c.mgmt_fee_cents, 0);
        assert_eq!(c.net_cents, 0);
        // Expenses can exceed income — the net goes negative and execution
        // refuses it upstream.
        let c = compute_amounts(100_000, 150_000, 800);
        assert_eq!(c.net_cents, 100_000 - 150_000 - 8_000);
        assert!(c.net_cents < 0);
        // A negative fee configuration never *adds* money.
        let c = compute_amounts(100_000, 0, -500);
        assert_eq!(c.mgmt_fee_cents, 0);
    }

    #[test]
    fn statement_carries_the_period_and_amounts() {
        let payout = entity::owner_payout::Model {
            id: Uuid::from_u128(1),
            tenant_id: Uuid::from_u128(2),
            entity_id: Uuid::from_u128(3),
            period_start: "2026-06-01".into(),
            period_end: "2026-06-30".into(),
            rent_collected_cents: 1_200_000,
            expenses_cents: 240_000,
            mgmt_fee_cents: 96_000,
            net_cents: 864_000,
            status: "paid".into(),
            provider: None,
            external_id: None,
            statement_document_id: None,
            ledger_txn_id: None,
            failure_reason: None,
            created_by: None,
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        };
        let text = statement_text("Maple Holdings LLC", &payout, "2026-07-05");
        assert!(text.contains("Maple Holdings LLC"));
        assert!(text.contains("2026-06-01 to 2026-06-30"));
        assert!(text.contains("$12,000"));
        assert!(text.contains("$8,640"));
    }
}
