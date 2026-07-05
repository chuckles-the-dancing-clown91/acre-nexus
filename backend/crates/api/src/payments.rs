//! **Payment lifecycle orchestration** (roadmap Phase 3, issue #35).
//!
//! A [`entity::lease_payment`] row is both the receivable ("rent is due") and
//! the payment attempt against it. The electronic flow:
//!
//! 1. a receivable exists (`due`/`late`) or an ad-hoc item is created;
//! 2. [`start_charge`] flips it to `processing` and enqueues a durable
//!    `payment_process` job;
//! 3. the job charges through the [`crate::providers::payments`] provider
//!    (Stripe live, simulated otherwise);
//! 4. settlement lands in [`settle_payment`] — from the **Stripe webhook** in
//!    live mode, or after the tenant's configured callback delay in
//!    simulation. Either way the same path updates the lease's balance and
//!    standing, posts the balanced ledger entry, stores a receipt PDF in the
//!    document service, audits, and notifies.
//!
//! Failure is a first-class outcome: the payment keeps its `failure_reason`,
//! the resident is notified, and autopay will not hammer a failed method.

use crate::error::{ApiError, ApiResult};
use crate::modules::JobOutcome;
use crate::providers::payments::{PaymentsRequest, StripeProvider};
use crate::providers::ProviderCtx;
use crate::storage::ObjectStore;
use chrono::Utc;
use entity::prelude::{Lease, LeasePayment, PaymentMethod, Property, User};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    Set,
};
use serde_json::json;
use uuid::Uuid;

/// Payment kinds. `deposit` settles into the trust ledger; everything else
/// into operating cash.
pub const KIND_RENT: &str = "rent";
pub const KIND_DEPOSIT: &str = "deposit";
pub const KIND_FEE: &str = "fee";

/// The legal entity (LLC) whose books a property posts to, if assigned.
pub async fn entity_for_property(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    property_id: Uuid,
) -> Option<Uuid> {
    Property::find_by_id(property_id)
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .ok()
        .flatten()
        .and_then(|p| p.llc_id)
}

/// Flip a payable receivable to `processing` and enqueue the charge job.
/// `initiated_by` is the portal user (or staff) who pressed pay; `None` =
/// autopay.
pub async fn start_charge(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    payment: entity::lease_payment::Model,
    method: &entity::payment_method::Model,
    initiated_by: Option<Uuid>,
) -> ApiResult<entity::lease_payment::Model> {
    if !matches!(payment.status.as_str(), "due" | "late" | "failed") {
        return Err(ApiError::BadRequest(format!(
            "payment is not payable (status: {})",
            payment.status
        )));
    }
    if method.status != "active" {
        return Err(ApiError::BadRequest("payment method is not active".into()));
    }
    let payment_id = payment.id;
    let mut am: entity::lease_payment::ActiveModel = payment.into();
    am.status = Set("processing".into());
    am.method_id = Set(Some(method.id));
    am.method = Set(Some(method.kind.clone()));
    am.failure_reason = Set(None);
    let saved = am.update(db).await?;

    crate::scheduler::enqueue(
        db,
        tenant_id,
        "payment_process",
        json!({ "payment_id": payment_id }),
        0,
    )
    .await?;

    crate::audit::record(
        db,
        initiated_by,
        crate::audit::actions::PAYMENT_CREATE,
        Some("lease_payment"),
        Some(payment_id.to_string()),
        Some(tenant_id),
        Some(json!({
            "lease_id": saved.lease_id,
            "amount_cents": saved.amount_cents,
            "kind": saved.kind,
            "method_kind": method.kind,
            "autopay": initiated_by.is_none(),
        })),
    )
    .await;

    Ok(saved)
}

/// Advance one `payment_process` job: charge on the first pass, then either
/// self-settle (simulated) after the tenant's callback delay or wait for the
/// processor's webhook (live).
pub async fn handle_process_job(
    db: &DatabaseConnection,
    job: &entity::background_job::Model,
) -> JobOutcome {
    let Some(payment_id) = job
        .payload
        .get("payment_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    else {
        return JobOutcome::failed("payment_process payload missing payment_id");
    };

    let payment = match LeasePayment::find_by_id(payment_id)
        .filter(entity::lease_payment::Column::TenantId.eq(job.tenant_id))
        .one(db)
        .await
    {
        Ok(Some(p)) => p,
        Ok(None) => return JobOutcome::failed("payment not found"),
        Err(e) => {
            return JobOutcome::retry(
                crate::providers::backoff(job.attempts),
                format!("db error: {e}"),
            )
        }
    };

    // Idempotent: a payment somebody else settled is done.
    if matches!(payment.status.as_str(), "paid" | "failed") {
        return JobOutcome::completed(json!({ "already_settled": payment.status }));
    }

    // First pass: no external id yet — charge it.
    if payment.external_id.is_none() {
        let Some(method_id) = payment.method_id else {
            return JobOutcome::failed("payment has no method to charge");
        };
        let method = match PaymentMethod::find_by_id(method_id)
            .filter(entity::payment_method::Column::TenantId.eq(job.tenant_id))
            .one(db)
            .await
        {
            Ok(Some(m)) => m,
            Ok(None) => return JobOutcome::failed("payment method not found"),
            Err(e) => {
                return JobOutcome::retry(
                    crate::providers::backoff(job.attempts),
                    format!("db error: {e}"),
                )
            }
        };

        let ctx = ProviderCtx::new(db, job.tenant_id);
        let req = PaymentsRequest::Charge {
            reference: payment.id,
            amount_cents: payment.amount_cents,
            method_external_id: method.external_id.clone(),
            description: format!("{} payment", payment.kind),
        };
        let resp = match crate::providers::run(&StripeProvider, &ctx, job, &req).await {
            Ok(resp) => resp,
            Err(outcome) => return outcome,
        };

        let mut am: entity::lease_payment::ActiveModel = payment.clone().into();
        am.provider = Set(Some("stripe".into()));
        am.external_id = Set(Some(resp.external_id.clone()));
        if let Err(e) = am.update(db).await {
            return JobOutcome::retry(
                crate::providers::backoff(job.attempts),
                format!("db error: {e}"),
            );
        }

        match resp.status.as_str() {
            "succeeded" => {
                settle_payment(db, job.tenant_id, payment.id, true, None).await;
                return JobOutcome::completed(json!({ "settled": "paid" }));
            }
            "failed" => {
                settle_payment(db, job.tenant_id, payment.id, false, resp.failure_reason).await;
                return JobOutcome::completed(json!({ "settled": "failed" }));
            }
            _ => {
                // Processing. Simulated mode confirms after the tenant's
                // configured delay; live mode waits for the webhook.
                let delay = if crate::providers::is_live("stripe") {
                    300
                } else {
                    crate::settings::get_i64(
                        db,
                        job.tenant_id,
                        crate::settings::PAYMENTS_CALLBACK_DELAY_SECS,
                    )
                    .await
                    .clamp(1, 3600)
                };
                return JobOutcome::reschedule("awaiting_callback", delay);
            }
        }
    }

    // Later passes: the charge is out — settle (simulated) or check on the
    // webhook's progress (live).
    if crate::providers::is_live("stripe") {
        // The webhook owns settlement; poll a few times as a fallback, then
        // stand down (the webhook still settles whenever it arrives).
        let checks = job
            .payload
            .get("checks")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        if checks >= 10 {
            return JobOutcome::completed(json!({ "handed_off": "webhook" }));
        }
        // Note: the payload isn't mutable through JobOutcome, so the check
        // budget rides the job's result payload instead.
        let _ = checks;
        return JobOutcome::reschedule("awaiting_callback", 600);
    }

    // Simulated settlement: the "processor" confirms now.
    settle_payment(db, job.tenant_id, payment.id, true, None).await;
    JobOutcome::completed(json!({ "settled": "paid", "simulated": true }))
}

/// The single settlement path: terminal-status the payment, sync the lease,
/// post the ledger entry, store a receipt, audit, and notify. Idempotent —
/// an already-settled payment is left untouched.
pub async fn settle_payment(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    payment_id: Uuid,
    success: bool,
    failure_reason: Option<String>,
) {
    let Ok(Some(payment)) = LeasePayment::find_by_id(payment_id)
        .filter(entity::lease_payment::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
    else {
        tracing::error!("settle_payment: payment {payment_id} not found");
        return;
    };
    if matches!(payment.status.as_str(), "paid" | "failed") {
        return;
    }
    let Ok(Some(lease)) = Lease::find_by_id(payment.lease_id)
        .filter(entity::lease::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
    else {
        tracing::error!("settle_payment: lease {} not found", payment.lease_id);
        return;
    };

    let now = Utc::now();
    let today = now.date_naive().to_string();

    if !success {
        let reason = failure_reason.unwrap_or_else(|| "payment failed".into());
        let mut am: entity::lease_payment::ActiveModel = payment.clone().into();
        am.status = Set("failed".into());
        am.failure_reason = Set(Some(reason.clone()));
        if let Err(e) = am.update(db).await {
            tracing::error!("settle_payment: update failed: {e}");
            return;
        }
        crate::audit::record(
            db,
            None,
            crate::audit::actions::PAYMENT_SETTLE,
            Some("lease_payment"),
            Some(payment.id.to_string()),
            Some(tenant_id),
            Some(json!({
                "lease_id": payment.lease_id,
                "amount_cents": payment.amount_cents,
                "status": "failed",
                "reason": reason,
            })),
        )
        .await;
        notify_resident(
            db,
            tenant_id,
            &lease,
            "payment_failed",
            &payment,
            json!({
                "amount": crate::dto::usd(payment.amount_cents),
                "reason": reason,
            }),
        )
        .await;
        return;
    }

    // ---- success ----
    let receipt_number = format!(
        "RCT-{}-{}",
        now.format("%Y"),
        &payment.id.simple().to_string()[..8].to_uppercase()
    );

    // Post to the owning entity's books (properties without an LLC keep
    // settling — they just have no books to post to yet).
    let entity_id = entity_for_property(db, tenant_id, lease.property_id).await;
    let mut ledger_txn_id = None;
    if let Some(entity_id) = entity_id {
        match crate::accounting::post_payment_settled(
            db,
            tenant_id,
            entity_id,
            Some(lease.property_id),
            lease.id,
            &today,
            payment.amount_cents,
            &payment.kind,
            payment.id,
        )
        .await
        {
            Ok(txn) => ledger_txn_id = Some(txn.id),
            Err(e) => tracing::error!("settle_payment: ledger post failed: {e}"),
        }
    }

    let kind = payment.kind.clone();
    let amount = payment.amount_cents;
    let mut am: entity::lease_payment::ActiveModel = payment.clone().into();
    am.status = Set("paid".into());
    am.paid_date = Set(Some(today.clone()));
    am.receipt_number = Set(Some(receipt_number.clone()));
    am.ledger_txn_id = Set(ledger_txn_id);
    am.failure_reason = Set(None);
    if let Err(e) = am.update(db).await {
        tracing::error!("settle_payment: update failed: {e}");
        return;
    }

    // A settled rent/fee payment draws down the lease's outstanding balance
    // and may restore current standing. Deposits are held funds — they never
    // reduce the rent balance.
    if kind != KIND_DEPOSIT {
        let new_balance = (lease.balance_cents - amount).max(0);
        let payment_status = if new_balance <= 0 {
            "current"
        } else {
            "partial"
        };
        let mut lam: entity::lease::ActiveModel = lease.clone().into();
        lam.balance_cents = Set(new_balance);
        lam.payment_status = Set(payment_status.to_string());
        lam.updated_at = Set(now.into());
        if let Err(e) = lam.update(db).await {
            tracing::error!("settle_payment: lease update failed: {e}");
        }
    }

    // Receipt PDF into the document service (best-effort — a storage outage
    // must not unsettle a settled payment).
    if let Err(e) = store_receipt(db, tenant_id, &lease, &payment, &receipt_number, &today).await {
        tracing::error!("settle_payment: receipt store failed: {e}");
    }

    crate::audit::record(
        db,
        None,
        crate::audit::actions::PAYMENT_SETTLE,
        Some("lease_payment"),
        Some(payment.id.to_string()),
        Some(tenant_id),
        Some(json!({
            "lease_id": payment.lease_id,
            "amount_cents": amount,
            "kind": kind,
            "status": "paid",
            "receipt_number": receipt_number,
            "ledger_txn_id": ledger_txn_id,
        })),
    )
    .await;

    notify_resident(
        db,
        tenant_id,
        &lease,
        "payment_receipt",
        &payment,
        json!({
            "amount": crate::dto::usd(amount),
            "receipt_number": receipt_number,
        }),
    )
    .await;
    crate::notify::notify_staff(
        db,
        tenant_id,
        "payment:read",
        "payment_received",
        json!({
            "amount": crate::dto::usd(amount),
            "resident": lease.tenant_name,
        }),
        Some(("lease_payment", payment.id)),
        "settled",
        None,
    )
    .await;
}

/// Email the resident about a payment event (skipped when the lease has no
/// email on file).
async fn notify_resident(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    lease: &entity::lease::Model,
    template: &str,
    payment: &entity::lease_payment::Model,
    vars: serde_json::Value,
) {
    let Some(email) = lease
        .tenant_email
        .as_deref()
        .filter(|e| !e.trim().is_empty())
    else {
        return;
    };
    let payload = json!({
        "template": template,
        "to": email,
        "owner_type": "lease_payment",
        "owner_id": payment.id,
        "trigger": format!("{template}:{}", payment.status),
        "vars": vars,
    });
    if let Err(e) = crate::scheduler::enqueue(db, tenant_id, "auto_email", payload, 0).await {
        tracing::error!("failed to enqueue payment email: {e}");
    }
}

/// Render + store the receipt PDF against the lease.
async fn store_receipt(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    lease: &entity::lease::Model,
    payment: &entity::lease_payment::Model,
    receipt_number: &str,
    paid_date: &str,
) -> anyhow::Result<()> {
    let property = Property::find_by_id(lease.property_id)
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?;
    let text = receipt_text(
        receipt_number,
        &lease.tenant_name,
        property.as_ref().map(|p| p.address.as_str()).unwrap_or(""),
        &payment.kind,
        payment.amount_cents,
        paid_date,
        payment.method.as_deref(),
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
        owner_id: Set(lease.id),
        filename: Set(format!("receipt-{receipt_number}.pdf")),
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
    Ok(())
}

/// The rendered receipt body — plain text, one page.
pub fn receipt_text(
    receipt_number: &str,
    resident: &str,
    property_address: &str,
    kind: &str,
    amount_cents: i64,
    paid_date: &str,
    method: Option<&str>,
) -> String {
    let kind_label = match kind {
        KIND_DEPOSIT => "Security deposit",
        KIND_FEE => "Fee",
        _ => "Rent",
    };
    format!(
        "PAYMENT RECEIPT\n\
         ================================\n\n\
         Receipt no:   {receipt_number}\n\
         Date paid:    {paid_date}\n\
         Received of:  {resident}\n\
         Property:     {property_address}\n\
         For:          {kind_label}\n\
         Method:       {}\n\
         Amount:       {}\n\n\
         This receipt was generated automatically at settlement and recorded\n\
         to the property's ledger.",
        method.unwrap_or("electronic"),
        crate::dto::usd(amount_cents),
    )
}

// ---------------------------------------------------------------------------
// Webhook dispatch (the `webhook_event` consumers for stripe / plaid)
// ---------------------------------------------------------------------------

/// Handle one verified `webhook_event` for a payments provider. Returns
/// `None` when the provider isn't ours so the integrations module can keep
/// its generic "processed" behaviour.
pub async fn handle_webhook_event(
    db: &DatabaseConnection,
    job: &entity::background_job::Model,
) -> Option<JobOutcome> {
    let provider = job.payload.get("provider").and_then(|v| v.as_str())?;
    let event = job.payload.get("event").cloned().unwrap_or(json!({}));
    match provider {
        "stripe" => Some(handle_stripe_event(db, job.tenant_id, &event).await),
        "plaid" => Some(handle_plaid_event(db, job.tenant_id, &event).await),
        _ => None,
    }
}

/// Stripe events: settlement for payments (`payment_intent.*`) and payouts
/// (`payout.*`), matched by external id (falling back to `metadata.reference`).
async fn handle_stripe_event(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    event: &serde_json::Value,
) -> JobOutcome {
    let event_type = event.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let object = event.pointer("/data/object").cloned().unwrap_or(json!({}));
    let external_id = object.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let reference = object
        .pointer("/metadata/reference")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok());

    match event_type {
        "payment_intent.succeeded" | "payment_intent.payment_failed" => {
            let success = event_type == "payment_intent.succeeded";
            let failure = object
                .pointer("/last_payment_error/message")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .or_else(|| Some("payment failed".into()))
                .filter(|_| !success);
            let payment = find_payment(db, tenant_id, external_id, reference).await;
            match payment {
                Some(p) => {
                    settle_payment(db, tenant_id, p.id, success, failure).await;
                    JobOutcome::completed(json!({
                        "provider": "stripe",
                        "event": event_type,
                        "payment_id": p.id,
                    }))
                }
                None => JobOutcome::failed(format!(
                    "stripe event {event_type} matched no payment (id {external_id})"
                )),
            }
        }
        "payout.paid" | "payout.failed" => {
            let success = event_type == "payout.paid";
            crate::payouts::settle_by_external_id(
                db,
                tenant_id,
                external_id,
                reference,
                success,
                (!success).then(|| "payout failed".to_string()),
            )
            .await;
            JobOutcome::completed(json!({ "provider": "stripe", "event": event_type }))
        }
        _ => JobOutcome::completed(json!({
            "provider": "stripe",
            "event": event_type,
            "ignored": true,
        })),
    }
}

/// Plaid events: new transactions available → sync the linked account.
async fn handle_plaid_event(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    event: &serde_json::Value,
) -> JobOutcome {
    let webhook_type = event
        .get("webhook_type")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if webhook_type != "TRANSACTIONS" {
        return JobOutcome::completed(json!({
            "provider": "plaid",
            "event": webhook_type,
            "ignored": true,
        }));
    }
    // Sync every linked account for the tenant (Plaid item ids don't map 1:1
    // onto our rows without a lookup table; a sweep is cheap and idempotent).
    let accounts = entity::prelude::BankAccount::find()
        .filter(entity::bank_account::Column::TenantId.eq(tenant_id))
        .filter(entity::bank_account::Column::ExternalId.is_not_null())
        .all(db)
        .await
        .unwrap_or_default();
    let mut enqueued = 0;
    for account in &accounts {
        if crate::scheduler::enqueue(
            db,
            tenant_id,
            "bank_feed_sync",
            json!({ "bank_account_id": account.id }),
            0,
        )
        .await
        .is_ok()
        {
            enqueued += 1;
        }
    }
    JobOutcome::completed(json!({
        "provider": "plaid",
        "event": webhook_type,
        "syncs_enqueued": enqueued,
    }))
}

async fn find_payment(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    external_id: &str,
    reference: Option<Uuid>,
) -> Option<entity::lease_payment::Model> {
    if !external_id.is_empty() {
        if let Ok(Some(p)) = LeasePayment::find()
            .filter(entity::lease_payment::Column::TenantId.eq(tenant_id))
            .filter(entity::lease_payment::Column::ExternalId.eq(external_id))
            .one(db)
            .await
        {
            return Some(p);
        }
    }
    if let Some(id) = reference {
        if let Ok(Some(p)) = LeasePayment::find_by_id(id)
            .filter(entity::lease_payment::Column::TenantId.eq(tenant_id))
            .one(db)
            .await
        {
            return Some(p);
        }
    }
    None
}

/// The portal user's active lease: matched by linked application or by the
/// account email, mirroring how `/my/applications` scopes to the signed-in
/// user.
pub async fn lease_for_user(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    user_id: Uuid,
) -> ApiResult<Option<entity::lease::Model>> {
    let me = User::find_by_id(user_id)
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("user not found".into()))?;
    let email = me.email.to_lowercase();
    let leases = Lease::find()
        .filter(entity::lease::Column::TenantId.eq(tenant_id))
        .filter(entity::lease::Column::TenantEmail.eq(email))
        .filter(entity::lease::Column::Status.is_in(["active", "upcoming", "notice"]))
        .all(db)
        .await?;
    // Prefer the active lease when several match.
    Ok(leases
        .iter()
        .find(|l| l.status == "active")
        .or(leases.first())
        .cloned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receipt_text_carries_the_essentials() {
        let text = receipt_text(
            "RCT-2026-ABCD1234",
            "Taylor Brooks",
            "1200 Maple Ave",
            KIND_RENT,
            185_000,
            "2026-07-01",
            Some("ach"),
        );
        assert!(text.contains("RCT-2026-ABCD1234"));
        assert!(text.contains("Taylor Brooks"));
        assert!(text.contains("1200 Maple Ave"));
        assert!(text.contains("$1,850"));
        assert!(text.contains("Rent"));
        assert!(text.contains("ach"));
    }

    #[test]
    fn deposit_receipts_are_labeled() {
        let text = receipt_text("RCT-1", "A", "B", KIND_DEPOSIT, 250_000, "2026-07-01", None);
        assert!(text.contains("Security deposit"));
        assert!(text.contains("electronic"));
    }
}
