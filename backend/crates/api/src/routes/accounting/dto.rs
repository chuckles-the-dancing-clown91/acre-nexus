//! Request/response shapes for the accounting endpoints.

use crate::dto::usd;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct LedgerAccountDto {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub code: String,
    pub name: String,
    pub kind: String,
    pub subtype: Option<String>,
    pub is_trust: bool,
    pub system: bool,
    pub active: bool,
    pub debit_cents: i64,
    pub credit_cents: i64,
    /// Balance in the account's normal direction.
    pub balance_cents: i64,
    pub balance_label: String,
}

impl LedgerAccountDto {
    pub fn from_activity(a: crate::accounting::AccountActivity) -> Self {
        let balance = a.balance_cents();
        LedgerAccountDto {
            id: a.account.id,
            entity_id: a.account.entity_id,
            code: a.account.code,
            name: a.account.name,
            kind: a.account.kind,
            subtype: a.account.subtype,
            is_trust: a.account.is_trust,
            system: a.account.system,
            active: a.account.active,
            debit_cents: a.debit_cents,
            credit_cents: a.credit_cents,
            balance_cents: balance,
            balance_label: usd(balance),
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateAccountReq {
    pub entity_id: Uuid,
    pub code: String,
    pub name: String,
    /// `asset` | `liability` | `equity` | `income` | `expense`.
    pub kind: String,
    pub is_trust: Option<bool>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct LedgerEntryDto {
    pub id: Uuid,
    pub account_id: Uuid,
    pub account_code: String,
    pub account_name: String,
    pub side: String,
    pub amount_cents: i64,
    pub amount_label: String,
    pub property_id: Option<Uuid>,
    pub lease_id: Option<Uuid>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct LedgerTxnDto {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub txn_date: String,
    pub memo: String,
    pub source_type: String,
    pub source_id: Option<Uuid>,
    pub posted_by: Option<Uuid>,
    pub created_at: String,
    pub entries: Vec<LedgerEntryDto>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ManualEntryLeg {
    pub account_id: Uuid,
    /// `debit` | `credit`.
    pub side: String,
    pub amount_cents: i64,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ManualTxnReq {
    pub entity_id: Uuid,
    /// `YYYY-MM-DD`; defaults to today.
    pub txn_date: Option<String>,
    pub memo: String,
    pub legs: Vec<ManualEntryLeg>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct TrialBalanceRow {
    pub code: String,
    pub name: String,
    pub kind: String,
    pub debit_cents: i64,
    pub credit_cents: i64,
    pub debit_label: String,
    pub credit_label: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct TrialBalanceResp {
    pub entity_id: Uuid,
    pub rows: Vec<TrialBalanceRow>,
    pub total_debits_cents: i64,
    pub total_credits_cents: i64,
    /// Debits equal credits — the books balance.
    pub balanced: bool,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct StatementLine {
    pub name: String,
    pub amount_cents: i64,
    pub amount_label: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct IncomeStatementResp {
    pub entity_id: Uuid,
    pub from: Option<String>,
    pub to: Option<String>,
    pub income: Vec<StatementLine>,
    pub expenses: Vec<StatementLine>,
    pub total_income_cents: i64,
    pub total_expenses_cents: i64,
    pub net_cents: i64,
    pub net_label: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct TrustReconciliationResp {
    pub entity_id: Uuid,
    pub trust_bank_cents: i64,
    pub trust_liability_cents: i64,
    pub difference_cents: i64,
    pub trust_bank_label: String,
    pub trust_liability_label: String,
    /// `difference_cents == 0` — escrow cash exactly covers what is owed back.
    pub reconciled: bool,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct FinanceSeriesResp {
    /// `YYYY-MM`, oldest first.
    pub months: Vec<String>,
    pub rent_due_cents: Vec<i64>,
    pub rent_collected_cents: Vec<i64>,
    pub noi_cents: Vec<i64>,
    pub occupancy_bps: Vec<i32>,
    pub delinquency_bps: Vec<i32>,
    pub portfolio_value_cents: Vec<i64>,
    pub active_leases: Vec<i32>,
}
