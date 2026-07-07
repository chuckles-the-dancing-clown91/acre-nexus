//! Request/response shapes for payments + the renter portal payment surface.

use crate::dto::usd;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct PaymentDto {
    pub id: Uuid,
    pub lease_id: Uuid,
    pub kind: String,
    pub due_date: String,
    pub paid_date: Option<String>,
    pub amount_cents: i64,
    pub amount_label: String,
    pub status: String,
    pub method: Option<String>,
    pub receipt_number: Option<String>,
    pub failure_reason: Option<String>,
    pub created_at: String,
}

impl From<entity::lease_payment::Model> for PaymentDto {
    fn from(p: entity::lease_payment::Model) -> Self {
        PaymentDto {
            amount_label: usd(p.amount_cents),
            id: p.id,
            lease_id: p.lease_id,
            kind: p.kind,
            due_date: p.due_date,
            paid_date: p.paid_date,
            amount_cents: p.amount_cents,
            status: p.status,
            method: p.method,
            receipt_number: p.receipt_number,
            failure_reason: p.failure_reason,
            created_at: p.created_at.to_rfc3339(),
        }
    }
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct PaymentMethodDto {
    pub id: Uuid,
    pub lease_id: Option<Uuid>,
    pub provider: String,
    pub kind: String,
    pub brand: Option<String>,
    pub last4: String,
    pub exp_month: Option<i32>,
    pub exp_year: Option<i32>,
    pub status: String,
    pub autopay: bool,
    pub autopay_day: Option<i32>,
}

impl From<entity::payment_method::Model> for PaymentMethodDto {
    fn from(m: entity::payment_method::Model) -> Self {
        PaymentMethodDto {
            id: m.id,
            lease_id: m.lease_id,
            provider: m.provider,
            kind: m.kind,
            brand: m.brand,
            last4: m.last4,
            exp_month: m.exp_month,
            exp_year: m.exp_year,
            status: m.status,
            autopay: m.autopay,
            autopay_day: m.autopay_day,
        }
    }
}

/// Save a payment method. Live deployments pass a provider token minted
/// client-side (Stripe.js); without one the simulated tokenizer mints a
/// `sim_pm_…` token from the display metadata — never PANs either way.
#[derive(Deserialize, schemars::JsonSchema)]
pub struct AddMethodReq {
    /// `card` | `ach`.
    pub kind: String,
    /// Provider token from client-side tokenization (`pm_…`), if any.
    pub external_id: Option<String>,
    /// Display metadata (last 4 digits only).
    pub last4: Option<String>,
    pub brand: Option<String>,
    pub exp_month: Option<i32>,
    pub exp_year: Option<i32>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct PayReq {
    /// The due item to pay in full…
    pub payment_id: Option<Uuid>,
    /// …or `deposit` to raise + pay the security deposit.
    pub kind: Option<String>,
    pub method_id: Uuid,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct AutopayReq {
    pub method_id: Uuid,
    /// Day of month to charge (1–28); defaults to the rent due day.
    pub day: Option<i32>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct MyLeaseResp {
    pub lease_id: Uuid,
    pub property_name: String,
    pub property_address: String,
    pub unit_label: Option<String>,
    pub tenant_name: String,
    /// Lease term, ISO dates.
    pub start_date: String,
    pub end_date: Option<String>,
    pub status: String,
    pub payment_status: String,
    pub rent_cents: i64,
    pub rent_label: String,
    pub balance_cents: i64,
    pub balance_label: String,
    pub deposit_cents: Option<i64>,
    pub deposit_label: Option<String>,
    /// The deposit has a settled (or in-flight) payment.
    pub deposit_paid: bool,
    pub autopay_enabled: bool,
    /// Items currently payable (due / late / failed).
    pub due_items: Vec<PaymentDto>,
    /// Settled + in-flight history, newest first.
    pub history: Vec<PaymentDto>,
    pub methods: Vec<PaymentMethodDto>,
}
