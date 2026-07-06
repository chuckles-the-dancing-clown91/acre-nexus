//! A **vendor bill** is the accounts-payable record: what a tenant owes an
//! external vendor (a [`super::counterparty`]) for work done — most often a
//! completed [`super::maintenance_ticket`]. It moves `draft → submitted →
//! approved → processing → paid` (with `failed` retryable and `void`
//! terminal): approval accrues the expense to the owning entity's ledger and
//! payment rides the payments provider, clearing the liability.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "vendor_bill")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// FK to `llc.id` — which entity's books the expense hits.
    pub entity_id: Uuid,
    /// FK to `counterparty.id` — the vendor being paid.
    pub counterparty_id: Uuid,
    /// Optional reporting dimension for the ledger legs.
    pub property_id: Option<Uuid>,
    /// The work order this bill pays for, when raised from maintenance.
    pub maintenance_ticket_id: Option<Uuid>,
    /// Human reference (`BILL-…`), unique per tenant.
    pub bill_number: String,
    pub memo: String,
    /// `[{ "description": …, "amount_cents": … }]` — the bill's line items;
    /// `amount_cents` is always their sum.
    pub line_items: Json,
    pub amount_cents: i64,
    /// `YYYY-MM-DD`.
    pub due_date: Option<String>,
    /// `draft` | `submitted` | `approved` | `processing` | `paid` | `failed` | `void`.
    pub status: String,
    pub submitted_by: Option<Uuid>,
    pub submitted_at: Option<DateTimeWithTimeZone>,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<DateTimeWithTimeZone>,
    /// Why the last reviewer sent it back to draft.
    pub rejected_reason: Option<String>,
    /// `stripe` | `simulated` — set when payment executes.
    pub provider: Option<String>,
    /// Provider transfer id (`po_…` / `sim_po_…`).
    pub external_id: Option<String>,
    /// The approval accrual posting (`Dr Property Expenses / Cr Accounts Payable`).
    pub accrual_txn_id: Option<Uuid>,
    /// The payment posting (`Dr Accounts Payable / Cr Operating Bank`).
    pub payment_txn_id: Option<Uuid>,
    pub failure_reason: Option<String>,
    pub paid_at: Option<DateTimeWithTimeZone>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
