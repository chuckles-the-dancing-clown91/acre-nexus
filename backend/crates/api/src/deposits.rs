//! **Security-deposit disposition** (roadmap Phase 5, issue #9) — the loop
//! from "tenant moved out" to "deposit settled".
//!
//! A disposition is drafted against a lease whose deposit settled into the
//! trust ledger: itemized deductions (damages, cleaning, unpaid rent) are
//! withheld — released from escrow into operating cash as recognized income —
//! and the remainder refunds to the resident through the payments provider's
//! payout rail (sandbox by default), exactly like owner draws and vendor
//! bills. Settlement — webhook-driven live, immediate in simulation — posts
//! `Dr Security Deposits Held / Cr Trust Bank`, files a generated disposition
//! statement PDF on the lease, and emails the resident.

use crate::error::{ApiError, ApiResult};
use crate::modules::JobOutcome;
use crate::providers::payments::{PaymentsRequest, StripeProvider};
use crate::providers::ProviderCtx;
use crate::storage::ObjectStore;
use chrono::Utc;
use entity::prelude::{DepositDeduction, DepositDisposition, Lease, LeasePayment, Property};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, Set,
};
use serde_json::json;
use uuid::Uuid;

/// Job kind for the refund transfer (owned by the accounting module).
pub const REFUND_JOB_KIND: &str = "deposit_refund";

/// Pure disposition arithmetic: every deduction strictly positive, the total
/// never exceeding the deposit. Returns `(withheld, refund)`.
pub fn compute_refund(deposit_cents: i64, deduction_cents: &[i64]) -> ApiResult<(i64, i64)> {
    if deposit_cents <= 0 {
        return Err(ApiError::BadRequest("this lease holds no deposit".into()));
    }
    let mut withheld: i64 = 0;
    for &d in deduction_cents {
        if d <= 0 {
            return Err(ApiError::BadRequest(
                "deduction amounts must be positive".into(),
            ));
        }
        withheld += d;
    }
    if withheld > deposit_cents {
        return Err(ApiError::BadRequest(format!(
            "deductions ({}) exceed the deposit held ({})",
            crate::dto::usd(withheld),
            crate::dto::usd(deposit_cents),
        )));
    }
    Ok((withheld, deposit_cents - withheld))
}

/// Whether the lease's deposit has actually settled into trust.
pub async fn deposit_settled(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    lease_id: Uuid,
) -> ApiResult<bool> {
    Ok(LeasePayment::find()
        .filter(entity::lease_payment::Column::TenantId.eq(tenant_id))
        .filter(entity::lease_payment::Column::LeaseId.eq(lease_id))
        .filter(entity::lease_payment::Column::Kind.eq(crate::payments::KIND_DEPOSIT))
        .filter(entity::lease_payment::Column::Status.eq("paid"))
        .one(db)
        .await?
        .is_some())
}

/// The deductions on a disposition, in display order.
pub async fn deductions(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    disposition_id: Uuid,
) -> ApiResult<Vec<entity::deposit_deduction::Model>> {
    Ok(DepositDeduction::find()
        .filter(entity::deposit_deduction::Column::TenantId.eq(tenant_id))
        .filter(entity::deposit_deduction::Column::DispositionId.eq(disposition_id))
        .order_by_asc(entity::deposit_deduction::Column::SortOrder)
        .all(db)
        .await?)
}

/// Finalize a draft (or retry a failed) disposition: verify the deposit is in
/// trust, fix the refund figure, post the withheld deductions to the ledger
/// (first finalize only), and either enqueue the refund transfer or — when
/// nothing refunds — settle immediately.
pub async fn finalize(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    disposition: entity::deposit_disposition::Model,
    finalized_by: Uuid,
) -> ApiResult<entity::deposit_disposition::Model> {
    if !matches!(disposition.status.as_str(), "draft" | "failed") {
        return Err(ApiError::BadRequest(format!(
            "disposition is not finalizable (status: {})",
            disposition.status
        )));
    }
    if !deposit_settled(db, tenant_id, disposition.lease_id).await? {
        return Err(ApiError::BadRequest(
            "the deposit has not settled into trust for this lease".into(),
        ));
    }
    let lines = deductions(db, tenant_id, disposition.id).await?;
    let amounts: Vec<i64> = lines.iter().map(|d| d.amount_cents).collect();
    let (withheld, refund) = compute_refund(disposition.deposit_cents, &amounts)?;

    let entity_id = crate::payments::entity_for_property(db, tenant_id, disposition.property_id)
        .await
        .ok_or_else(|| {
            ApiError::BadRequest(
                "the property has no owning legal entity — assign an LLC before settling \
                 its deposit"
                    .into(),
            )
        })?;

    // Withheld deductions post exactly once, on the first finalize; a retry
    // after a failed refund must not double-recognize the income.
    let first_finalize = disposition.finalized_at.is_none();
    let now = Utc::now();
    let today = now.date_naive().to_string();
    if first_finalize && withheld > 0 {
        crate::accounting::post_deposit_withheld(
            db,
            tenant_id,
            entity_id,
            Some(disposition.property_id),
            disposition.lease_id,
            &today,
            withheld,
            disposition.id,
            Some(finalized_by),
        )
        .await?;
    }

    let id = disposition.id;
    let mut am: entity::deposit_disposition::ActiveModel = disposition.into();
    am.refund_cents = Set(Some(refund));
    am.status = Set("processing".into());
    am.failure_reason = Set(None);
    am.finalized_by = Set(Some(finalized_by));
    am.finalized_at = Set(Some(now.into()));
    am.updated_at = Set(now.into());
    let saved = am.update(db).await?;

    crate::audit::record(
        db,
        Some(finalized_by),
        crate::audit::actions::DEPOSIT_DISPOSITION_FINALIZE,
        Some("deposit_disposition"),
        Some(id.to_string()),
        Some(tenant_id),
        Some(json!({
            "lease_id": saved.lease_id,
            "deposit_cents": saved.deposit_cents,
            "withheld_cents": withheld,
            "refund_cents": refund,
        })),
    )
    .await;

    if refund > 0 {
        crate::scheduler::enqueue(
            db,
            tenant_id,
            REFUND_JOB_KIND,
            json!({ "disposition_id": id }),
            0,
        )
        .await?;
        Ok(saved)
    } else {
        // Nothing to transfer — the disposition settles right here.
        settle_refund(db, tenant_id, id, true, None).await;
        Ok(DepositDisposition::find_by_id(id)
            .filter(entity::deposit_disposition::Column::TenantId.eq(tenant_id))
            .one(db)
            .await?
            .unwrap_or(saved))
    }
}

/// Advance one `deposit_refund` job: transfer on the first pass, then settle
/// (simulated) or wait for the payout webhook (live).
pub async fn handle_refund_job(
    db: &DatabaseConnection,
    job: &entity::background_job::Model,
) -> JobOutcome {
    let Some(disposition_id) = job
        .payload
        .get("disposition_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    else {
        return JobOutcome::failed("deposit_refund payload missing disposition_id");
    };
    let disposition = match DepositDisposition::find_by_id(disposition_id)
        .filter(entity::deposit_disposition::Column::TenantId.eq(job.tenant_id))
        .one(db)
        .await
    {
        Ok(Some(d)) => d,
        Ok(None) => return JobOutcome::failed("disposition not found"),
        Err(e) => {
            return JobOutcome::retry(
                crate::providers::backoff(job.attempts),
                format!("db error: {e}"),
            )
        }
    };
    if matches!(disposition.status.as_str(), "closed" | "failed") {
        return JobOutcome::completed(json!({ "already_settled": disposition.status }));
    }
    let refund = disposition.refund_cents.unwrap_or(0);
    if refund <= 0 {
        settle_refund(db, job.tenant_id, disposition.id, true, None).await;
        return JobOutcome::completed(json!({ "settled": "closed", "refund": 0 }));
    }

    if disposition.external_id.is_none() {
        let ctx = ProviderCtx::new(db, job.tenant_id);
        let req = PaymentsRequest::Payout {
            reference: disposition.id,
            amount_cents: refund,
            description: format!("Security deposit refund — lease {}", disposition.lease_id),
        };
        let resp = match crate::providers::run(&StripeProvider, &ctx, job, &req).await {
            Ok(resp) => resp,
            Err(outcome) => return outcome,
        };
        let mut am: entity::deposit_disposition::ActiveModel = disposition.clone().into();
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
                settle_refund(db, job.tenant_id, disposition.id, true, None).await;
                return JobOutcome::completed(json!({ "settled": "closed" }));
            }
            "failed" => {
                settle_refund(
                    db,
                    job.tenant_id,
                    disposition.id,
                    false,
                    resp.failure_reason,
                )
                .await;
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
        return JobOutcome::completed(json!({ "handed_off": "webhook" }));
    }
    settle_refund(db, job.tenant_id, disposition.id, true, None).await;
    JobOutcome::completed(json!({ "settled": "closed", "simulated": true }))
}

/// Settle a refund found by provider id (payout-webhook path). Returns
/// whether anything matched, so the dispatcher can fall through.
pub async fn settle_by_external_id(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    external_id: &str,
    reference: Option<Uuid>,
    success: bool,
    failure_reason: Option<String>,
) -> bool {
    let mut disposition = None;
    if !external_id.is_empty() {
        disposition = DepositDisposition::find()
            .filter(entity::deposit_disposition::Column::TenantId.eq(tenant_id))
            .filter(entity::deposit_disposition::Column::ExternalId.eq(external_id))
            .one(db)
            .await
            .ok()
            .flatten();
    }
    if disposition.is_none() {
        if let Some(id) = reference {
            disposition = DepositDisposition::find_by_id(id)
                .filter(entity::deposit_disposition::Column::TenantId.eq(tenant_id))
                .one(db)
                .await
                .ok()
                .flatten();
        }
    }
    match disposition {
        Some(d) => {
            settle_refund(db, tenant_id, d.id, success, failure_reason).await;
            true
        }
        None => false,
    }
}

/// The single disposition settlement path: terminal status, refund ledger
/// posting, statement PDF on the lease, audit, resident email. Idempotent.
pub async fn settle_refund(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    disposition_id: Uuid,
    success: bool,
    failure_reason: Option<String>,
) {
    let Ok(Some(disposition)) = DepositDisposition::find_by_id(disposition_id)
        .filter(entity::deposit_disposition::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
    else {
        tracing::error!("settle_refund: disposition {disposition_id} not found");
        return;
    };
    if matches!(disposition.status.as_str(), "closed" | "failed") {
        return;
    }
    let now = Utc::now();

    if !success {
        let reason = failure_reason.unwrap_or_else(|| "deposit refund failed".into());
        let mut am: entity::deposit_disposition::ActiveModel = disposition.clone().into();
        am.status = Set("failed".into());
        am.failure_reason = Set(Some(reason.clone()));
        am.updated_at = Set(now.into());
        if let Err(e) = am.update(db).await {
            tracing::error!("settle_refund: update failed: {e}");
            return;
        }
        crate::audit::record(
            db,
            None,
            crate::audit::actions::DEPOSIT_DISPOSITION_SETTLE,
            Some("deposit_disposition"),
            Some(disposition.id.to_string()),
            Some(tenant_id),
            Some(json!({ "status": "failed", "reason": reason })),
        )
        .await;
        return;
    }

    let refund = disposition.refund_cents.unwrap_or(0);
    let today = now.date_naive().to_string();

    // Ledger: escrow cash returns to the resident, extinguishing the
    // liability (only when something actually refunds).
    if refund > 0 {
        let entity_id =
            crate::payments::entity_for_property(db, tenant_id, disposition.property_id).await;
        match entity_id {
            Some(entity_id) => {
                if let Err(e) = crate::accounting::post_deposit_refund(
                    db,
                    tenant_id,
                    entity_id,
                    Some(disposition.property_id),
                    disposition.lease_id,
                    &today,
                    refund,
                    disposition.id,
                )
                .await
                {
                    tracing::error!("settle_refund: ledger post failed: {e}");
                }
            }
            None => tracing::error!("settle_refund: property has no owning entity"),
        }
    }

    let lines = deductions(db, tenant_id, disposition.id)
        .await
        .unwrap_or_default();
    let withheld: i64 = lines.iter().map(|d| d.amount_cents).sum();

    let lease = Lease::find_by_id(disposition.lease_id)
        .filter(entity::lease::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .ok()
        .flatten();

    // Statement PDF against the lease (best-effort).
    let statement_document_id =
        match store_statement(db, tenant_id, &disposition, lease.as_ref(), &lines, &today).await {
            Ok(id) => Some(id),
            Err(e) => {
                tracing::error!("settle_refund: statement store failed: {e}");
                None
            }
        };

    let mut am: entity::deposit_disposition::ActiveModel = disposition.clone().into();
    am.status = Set("closed".into());
    am.statement_document_id = Set(statement_document_id);
    am.closed_at = Set(Some(now.into()));
    am.updated_at = Set(now.into());
    if let Err(e) = am.update(db).await {
        tracing::error!("settle_refund: update failed: {e}");
        return;
    }

    crate::audit::record(
        db,
        None,
        crate::audit::actions::DEPOSIT_DISPOSITION_SETTLE,
        Some("deposit_disposition"),
        Some(disposition.id.to_string()),
        Some(tenant_id),
        Some(json!({
            "status": "closed",
            "refund_cents": refund,
            "withheld_cents": withheld,
            "statement_document_id": statement_document_id,
        })),
    )
    .await;

    // Email the resident their statement (skipped without an email on file).
    if let Some(lease) = &lease {
        if let Some(email) = lease
            .tenant_email
            .as_deref()
            .filter(|e| !e.trim().is_empty())
        {
            let payload = json!({
                "template": "deposit_disposition_closed",
                "to": email,
                "owner_type": "deposit_disposition",
                "owner_id": disposition.id,
                "trigger": "closed",
                "vars": {
                    "deposit": crate::dto::usd(disposition.deposit_cents),
                    "withheld": crate::dto::usd(withheld),
                    "refund": crate::dto::usd(refund),
                    "deduction_count": lines.len(),
                },
            });
            if let Err(e) = crate::scheduler::enqueue(db, tenant_id, "auto_email", payload, 0).await
            {
                tracing::error!("failed to enqueue deposit statement email: {e}");
            }
        }
    }

    crate::notify::notify_staff(
        db,
        tenant_id,
        "payout:manage",
        "payout_paid",
        json!({ "amount": crate::dto::usd(refund) }),
        Some(("deposit_disposition", disposition.id)),
        "settled",
        None,
    )
    .await;
}

/// Render + store the disposition statement PDF against the lease.
async fn store_statement(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    disposition: &entity::deposit_disposition::Model,
    lease: Option<&entity::lease::Model>,
    lines: &[entity::deposit_deduction::Model],
    date: &str,
) -> anyhow::Result<Uuid> {
    let property = Property::find_by_id(disposition.property_id)
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?;
    let text = statement_text(
        lease.map(|l| l.tenant_name.as_str()).unwrap_or("Resident"),
        property.as_ref().map(|p| p.address.as_str()).unwrap_or(""),
        disposition,
        lines,
        date,
    );
    let bytes = crate::pdf::text_to_pdf(&text);

    let id = Uuid::new_v4();
    let storage_key = format!("{tenant_id}/{id}");
    let store = ObjectStore::from_env()?;
    store.put_bytes(&storage_key, &bytes).await?;

    let now = Utc::now();
    entity::document::ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant_id),
        owner_type: Set("lease".into()),
        owner_id: Set(disposition.lease_id),
        filename: Set(format!("deposit-disposition-{date}.pdf")),
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

/// The rendered disposition-statement body.
pub fn statement_text(
    tenant_name: &str,
    property_address: &str,
    disposition: &entity::deposit_disposition::Model,
    lines: &[entity::deposit_deduction::Model],
    date: &str,
) -> String {
    let usd = crate::dto::usd;
    let withheld: i64 = lines.iter().map(|d| d.amount_cents).sum();
    let refund = disposition.refund_cents.unwrap_or(0);
    let mut deduction_lines = String::new();
    if lines.is_empty() {
        deduction_lines.push_str("(none)\n");
    } else {
        for d in lines {
            deduction_lines.push_str(&format!("- {}: {}\n", d.description, usd(d.amount_cents)));
        }
    }
    format!(
        "SECURITY DEPOSIT DISPOSITION STATEMENT\n\
         ======================================\n\n\
         Resident:          {tenant_name}\n\
         Property:          {property_address}\n\
         Statement date:    {date}\n\n\
         Deposit held:      {}\n\n\
         DEDUCTIONS\n\
         --------------------------------------\n\
         {deduction_lines}\
         --------------------------------------\n\
         Total withheld:    {}\n\
         Refund to resident: {}\n\n\
         The refund was transferred to you by ACH. Withheld amounts are \n\
         itemized above; the corresponding journal entries are referenced on \n\
         the disposition record. If you have questions about this statement, \n\
         reply through your resident portal.",
        usd(disposition.deposit_cents),
        usd(withheld),
        usd(refund),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refund_math_is_exact() {
        // $2,500 deposit, $350 + $150 withheld → $2,000 refund.
        let (withheld, refund) = compute_refund(250_000, &[35_000, 15_000]).unwrap();
        assert_eq!(withheld, 50_000);
        assert_eq!(refund, 200_000);
    }

    #[test]
    fn no_deductions_refunds_everything() {
        let (withheld, refund) = compute_refund(250_000, &[]).unwrap();
        assert_eq!(withheld, 0);
        assert_eq!(refund, 250_000);
    }

    #[test]
    fn deductions_may_consume_the_whole_deposit() {
        let (withheld, refund) = compute_refund(250_000, &[250_000]).unwrap();
        assert_eq!(withheld, 250_000);
        assert_eq!(refund, 0);
    }

    #[test]
    fn overdrawn_deductions_are_rejected() {
        assert!(compute_refund(250_000, &[250_001]).is_err());
        assert!(compute_refund(250_000, &[200_000, 100_000]).is_err());
    }

    #[test]
    fn non_positive_deductions_are_rejected() {
        assert!(compute_refund(250_000, &[0]).is_err());
        assert!(compute_refund(250_000, &[-5]).is_err());
        assert!(compute_refund(0, &[]).is_err());
    }

    #[test]
    fn statement_carries_the_essentials() {
        let now = chrono::Utc::now().into();
        let disposition = entity::deposit_disposition::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            lease_id: Uuid::new_v4(),
            property_id: Uuid::new_v4(),
            status: "closed".into(),
            deposit_cents: 250_000,
            refund_cents: Some(200_000),
            notes: None,
            provider: None,
            external_id: None,
            failure_reason: None,
            statement_document_id: None,
            finalized_by: None,
            finalized_at: None,
            closed_at: None,
            created_at: now,
            updated_at: now,
        };
        let lines = vec![entity::deposit_deduction::Model {
            id: Uuid::new_v4(),
            tenant_id: disposition.tenant_id,
            disposition_id: disposition.id,
            description: "Carpet cleaning".into(),
            amount_cents: 50_000,
            sort_order: 0,
            created_at: now,
        }];
        let text = statement_text(
            "Taylor Brooks",
            "1200 Maple Ave",
            &disposition,
            &lines,
            "2026-07-07",
        );
        assert!(text.contains("Taylor Brooks"));
        assert!(text.contains("1200 Maple Ave"));
        assert!(text.contains("Carpet cleaning"));
        assert!(text.contains("$2,500"));
        assert!(text.contains("$500"));
        assert!(text.contains("$2,000"));
    }
}
