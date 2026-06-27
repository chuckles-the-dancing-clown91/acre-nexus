//! Request/response shapes for the property financing (mortgage) endpoints.

use crate::dto::usd;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Label an optional cents amount as USD.
fn label(cents: Option<i64>) -> Option<String> {
    cents.map(usd)
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct MortgageDto {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub property_id: Uuid,
    pub lender_id: Option<Uuid>,
    pub kind: String,
    pub position: i32,
    pub original_amount_cents: Option<i64>,
    pub original_amount_label: Option<String>,
    pub current_balance_cents: Option<i64>,
    pub current_balance_label: Option<String>,
    pub interest_rate_bps: Option<i32>,
    pub interest_rate_pct: Option<f64>,
    pub term_months: Option<i32>,
    pub monthly_payment_cents: Option<i64>,
    pub monthly_payment_label: Option<String>,
    pub escrow_monthly_cents: Option<i64>,
    pub escrow_monthly_label: Option<String>,
    pub start_date: Option<String>,
    pub maturity_date: Option<String>,
    pub loan_number: Option<String>,
    pub status: String,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<entity::mortgage::Model> for MortgageDto {
    fn from(m: entity::mortgage::Model) -> Self {
        MortgageDto {
            original_amount_label: label(m.original_amount_cents),
            current_balance_label: label(m.current_balance_cents),
            monthly_payment_label: label(m.monthly_payment_cents),
            escrow_monthly_label: label(m.escrow_monthly_cents),
            interest_rate_pct: m.interest_rate_bps.map(|b| b as f64 / 100.0),
            id: m.id,
            tenant_id: m.tenant_id,
            property_id: m.property_id,
            lender_id: m.lender_id,
            kind: m.kind,
            position: m.position,
            original_amount_cents: m.original_amount_cents,
            current_balance_cents: m.current_balance_cents,
            interest_rate_bps: m.interest_rate_bps,
            term_months: m.term_months,
            monthly_payment_cents: m.monthly_payment_cents,
            escrow_monthly_cents: m.escrow_monthly_cents,
            start_date: m.start_date,
            maturity_date: m.maturity_date,
            loan_number: m.loan_number,
            status: m.status,
            notes: m.notes,
            created_at: m.created_at.to_rfc3339(),
            updated_at: m.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateMortgageReq {
    pub lender_id: Option<Uuid>,
    pub kind: String,
    pub position: Option<i32>,
    pub original_amount_cents: Option<i64>,
    pub current_balance_cents: Option<i64>,
    pub interest_rate_bps: Option<i32>,
    pub term_months: Option<i32>,
    pub monthly_payment_cents: Option<i64>,
    pub escrow_monthly_cents: Option<i64>,
    pub start_date: Option<String>,
    pub maturity_date: Option<String>,
    pub loan_number: Option<String>,
    pub status: Option<String>,
    pub notes: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateMortgageReq {
    pub lender_id: Option<Uuid>,
    pub kind: Option<String>,
    pub position: Option<i32>,
    pub original_amount_cents: Option<i64>,
    pub current_balance_cents: Option<i64>,
    pub interest_rate_bps: Option<i32>,
    pub term_months: Option<i32>,
    pub monthly_payment_cents: Option<i64>,
    pub escrow_monthly_cents: Option<i64>,
    pub start_date: Option<String>,
    pub maturity_date: Option<String>,
    pub loan_number: Option<String>,
    pub status: Option<String>,
    pub notes: Option<String>,
}
