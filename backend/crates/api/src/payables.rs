//! **Accounts payable** (issue #58) — the loop from "contractor did the work"
//! to "contractor got paid".
//!
//! A [`entity::vendor_bill`] ties a vendor ([`entity::counterparty`]) and
//! optionally a completed maintenance ticket to an amount with line items. It
//! moves through a code-defined state machine (`draft → submitted → approved
//! → processing → paid`, `failed` retryable, `void` terminal from the
//! pre-approval states):
//!
//! * **submit** hands the bill to the approvers (`payable:approve` holders
//!   are notified);
//! * **approve** accrues the expense to the owning entity's ledger
//!   (`Dr Property Expenses / Cr Accounts Payable`) — the books recognize the
//!   cost the moment the obligation is real, not when cash moves;
//! * **pay** executes through the payments provider (sandbox by default, ACH
//!   live) on the durable queue; settlement clears the liability
//!   (`Dr Accounts Payable / Cr Operating Bank`), stamps the ticket, and
//!   notifies staff + the vendor.

use crate::error::{ApiError, ApiResult};
use crate::modules::JobOutcome;
use crate::providers::payments::{PaymentsRequest, StripeProvider};
use crate::providers::ProviderCtx;
use chrono::Utc;
use entity::prelude::{Counterparty, Llc, MaintenanceTicket, Property, VendorBill};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    Set,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

/// The background-job kind that executes an approved bill's payment.
pub const PAY_JOB_KIND: &str = "vendor_bill_pay";

// ---------------------------------------------------------------------------
// State machine
// ---------------------------------------------------------------------------

/// Every status a vendor bill can hold.
pub const STATUSES: &[&str] = &[
    "draft",
    "submitted",
    "approved",
    "processing",
    "paid",
    "failed",
    "void",
];

/// The transitions the API permits from each status. `processing → paid/failed`
/// belongs to the payment pipeline, not a user action, but is listed so the
/// whole lifecycle is auditable from one table.
pub fn allowed_transitions(from: &str) -> &'static [&'static str] {
    match from {
        "draft" => &["submitted", "void"],
        "submitted" => &["approved", "draft", "void"],
        "approved" => &["processing"],
        "processing" => &["paid", "failed"],
        "failed" => &["processing"],
        _ => &[], // paid | void | unknown — terminal
    }
}

/// Whether `from → to` is a legal lifecycle step.
pub fn is_valid_transition(from: &str, to: &str) -> bool {
    allowed_transitions(from).contains(&to)
}

// ---------------------------------------------------------------------------
// Line items
// ---------------------------------------------------------------------------

/// One line of a bill.
#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct LineItem {
    pub description: String,
    pub amount_cents: i64,
}

/// Validate line items (every amount strictly positive) and return their sum.
pub fn sum_line_items(items: &[LineItem]) -> Result<i64, String> {
    let mut total: i64 = 0;
    for item in items {
        if item.amount_cents <= 0 {
            return Err(format!(
                "line item '{}' must have a positive amount",
                item.description
            ));
        }
        total += item.amount_cents;
    }
    Ok(total)
}

/// Short human bill reference, unique enough per tenant (uniqueness is
/// enforced by `uq_vendor_bill_number`).
pub fn bill_number(id: Uuid) -> String {
    format!("BILL-{}", &id.simple().to_string()[..8].to_uppercase())
}

// ---------------------------------------------------------------------------
// Creation
// ---------------------------------------------------------------------------

/// Everything `create_bill` needs, after DTO parsing.
pub struct NewBill {
    pub counterparty_id: Uuid,
    /// Books to hit. Resolved from the property when omitted.
    pub entity_id: Option<Uuid>,
    pub property_id: Option<Uuid>,
    pub maintenance_ticket_id: Option<Uuid>,
    pub memo: Option<String>,
    pub line_items: Vec<LineItem>,
    /// Used when no line items are given (a one-line bill is synthesized).
    pub amount_cents: Option<i64>,
    pub due_date: Option<String>,
}

/// Create a draft bill. When `maintenance_ticket_id` is set, missing fields
/// (vendor, property, amount, memo) prefill from the ticket, so a completed
/// work order becomes a payable bill in one call.
pub async fn create_bill(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    mut req: NewBill,
    created_by: Uuid,
) -> ApiResult<entity::vendor_bill::Model> {
    // Prefill from the originating work order.
    let mut ticket = None;
    if let Some(tid) = req.maintenance_ticket_id {
        let t = MaintenanceTicket::find_by_id(tid)
            .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
            .one(db)
            .await?
            .ok_or_else(|| ApiError::NotFound("maintenance ticket not found".into()))?;
        if req.property_id.is_none() {
            req.property_id = Some(t.property_id);
        }
        if req.amount_cents.is_none() && req.line_items.is_empty() {
            req.amount_cents = t.cost_cents.filter(|c| *c > 0);
        }
        if req.memo.as_deref().map(str::trim).unwrap_or("").is_empty() {
            req.memo = Some(t.title.clone());
        }
        ticket = Some(t);
    }
    // A ticket dispatched to a contractor supplies the vendor by default.
    let counterparty_id = if req.counterparty_id != Uuid::nil() {
        req.counterparty_id
    } else {
        ticket
            .as_ref()
            .and_then(|t| t.assignee_entity_id)
            .ok_or_else(|| ApiError::BadRequest("counterparty_id is required".into()))?
    };

    let vendor = Counterparty::find_by_id(counterparty_id)
        .filter(entity::counterparty::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("vendor (counterparty) not found".into()))?;

    // Resolve which entity's books the expense hits: explicit, else the
    // property's owning LLC.
    let entity_id = match req.entity_id {
        Some(e) => e,
        None => {
            let pid = req.property_id.ok_or_else(|| {
                ApiError::BadRequest("entity_id is required when the bill has no property".into())
            })?;
            let property = Property::find_by_id(pid)
                .filter(entity::property::Column::TenantId.eq(tenant_id))
                .one(db)
                .await?
                .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
            property.llc_id.ok_or_else(|| {
                ApiError::BadRequest(
                    "property has no owning entity — pass entity_id explicitly".into(),
                )
            })?
        }
    };
    Llc::find_by_id(entity_id)
        .filter(entity::llc::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("entity (LLC) not found".into()))?;

    // Amount: sum of line items, or a synthesized one-liner.
    let mut line_items = req.line_items;
    if line_items.is_empty() {
        let amount = req
            .amount_cents
            .ok_or_else(|| ApiError::BadRequest("amount_cents or line_items required".into()))?;
        line_items = vec![LineItem {
            description: req
                .memo
                .clone()
                .unwrap_or_else(|| "Services rendered".into()),
            amount_cents: amount,
        }];
    }
    let amount_cents = sum_line_items(&line_items).map_err(ApiError::BadRequest)?;

    let now = Utc::now();
    let id = Uuid::new_v4();
    let bill = entity::vendor_bill::ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant_id),
        entity_id: Set(entity_id),
        counterparty_id: Set(counterparty_id),
        property_id: Set(req.property_id),
        maintenance_ticket_id: Set(req.maintenance_ticket_id),
        bill_number: Set(bill_number(id)),
        memo: Set(req
            .memo
            .filter(|m| !m.trim().is_empty())
            .unwrap_or_else(|| format!("Bill from {}", vendor.name))),
        line_items: Set(serde_json::to_value(&line_items).unwrap_or_else(|_| json!([]))),
        amount_cents: Set(amount_cents),
        due_date: Set(req.due_date),
        status: Set("draft".into()),
        submitted_by: Set(None),
        submitted_at: Set(None),
        approved_by: Set(None),
        approved_at: Set(None),
        rejected_reason: Set(None),
        provider: Set(None),
        external_id: Set(None),
        accrual_txn_id: Set(None),
        payment_txn_id: Set(None),
        failure_reason: Set(None),
        paid_at: Set(None),
        created_by: Set(Some(created_by)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;

    crate::audit::record(
        db,
        Some(created_by),
        crate::audit::actions::VENDOR_BILL_CREATE,
        Some("vendor_bill"),
        Some(bill.id.to_string()),
        Some(tenant_id),
        Some(json!({
            "bill_number": bill.bill_number,
            "counterparty_id": counterparty_id,
            "entity_id": entity_id,
            "maintenance_ticket_id": req.maintenance_ticket_id,
            "amount_cents": amount_cents,
        })),
    )
    .await;

    Ok(bill)
}

// ---------------------------------------------------------------------------
// Payment execution
// ---------------------------------------------------------------------------

/// Kick an approved (or previously failed) bill into payment: flip to
/// `processing` and enqueue the durable pay job. Route-level caller has
/// already verified permission and ownership.
pub async fn pay_bill(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    bill: entity::vendor_bill::Model,
    paid_by: Uuid,
) -> ApiResult<entity::vendor_bill::Model> {
    if !is_valid_transition(&bill.status, "processing") {
        return Err(ApiError::BadRequest(format!(
            "bill is not payable (status: {})",
            bill.status
        )));
    }
    if bill.amount_cents <= 0 {
        return Err(ApiError::BadRequest(
            "bill amount must be positive to pay".into(),
        ));
    }
    let id = bill.id;
    let mut am: entity::vendor_bill::ActiveModel = bill.into();
    am.status = Set("processing".into());
    am.failure_reason = Set(None);
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(db).await?;

    crate::scheduler::enqueue(db, tenant_id, PAY_JOB_KIND, json!({ "bill_id": id }), 0).await?;

    crate::audit::record(
        db,
        Some(paid_by),
        crate::audit::actions::VENDOR_BILL_PAY,
        Some("vendor_bill"),
        Some(id.to_string()),
        Some(tenant_id),
        Some(json!({ "amount_cents": saved.amount_cents })),
    )
    .await;
    Ok(saved)
}

/// Advance one `vendor_bill_pay` job: transfer on the first pass, then settle
/// (simulated) or wait for the payout webhook (live).
pub async fn handle_pay_job(
    db: &DatabaseConnection,
    job: &entity::background_job::Model,
) -> JobOutcome {
    let Some(bill_id) = job
        .payload
        .get("bill_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    else {
        return JobOutcome::failed("vendor_bill_pay payload missing bill_id");
    };
    let bill = match VendorBill::find_by_id(bill_id)
        .filter(entity::vendor_bill::Column::TenantId.eq(job.tenant_id))
        .one(db)
        .await
    {
        Ok(Some(b)) => b,
        Ok(None) => return JobOutcome::failed("vendor bill not found"),
        Err(e) => {
            return JobOutcome::retry(
                crate::providers::backoff(job.attempts),
                format!("db error: {e}"),
            )
        }
    };
    if matches!(bill.status.as_str(), "paid" | "failed") {
        return JobOutcome::completed(json!({ "already_settled": bill.status }));
    }

    if bill.external_id.is_none() {
        let ctx = ProviderCtx::new(db, job.tenant_id);
        let req = PaymentsRequest::Payout {
            reference: bill.id,
            amount_cents: bill.amount_cents,
            description: format!("Vendor bill {} — {}", bill.bill_number, bill.memo),
        };
        let resp = match crate::providers::run(&StripeProvider, &ctx, job, &req).await {
            Ok(resp) => resp,
            Err(outcome) => return outcome,
        };
        let mut am: entity::vendor_bill::ActiveModel = bill.clone().into();
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
                settle_bill(db, job.tenant_id, bill.id, true, None).await;
                return JobOutcome::completed(json!({ "settled": "paid" }));
            }
            "failed" => {
                settle_bill(db, job.tenant_id, bill.id, false, resp.failure_reason).await;
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
    settle_bill(db, job.tenant_id, bill.id, true, None).await;
    JobOutcome::completed(json!({ "settled": "paid", "simulated": true }))
}

/// Settle a bill found by provider transfer id (webhook path). Returns whether
/// anything matched, so the caller can fall through to other payout kinds.
pub async fn settle_by_external_id(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    external_id: &str,
    reference: Option<Uuid>,
    success: bool,
    failure_reason: Option<String>,
) -> bool {
    let mut bill = None;
    if !external_id.is_empty() {
        bill = VendorBill::find()
            .filter(entity::vendor_bill::Column::TenantId.eq(tenant_id))
            .filter(entity::vendor_bill::Column::ExternalId.eq(external_id))
            .one(db)
            .await
            .ok()
            .flatten();
    }
    if bill.is_none() {
        if let Some(id) = reference {
            bill = VendorBill::find_by_id(id)
                .filter(entity::vendor_bill::Column::TenantId.eq(tenant_id))
                .one(db)
                .await
                .ok()
                .flatten();
        }
    }
    match bill {
        Some(b) => {
            settle_bill(db, tenant_id, b.id, success, failure_reason).await;
            true
        }
        None => false,
    }
}

/// The single bill settlement path: terminal status, ledger posting (clear
/// the liability), ticket update, audit, notifications. Idempotent.
pub async fn settle_bill(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    bill_id: Uuid,
    success: bool,
    failure_reason: Option<String>,
) {
    let Ok(Some(bill)) = VendorBill::find_by_id(bill_id)
        .filter(entity::vendor_bill::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
    else {
        tracing::error!("settle_bill: vendor bill {bill_id} not found");
        return;
    };
    if matches!(bill.status.as_str(), "paid" | "failed") {
        return;
    }
    let now = Utc::now();

    if !success {
        let reason = failure_reason.unwrap_or_else(|| "payment failed".into());
        let mut am: entity::vendor_bill::ActiveModel = bill.clone().into();
        am.status = Set("failed".into());
        am.failure_reason = Set(Some(reason.clone()));
        am.updated_at = Set(now.into());
        if let Err(e) = am.update(db).await {
            tracing::error!("settle_bill: update failed: {e}");
            return;
        }
        crate::audit::record(
            db,
            None,
            crate::audit::actions::VENDOR_BILL_SETTLE,
            Some("vendor_bill"),
            Some(bill.id.to_string()),
            Some(tenant_id),
            Some(json!({ "status": "failed", "reason": reason })),
        )
        .await;
        return;
    }

    // Ledger: cash leaves operating, the payable clears.
    let today = now.date_naive().to_string();
    let mut payment_txn_id = None;
    match crate::accounting::post_vendor_bill_paid(
        db,
        tenant_id,
        bill.entity_id,
        bill.property_id,
        &today,
        bill.amount_cents,
        bill.id,
    )
    .await
    {
        Ok(txn) => payment_txn_id = Some(txn.id),
        Err(e) => tracing::error!("settle_bill: ledger post failed: {e}"),
    }

    let mut am: entity::vendor_bill::ActiveModel = bill.clone().into();
    am.status = Set("paid".into());
    am.payment_txn_id = Set(payment_txn_id);
    am.paid_at = Set(Some(now.into()));
    am.updated_at = Set(now.into());
    if let Err(e) = am.update(db).await {
        tracing::error!("settle_bill: update failed: {e}");
        return;
    }

    // Close the loop on the originating work order: record the cost and note
    // the payment on the ticket's timeline.
    if let Some(ticket_id) = bill.maintenance_ticket_id {
        if let Ok(Some(ticket)) = MaintenanceTicket::find_by_id(ticket_id)
            .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
            .one(db)
            .await
        {
            let mut tam: entity::maintenance_ticket::ActiveModel = ticket.clone().into();
            if ticket.cost_cents.is_none() || ticket.cost_cents == Some(0) {
                tam.cost_cents = Set(Some(bill.amount_cents));
            }
            tam.updated_at = Set(now.into());
            let _ = tam.update(db).await;
            let comment = entity::ticket_comment::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                ticket_id: Set(ticket_id),
                author_user_id: Set(None),
                kind: Set("status".into()),
                // Money detail stays off the resident-visible timeline.
                visibility: Set("internal".into()),
                author_name: Set(None),
                body: Set(format!(
                    "Vendor bill {} paid — {}",
                    bill.bill_number,
                    crate::dto::usd(bill.amount_cents)
                )),
                created_at: Set(now.into()),
            };
            let _ = comment.insert(db).await;
        }
    }

    crate::audit::record(
        db,
        None,
        crate::audit::actions::VENDOR_BILL_SETTLE,
        Some("vendor_bill"),
        Some(bill.id.to_string()),
        Some(tenant_id),
        Some(json!({
            "status": "paid",
            "amount_cents": bill.amount_cents,
            "payment_txn_id": payment_txn_id,
            "maintenance_ticket_id": bill.maintenance_ticket_id,
        })),
    )
    .await;

    let vendor = Counterparty::find_by_id(bill.counterparty_id)
        .filter(entity::counterparty::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .ok()
        .flatten();
    let vendor_name = vendor
        .as_ref()
        .map(|v| v.name.clone())
        .unwrap_or_else(|| "the vendor".into());

    crate::notify::notify_staff(
        db,
        tenant_id,
        "payable:read",
        "vendor_bill_paid",
        json!({
            "bill_number": bill.bill_number,
            "vendor": vendor_name,
            "amount": crate::dto::usd(bill.amount_cents),
        }),
        Some(("vendor_bill", bill.id)),
        "settled",
        None,
    )
    .await;

    // Remittance advice to the vendor, when we have an address for them.
    if let Some(email) = vendor
        .as_ref()
        .and_then(|v| v.email.as_deref())
        .filter(|e| !e.trim().is_empty())
    {
        let payload = json!({
            "template": "vendor_bill_remittance",
            "to": email,
            "owner_type": "vendor_bill",
            "owner_id": bill.id,
            "trigger": "remittance",
            "vars": {
                "recipient": vendor_name,
                "bill_number": bill.bill_number,
                "amount": crate::dto::usd(bill.amount_cents),
                "memo": bill.memo,
            },
        });
        if let Err(e) = crate::scheduler::enqueue(db, tenant_id, "auto_email", payload, 0).await {
            tracing::error!("failed to enqueue remittance email: {e}");
        }
    }
}

/// Vendor names keyed by counterparty id, for list endpoints.
pub async fn vendor_names(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
) -> Result<std::collections::HashMap<Uuid, String>, sea_orm::DbErr> {
    Ok(Counterparty::find()
        .filter(entity::counterparty::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?
        .into_iter()
        .map(|c| (c.id, c.name))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_covers_the_happy_path() {
        assert!(is_valid_transition("draft", "submitted"));
        assert!(is_valid_transition("submitted", "approved"));
        assert!(is_valid_transition("approved", "processing"));
        assert!(is_valid_transition("processing", "paid"));
    }

    #[test]
    fn lifecycle_permits_rework_and_retry() {
        // A reviewer can send a submitted bill back to draft…
        assert!(is_valid_transition("submitted", "draft"));
        // …and a failed payment can be retried.
        assert!(is_valid_transition("failed", "processing"));
    }

    #[test]
    fn lifecycle_blocks_shortcuts_and_terminal_states() {
        // No skipping approval.
        assert!(!is_valid_transition("draft", "approved"));
        assert!(!is_valid_transition("draft", "processing"));
        assert!(!is_valid_transition("submitted", "processing"));
        // Approved bills are committed to the books — they pay, not vanish.
        assert!(!is_valid_transition("approved", "void"));
        assert!(!is_valid_transition("approved", "draft"));
        // Terminal.
        assert!(allowed_transitions("paid").is_empty());
        assert!(allowed_transitions("void").is_empty());
        assert!(allowed_transitions("bogus").is_empty());
    }

    #[test]
    fn line_items_sum_and_validate() {
        let items = vec![
            LineItem {
                description: "Labor".into(),
                amount_cents: 45_000,
            },
            LineItem {
                description: "Parts".into(),
                amount_cents: 12_550,
            },
        ];
        assert_eq!(sum_line_items(&items).unwrap(), 57_550);

        let bad = vec![LineItem {
            description: "Credit".into(),
            amount_cents: -500,
        }];
        assert!(sum_line_items(&bad).is_err());
        assert_eq!(sum_line_items(&[]).unwrap(), 0);
    }

    #[test]
    fn bill_numbers_are_stable_and_prefixed() {
        let id = Uuid::from_u128(0xdeadbeef_0000_0000_0000_000000000000);
        let n = bill_number(id);
        assert!(n.starts_with("BILL-"));
        assert_eq!(n, bill_number(id));
        assert_eq!(n.len(), "BILL-".len() + 8);
    }
}
