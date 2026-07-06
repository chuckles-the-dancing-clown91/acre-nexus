//! Request/response shapes for accounts payable (vendor bills).

use crate::dto::usd;
use crate::payables::LineItem;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct VendorBillDto {
    pub id: Uuid,
    pub bill_number: String,
    pub entity_id: Uuid,
    pub entity_name: Option<String>,
    pub counterparty_id: Uuid,
    pub vendor_name: Option<String>,
    pub property_id: Option<Uuid>,
    pub maintenance_ticket_id: Option<Uuid>,
    pub memo: String,
    pub line_items: Vec<LineItem>,
    pub amount_cents: i64,
    pub amount_label: String,
    pub due_date: Option<String>,
    pub status: String,
    pub submitted_at: Option<String>,
    pub approved_at: Option<String>,
    pub rejected_reason: Option<String>,
    pub accrual_txn_id: Option<Uuid>,
    pub payment_txn_id: Option<Uuid>,
    pub failure_reason: Option<String>,
    pub paid_at: Option<String>,
    pub created_at: String,
}

impl VendorBillDto {
    pub fn from_model(
        b: entity::vendor_bill::Model,
        entity_name: Option<String>,
        vendor_name: Option<String>,
    ) -> Self {
        let line_items: Vec<LineItem> =
            serde_json::from_value(b.line_items.clone()).unwrap_or_default();
        VendorBillDto {
            id: b.id,
            bill_number: b.bill_number,
            entity_id: b.entity_id,
            entity_name,
            counterparty_id: b.counterparty_id,
            vendor_name,
            property_id: b.property_id,
            maintenance_ticket_id: b.maintenance_ticket_id,
            memo: b.memo,
            line_items,
            amount_label: usd(b.amount_cents),
            amount_cents: b.amount_cents,
            due_date: b.due_date,
            status: b.status,
            submitted_at: b.submitted_at.map(|x| x.to_rfc3339()),
            approved_at: b.approved_at.map(|x| x.to_rfc3339()),
            rejected_reason: b.rejected_reason,
            accrual_txn_id: b.accrual_txn_id,
            payment_txn_id: b.payment_txn_id,
            failure_reason: b.failure_reason,
            paid_at: b.paid_at.map(|x| x.to_rfc3339()),
            created_at: b.created_at.to_rfc3339(),
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateVendorBillReq {
    /// The vendor. Optional when the bill is raised from a ticket dispatched
    /// to a contractor — the assignee prefills.
    pub counterparty_id: Option<Uuid>,
    /// Which entity's books the expense hits; resolved from the property's
    /// owning LLC when omitted.
    pub entity_id: Option<Uuid>,
    pub property_id: Option<Uuid>,
    /// Raise the bill from a completed work order: vendor, property, amount,
    /// and memo prefill from the ticket.
    pub maintenance_ticket_id: Option<Uuid>,
    pub memo: Option<String>,
    #[serde(default)]
    pub line_items: Vec<LineItem>,
    /// Used when no line items are given.
    pub amount_cents: Option<i64>,
    /// `YYYY-MM-DD`.
    pub due_date: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateVendorBillReq {
    pub memo: Option<String>,
    pub line_items: Option<Vec<LineItem>>,
    pub amount_cents: Option<i64>,
    pub due_date: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct RejectVendorBillReq {
    pub reason: Option<String>,
}
