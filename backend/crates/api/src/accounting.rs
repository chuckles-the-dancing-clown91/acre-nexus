//! The **double-entry posting engine** for the multi-entity ledger
//! (roadmap Phase 3, issue #33).
//!
//! The GL is partitioned by `entity_id`: each legal entity (LLC) keeps its own
//! books with its own chart of accounts ([`DEFAULT_CHART`], seeded on first
//! touch), and **trust** (escrow) accounts carry the *no-commingling*
//! invariant — a posting may never move funds between two different entities'
//! trust ledgers (§11.2), and within one entity trust cash only ever moves
//! against trust liabilities, so escrow funds can't quietly finance
//! operations. Both rules are domain rules enforced here, in the **single
//! posting path** ([`post`]) every code path that records money goes through —
//! payments, deposits, late fees, payouts, and manual journal entries alike.
//!
//! Every transaction is **balanced by construction**: [`validate_legs`]
//! rejects an unbalanced posting before anything is written, so a trial
//! balance over any entity's books always sums to zero.

use crate::audit;
use crate::error::{ApiError, ApiResult};
use chrono::Utc;
use entity::prelude::{LedgerAccount, LedgerEntry, LedgerTxn};
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use std::collections::HashMap;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Chart of accounts
// ---------------------------------------------------------------------------

/// Stable subtype keys the posting rules resolve accounts by.
pub mod subtypes {
    pub const OPERATING_BANK: &str = "operating_bank";
    pub const TRUST_BANK: &str = "trust_bank";
    pub const ACCOUNTS_RECEIVABLE: &str = "accounts_receivable";
    pub const ACCOUNTS_PAYABLE: &str = "accounts_payable";
    pub const SECURITY_DEPOSITS: &str = "security_deposits";
    pub const OWNER_EQUITY: &str = "owner_equity";
    pub const OWNER_DRAWS: &str = "owner_draws";
    pub const RENT_INCOME: &str = "rent_income";
    pub const LATE_FEE_INCOME: &str = "late_fee_income";
    pub const FEE_INCOME: &str = "fee_income";
    pub const PROPERTY_EXPENSES: &str = "property_expenses";
    pub const MANAGEMENT_FEES: &str = "management_fees";
}

/// One line of the default (GAAP-ish) chart of accounts.
pub struct CoaDef {
    pub code: &'static str,
    pub name: &'static str,
    /// `asset` | `liability` | `equity` | `income` | `expense`.
    pub kind: &'static str,
    pub subtype: &'static str,
    pub is_trust: bool,
}

/// The default chart seeded (idempotently) for every entity that posts.
/// Tenants can add custom accounts on top; these system rows are the ones the
/// automated posting rules resolve by subtype.
pub const DEFAULT_CHART: &[CoaDef] = &[
    CoaDef {
        code: "1000",
        name: "Operating Bank",
        kind: "asset",
        subtype: subtypes::OPERATING_BANK,
        is_trust: false,
    },
    CoaDef {
        code: "1100",
        name: "Trust Bank (Escrow)",
        kind: "asset",
        subtype: subtypes::TRUST_BANK,
        is_trust: true,
    },
    CoaDef {
        code: "1200",
        name: "Accounts Receivable",
        kind: "asset",
        subtype: subtypes::ACCOUNTS_RECEIVABLE,
        is_trust: false,
    },
    CoaDef {
        code: "2000",
        name: "Accounts Payable",
        kind: "liability",
        subtype: subtypes::ACCOUNTS_PAYABLE,
        is_trust: false,
    },
    CoaDef {
        code: "2100",
        name: "Security Deposits Held",
        kind: "liability",
        subtype: subtypes::SECURITY_DEPOSITS,
        is_trust: true,
    },
    CoaDef {
        code: "3000",
        name: "Owner Equity",
        kind: "equity",
        subtype: subtypes::OWNER_EQUITY,
        is_trust: false,
    },
    CoaDef {
        code: "3100",
        name: "Owner Draws",
        kind: "equity",
        subtype: subtypes::OWNER_DRAWS,
        is_trust: false,
    },
    CoaDef {
        code: "4000",
        name: "Rental Income",
        kind: "income",
        subtype: subtypes::RENT_INCOME,
        is_trust: false,
    },
    CoaDef {
        code: "4100",
        name: "Late Fee Income",
        kind: "income",
        subtype: subtypes::LATE_FEE_INCOME,
        is_trust: false,
    },
    CoaDef {
        code: "4200",
        name: "Other Fee Income",
        kind: "income",
        subtype: subtypes::FEE_INCOME,
        is_trust: false,
    },
    CoaDef {
        code: "5000",
        name: "Property Expenses",
        kind: "expense",
        subtype: subtypes::PROPERTY_EXPENSES,
        is_trust: false,
    },
    CoaDef {
        code: "5100",
        name: "Management Fees",
        kind: "expense",
        subtype: subtypes::MANAGEMENT_FEES,
        is_trust: false,
    },
];

/// Idempotently create the default chart for `(tenant, entity)`. Existing
/// codes (system or custom) are left untouched.
pub async fn ensure_chart(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    entity_id: Uuid,
) -> ApiResult<()> {
    let existing: Vec<String> = LedgerAccount::find()
        .filter(entity::ledger_account::Column::TenantId.eq(tenant_id))
        .filter(entity::ledger_account::Column::EntityId.eq(entity_id))
        .all(db)
        .await?
        .into_iter()
        .map(|a| a.code)
        .collect();
    let now = Utc::now();
    for def in DEFAULT_CHART {
        if existing.iter().any(|c| c == def.code) {
            continue;
        }
        entity::ledger_account::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            entity_id: Set(entity_id),
            code: Set(def.code.to_string()),
            name: Set(def.name.to_string()),
            kind: Set(def.kind.to_string()),
            subtype: Set(Some(def.subtype.to_string())),
            is_trust: Set(def.is_trust),
            system: Set(true),
            active: Set(true),
            created_at: Set(now.into()),
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

/// Resolve `(tenant, entity)`'s system account by subtype, seeding the default
/// chart on first touch.
pub async fn account(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    entity_id: Uuid,
    subtype: &str,
) -> ApiResult<entity::ledger_account::Model> {
    ensure_chart(db, tenant_id, entity_id).await?;
    LedgerAccount::find()
        .filter(entity::ledger_account::Column::TenantId.eq(tenant_id))
        .filter(entity::ledger_account::Column::EntityId.eq(entity_id))
        .filter(entity::ledger_account::Column::Subtype.eq(subtype))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("no {subtype} account for entity")))
}

// ---------------------------------------------------------------------------
// Posting
// ---------------------------------------------------------------------------

/// Which way an account moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Debit,
    Credit,
}

impl Side {
    pub fn as_str(self) -> &'static str {
        match self {
            Side::Debit => "debit",
            Side::Credit => "credit",
        }
    }
}

/// One leg of a posting under construction.
#[derive(Clone, Debug)]
pub struct Leg {
    pub account_id: Uuid,
    pub side: Side,
    pub amount_cents: i64,
    pub property_id: Option<Uuid>,
    pub lease_id: Option<Uuid>,
}

impl Leg {
    pub fn debit(account_id: Uuid, amount_cents: i64) -> Self {
        Leg {
            account_id,
            side: Side::Debit,
            amount_cents,
            property_id: None,
            lease_id: None,
        }
    }

    pub fn credit(account_id: Uuid, amount_cents: i64) -> Self {
        Leg {
            account_id,
            side: Side::Credit,
            amount_cents,
            property_id: None,
            lease_id: None,
        }
    }

    pub fn on(mut self, property_id: Option<Uuid>, lease_id: Option<Uuid>) -> Self {
        self.property_id = property_id;
        self.lease_id = lease_id;
        self
    }
}

/// Header fields for one posting.
pub struct PostArgs<'a> {
    pub entity_id: Uuid,
    /// Effective date, `YYYY-MM-DD`.
    pub txn_date: &'a str,
    pub memo: &'a str,
    /// `rent_due` | `payment` | `deposit` | `late_fee` | `payout` | `manual` | …
    pub source_type: &'a str,
    pub source_id: Option<Uuid>,
    /// `None` = the pipeline posted it.
    pub posted_by: Option<Uuid>,
}

/// The account facts [`validate_legs`] checks a posting against.
#[derive(Clone, Copy, Debug)]
pub struct AccountMeta {
    pub entity_id: Uuid,
    /// `asset` | `liability` | `equity` | `income` | `expense`.
    pub kind: AccountKind,
    pub is_trust: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountKind {
    Asset,
    Liability,
    Equity,
    Income,
    Expense,
}

impl AccountKind {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "asset" => AccountKind::Asset,
            "liability" => AccountKind::Liability,
            "equity" => AccountKind::Equity,
            "income" => AccountKind::Income,
            "expense" => AccountKind::Expense,
            _ => return None,
        })
    }

    /// Whether a debit increases this account (assets + expenses) or a credit
    /// does (liabilities, equity, income).
    pub fn debit_normal(self) -> bool {
        matches!(self, AccountKind::Asset | AccountKind::Expense)
    }
}

/// Validate a set of legs against the accounts they touch. Pure — the whole
/// invariant surface is unit-testable without a database:
///
/// 1. at least two legs, every amount strictly positive;
/// 2. total debits equal total credits (double entry);
/// 3. every account belongs to the posting's entity (one transaction never
///    spans two entities' books — the cross-entity commingling guard by
///    construction);
/// 4. **trust integrity**: the signed movement of trust *asset* accounts must
///    equal the signed movement of trust *liability* accounts, so escrow cash
///    only ever moves against what is owed back — trust funds can neither
///    leak into operating cash nor absorb operating shortfalls.
pub fn validate_legs(
    legs: &[Leg],
    accounts: &HashMap<Uuid, AccountMeta>,
    entity_id: Uuid,
) -> Result<(), ApiError> {
    if legs.len() < 2 {
        return Err(ApiError::BadRequest(
            "a posting needs at least a debit and a credit".into(),
        ));
    }
    let mut debits: i64 = 0;
    let mut credits: i64 = 0;
    let mut trust_asset_delta: i64 = 0;
    let mut trust_liability_delta: i64 = 0;
    for leg in legs {
        if leg.amount_cents <= 0 {
            return Err(ApiError::BadRequest(
                "posting amounts must be positive".into(),
            ));
        }
        let meta = accounts
            .get(&leg.account_id)
            .ok_or_else(|| ApiError::BadRequest("unknown ledger account".into()))?;
        if meta.entity_id != entity_id {
            return Err(ApiError::BadRequest(
                "a posting may not span two entities' books".into(),
            ));
        }
        match leg.side {
            Side::Debit => debits += leg.amount_cents,
            Side::Credit => credits += leg.amount_cents,
        }
        if meta.is_trust {
            // Signed delta in the account's *increasing* direction.
            let signed = match (meta.kind.debit_normal(), leg.side) {
                (true, Side::Debit) | (false, Side::Credit) => leg.amount_cents,
                _ => -leg.amount_cents,
            };
            match meta.kind {
                AccountKind::Asset => trust_asset_delta += signed,
                _ => trust_liability_delta += signed,
            }
        }
    }
    if debits != credits {
        return Err(ApiError::BadRequest(format!(
            "unbalanced posting: debits {debits} != credits {credits}"
        )));
    }
    if trust_asset_delta != trust_liability_delta {
        return Err(ApiError::BadRequest(
            "trust integrity violation: escrow cash may only move against trust \
             liabilities (no commingling with operating funds)"
                .into(),
        ));
    }
    Ok(())
}

/// Post one balanced transaction to `(tenant, entity)`'s books. This is the
/// single write path into the ledger: it validates ([`validate_legs`]) and
/// then writes the header + legs, auditing the posting.
pub async fn post(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    args: PostArgs<'_>,
    legs: Vec<Leg>,
) -> ApiResult<entity::ledger_txn::Model> {
    // Load the touched accounts once and validate before writing anything.
    let ids: Vec<Uuid> = legs.iter().map(|l| l.account_id).collect();
    let rows = LedgerAccount::find()
        .filter(entity::ledger_account::Column::TenantId.eq(tenant_id))
        .filter(entity::ledger_account::Column::Id.is_in(ids))
        .all(db)
        .await?;
    let metas: HashMap<Uuid, AccountMeta> = rows
        .iter()
        .filter_map(|a| {
            Some((
                a.id,
                AccountMeta {
                    entity_id: a.entity_id,
                    kind: AccountKind::parse(&a.kind)?,
                    is_trust: a.is_trust,
                },
            ))
        })
        .collect();
    validate_legs(&legs, &metas, args.entity_id)?;

    let now = Utc::now();
    let txn = entity::ledger_txn::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        entity_id: Set(args.entity_id),
        txn_date: Set(args.txn_date.to_string()),
        memo: Set(args.memo.to_string()),
        source_type: Set(args.source_type.to_string()),
        source_id: Set(args.source_id),
        posted_by: Set(args.posted_by),
        created_at: Set(now.into()),
    }
    .insert(db)
    .await?;

    for leg in &legs {
        entity::ledger_entry::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            txn_id: Set(txn.id),
            account_id: Set(leg.account_id),
            side: Set(leg.side.as_str().to_string()),
            amount_cents: Set(leg.amount_cents),
            property_id: Set(leg.property_id),
            lease_id: Set(leg.lease_id),
            created_at: Set(now.into()),
        }
        .insert(db)
        .await?;
    }

    audit::record(
        db,
        args.posted_by,
        audit::actions::LEDGER_POST,
        Some("ledger_txn"),
        Some(txn.id.to_string()),
        Some(tenant_id),
        Some(serde_json::json!({
            "entity_id": args.entity_id,
            "source_type": args.source_type,
            "source_id": args.source_id,
            "memo": args.memo,
            "legs": legs.len(),
            "total_cents": legs
                .iter()
                .filter(|l| l.side == Side::Debit)
                .map(|l| l.amount_cents)
                .sum::<i64>(),
        })),
    )
    .await;

    Ok(txn)
}

// ---------------------------------------------------------------------------
// Posting rules — the standard translations from domain events to entries
// ---------------------------------------------------------------------------

/// Rent (or a fee) falls due: the entity is owed money it hasn't received.
/// `Dr Accounts Receivable / Cr Rental Income`.
#[allow(clippy::too_many_arguments)]
pub async fn post_rent_due(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    entity_id: Uuid,
    property_id: Option<Uuid>,
    lease_id: Uuid,
    date: &str,
    amount_cents: i64,
    source_id: Uuid,
) -> ApiResult<entity::ledger_txn::Model> {
    let ar = account(db, tenant_id, entity_id, subtypes::ACCOUNTS_RECEIVABLE).await?;
    let income = account(db, tenant_id, entity_id, subtypes::RENT_INCOME).await?;
    post(
        db,
        tenant_id,
        PostArgs {
            entity_id,
            txn_date: date,
            memo: "Rent due",
            source_type: "rent_due",
            source_id: Some(source_id),
            posted_by: None,
        },
        vec![
            Leg::debit(ar.id, amount_cents).on(property_id, Some(lease_id)),
            Leg::credit(income.id, amount_cents).on(property_id, Some(lease_id)),
        ],
    )
    .await
}

/// A receivable settles: cash lands in the right bank for its kind.
/// Rent/fees: `Dr Operating Bank / Cr Accounts Receivable`. Security
/// deposits: `Dr Trust Bank / Cr Security Deposits Held` — escrow cash rises
/// exactly as the amount owed back does, which is what keeps the trust
/// invariant intact.
#[allow(clippy::too_many_arguments)]
pub async fn post_payment_settled(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    entity_id: Uuid,
    property_id: Option<Uuid>,
    lease_id: Uuid,
    date: &str,
    amount_cents: i64,
    kind: &str,
    source_id: Uuid,
) -> ApiResult<entity::ledger_txn::Model> {
    let (debit_acc, credit_acc, memo) = if kind == "deposit" {
        (
            account(db, tenant_id, entity_id, subtypes::TRUST_BANK).await?,
            account(db, tenant_id, entity_id, subtypes::SECURITY_DEPOSITS).await?,
            "Security deposit received",
        )
    } else {
        (
            account(db, tenant_id, entity_id, subtypes::OPERATING_BANK).await?,
            account(db, tenant_id, entity_id, subtypes::ACCOUNTS_RECEIVABLE).await?,
            "Payment received",
        )
    };
    post(
        db,
        tenant_id,
        PostArgs {
            entity_id,
            txn_date: date,
            memo,
            source_type: if kind == "deposit" {
                "deposit"
            } else {
                "payment"
            },
            source_id: Some(source_id),
            posted_by: None,
        },
        vec![
            Leg::debit(debit_acc.id, amount_cents).on(property_id, Some(lease_id)),
            Leg::credit(credit_acc.id, amount_cents).on(property_id, Some(lease_id)),
        ],
    )
    .await
}

/// A late fee is assessed: `Dr Accounts Receivable / Cr Late Fee Income`.
#[allow(clippy::too_many_arguments)]
pub async fn post_late_fee(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    entity_id: Uuid,
    property_id: Option<Uuid>,
    lease_id: Uuid,
    date: &str,
    amount_cents: i64,
    source_id: Uuid,
) -> ApiResult<entity::ledger_txn::Model> {
    let ar = account(db, tenant_id, entity_id, subtypes::ACCOUNTS_RECEIVABLE).await?;
    let income = account(db, tenant_id, entity_id, subtypes::LATE_FEE_INCOME).await?;
    post(
        db,
        tenant_id,
        PostArgs {
            entity_id,
            txn_date: date,
            memo: "Late fee assessed",
            source_type: "late_fee",
            source_id: Some(source_id),
            posted_by: None,
        },
        vec![
            Leg::debit(ar.id, amount_cents).on(property_id, Some(lease_id)),
            Leg::credit(income.id, amount_cents).on(property_id, Some(lease_id)),
        ],
    )
    .await
}

/// An owner payout settles: the management fee is recognized and the net draw
/// leaves the operating account.
/// `Dr Owner Draws (net) + Dr Management Fees (fee) / Cr Operating Bank`.
pub async fn post_payout(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    entity_id: Uuid,
    date: &str,
    net_cents: i64,
    mgmt_fee_cents: i64,
    source_id: Uuid,
) -> ApiResult<entity::ledger_txn::Model> {
    let draws = account(db, tenant_id, entity_id, subtypes::OWNER_DRAWS).await?;
    let fees = account(db, tenant_id, entity_id, subtypes::MANAGEMENT_FEES).await?;
    let bank = account(db, tenant_id, entity_id, subtypes::OPERATING_BANK).await?;
    let mut legs = vec![Leg::debit(draws.id, net_cents)];
    if mgmt_fee_cents > 0 {
        legs.push(Leg::debit(fees.id, mgmt_fee_cents));
    }
    legs.push(Leg::credit(bank.id, net_cents + mgmt_fee_cents));
    post(
        db,
        tenant_id,
        PostArgs {
            entity_id,
            txn_date: date,
            memo: "Owner payout",
            source_type: "payout",
            source_id: Some(source_id),
            posted_by: None,
        },
        legs,
    )
    .await
}

/// A vendor bill is approved: the expense is recognized and the amount owed
/// to the vendor becomes a liability.
/// `Dr Property Expenses / Cr Accounts Payable`.
#[allow(clippy::too_many_arguments)]
pub async fn post_vendor_bill_approved(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    entity_id: Uuid,
    property_id: Option<Uuid>,
    date: &str,
    amount_cents: i64,
    source_id: Uuid,
    approved_by: Option<Uuid>,
) -> ApiResult<entity::ledger_txn::Model> {
    let expenses = account(db, tenant_id, entity_id, subtypes::PROPERTY_EXPENSES).await?;
    let payable = account(db, tenant_id, entity_id, subtypes::ACCOUNTS_PAYABLE).await?;
    post(
        db,
        tenant_id,
        PostArgs {
            entity_id,
            txn_date: date,
            memo: "Vendor bill approved",
            source_type: "vendor_bill",
            source_id: Some(source_id),
            posted_by: approved_by,
        },
        vec![
            Leg::debit(expenses.id, amount_cents).on(property_id, None),
            Leg::credit(payable.id, amount_cents).on(property_id, None),
        ],
    )
    .await
}

/// A vendor bill is paid: cash leaves operating and the liability clears.
/// `Dr Accounts Payable / Cr Operating Bank`.
pub async fn post_vendor_bill_paid(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    entity_id: Uuid,
    property_id: Option<Uuid>,
    date: &str,
    amount_cents: i64,
    source_id: Uuid,
) -> ApiResult<entity::ledger_txn::Model> {
    let payable = account(db, tenant_id, entity_id, subtypes::ACCOUNTS_PAYABLE).await?;
    let bank = account(db, tenant_id, entity_id, subtypes::OPERATING_BANK).await?;
    post(
        db,
        tenant_id,
        PostArgs {
            entity_id,
            txn_date: date,
            memo: "Vendor bill paid",
            source_type: "vendor_bill",
            source_id: Some(source_id),
            posted_by: None,
        },
        vec![
            Leg::debit(payable.id, amount_cents).on(property_id, None),
            Leg::credit(bank.id, amount_cents).on(property_id, None),
        ],
    )
    .await
}

// ---------------------------------------------------------------------------
// Reports
// ---------------------------------------------------------------------------

/// One account's activity totals.
#[derive(Clone, Debug)]
pub struct AccountActivity {
    pub account: entity::ledger_account::Model,
    pub debit_cents: i64,
    pub credit_cents: i64,
}

impl AccountActivity {
    /// Balance in the account's normal direction (assets/expenses debit-normal,
    /// the rest credit-normal).
    pub fn balance_cents(&self) -> i64 {
        match AccountKind::parse(&self.account.kind) {
            Some(k) if k.debit_normal() => self.debit_cents - self.credit_cents,
            _ => self.credit_cents - self.debit_cents,
        }
    }
}

/// Per-account debit/credit totals for `(tenant, entity)`, optionally limited
/// to postings dated within `[from, to]` (inclusive, `YYYY-MM-DD` — string
/// comparison is correct for ISO dates).
pub async fn account_activity(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    entity_id: Uuid,
    from: Option<&str>,
    to: Option<&str>,
) -> ApiResult<Vec<AccountActivity>> {
    let accounts = LedgerAccount::find()
        .filter(entity::ledger_account::Column::TenantId.eq(tenant_id))
        .filter(entity::ledger_account::Column::EntityId.eq(entity_id))
        .all(db)
        .await?;

    let mut txns = LedgerTxn::find()
        .filter(entity::ledger_txn::Column::TenantId.eq(tenant_id))
        .filter(entity::ledger_txn::Column::EntityId.eq(entity_id));
    if let Some(from) = from {
        txns = txns.filter(entity::ledger_txn::Column::TxnDate.gte(from));
    }
    if let Some(to) = to {
        txns = txns.filter(entity::ledger_txn::Column::TxnDate.lte(to));
    }
    let txn_ids: Vec<Uuid> = txns.all(db).await?.into_iter().map(|t| t.id).collect();

    let mut debits: HashMap<Uuid, i64> = HashMap::new();
    let mut credits: HashMap<Uuid, i64> = HashMap::new();
    if !txn_ids.is_empty() {
        let entries = LedgerEntry::find()
            .filter(entity::ledger_entry::Column::TenantId.eq(tenant_id))
            .filter(entity::ledger_entry::Column::TxnId.is_in(txn_ids))
            .all(db)
            .await?;
        for e in entries {
            let bucket = if e.side == "debit" {
                &mut debits
            } else {
                &mut credits
            };
            *bucket.entry(e.account_id).or_default() += e.amount_cents;
        }
    }

    Ok(accounts
        .into_iter()
        .map(|account| AccountActivity {
            debit_cents: debits.get(&account.id).copied().unwrap_or(0),
            credit_cents: credits.get(&account.id).copied().unwrap_or(0),
            account,
        })
        .collect())
}

/// Trust reconciliation: escrow cash on hand vs what is owed back. A healthy
/// trust ledger has `difference == 0`.
pub struct TrustReconciliation {
    pub trust_bank_cents: i64,
    pub trust_liability_cents: i64,
}

impl TrustReconciliation {
    pub fn difference_cents(&self) -> i64 {
        self.trust_bank_cents - self.trust_liability_cents
    }
}

pub async fn trust_reconciliation(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    entity_id: Uuid,
) -> ApiResult<TrustReconciliation> {
    let activity = account_activity(db, tenant_id, entity_id, None, None).await?;
    let mut bank = 0;
    let mut liability = 0;
    for a in &activity {
        if !a.account.is_trust {
            continue;
        }
        match AccountKind::parse(&a.account.kind) {
            Some(AccountKind::Asset) => bank += a.balance_cents(),
            Some(_) => liability += a.balance_cents(),
            None => {}
        }
    }
    Ok(TrustReconciliation {
        trust_bank_cents: bank,
        trust_liability_cents: liability,
    })
}

// ---------------------------------------------------------------------------
// Cross-entity transfer guard (predates the posting engine; still the single
// rule any future inter-entity transfer endpoint must call)
// ---------------------------------------------------------------------------

/// One side of a transfer between two bank accounts.
///
/// [`post`] can never commingle across entities — a transaction is bound to
/// one entity's books — so this guard exists for the flows that move money
/// *between* entities (e.g. a management-fee sweep executed as two postings).
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct PostingLeg {
    /// The legal entity (LLC) whose ledger this leg belongs to.
    pub entity_id: Uuid,
    /// Whether the account is a `trust`/escrow account (vs `operating`).
    pub is_trust: bool,
}

/// Assert a transfer does not commingle two entities' trust funds.
///
/// A transfer that touches a **trust** account on each side is only legal when
/// both sides belong to the **same** legal entity. Operating-to-operating and
/// operating-to-trust transfers across entities are permitted (e.g. a management
/// fee sweep); trust-to-trust across entities is the commingling the rule forbids.
#[allow(dead_code)]
pub fn assert_no_commingling(from: PostingLeg, to: PostingLeg) -> Result<(), ApiError> {
    if from.is_trust && to.is_trust && from.entity_id != to.entity_id {
        return Err(ApiError::BadRequest(
            "commingling violation: a trust posting may not move funds between two \
             entities' trust ledgers"
                .into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leg(entity: u128, trust: bool) -> PostingLeg {
        PostingLeg {
            entity_id: Uuid::from_u128(entity),
            is_trust: trust,
        }
    }

    #[test]
    fn same_entity_trust_transfer_ok() {
        assert!(assert_no_commingling(leg(1, true), leg(1, true)).is_ok());
    }

    #[test]
    fn cross_entity_trust_transfer_rejected() {
        assert!(assert_no_commingling(leg(1, true), leg(2, true)).is_err());
    }

    #[test]
    fn cross_entity_operating_transfer_ok() {
        assert!(assert_no_commingling(leg(1, false), leg(2, false)).is_ok());
        // Operating -> trust across entities (e.g. funding an escrow) is allowed.
        assert!(assert_no_commingling(leg(1, false), leg(2, true)).is_ok());
    }

    // ---- posting-engine validation ----

    fn entity_id() -> Uuid {
        Uuid::from_u128(42)
    }

    fn meta(id: u128, kind: AccountKind, is_trust: bool) -> (Uuid, AccountMeta) {
        (
            Uuid::from_u128(id),
            AccountMeta {
                entity_id: entity_id(),
                kind,
                is_trust,
            },
        )
    }

    /// The default chart used by the validation tests: operating bank, trust
    /// bank, AR, deposit liability, rent income.
    fn accounts() -> HashMap<Uuid, AccountMeta> {
        [
            meta(1, AccountKind::Asset, false),    // operating bank
            meta(2, AccountKind::Asset, true),     // trust bank
            meta(3, AccountKind::Asset, false),    // accounts receivable
            meta(4, AccountKind::Liability, true), // security deposits held
            meta(5, AccountKind::Income, false),   // rent income
            meta(6, AccountKind::Expense, false),  // property expenses
        ]
        .into_iter()
        .collect()
    }

    fn debit(id: u128, amount: i64) -> Leg {
        Leg::debit(Uuid::from_u128(id), amount)
    }

    fn credit(id: u128, amount: i64) -> Leg {
        Leg::credit(Uuid::from_u128(id), amount)
    }

    #[test]
    fn balanced_posting_validates() {
        // Rent received: Dr operating bank / Cr AR.
        let legs = vec![debit(1, 185_000), credit(3, 185_000)];
        assert!(validate_legs(&legs, &accounts(), entity_id()).is_ok());
    }

    #[test]
    fn unbalanced_posting_rejected() {
        let legs = vec![debit(1, 185_000), credit(5, 184_999)];
        let err = validate_legs(&legs, &accounts(), entity_id()).unwrap_err();
        assert!(err.to_string().contains("unbalanced"));
    }

    #[test]
    fn single_leg_and_nonpositive_amounts_rejected() {
        assert!(validate_legs(&[debit(1, 100)], &accounts(), entity_id()).is_err());
        let legs = vec![debit(1, 0), credit(5, 0)];
        assert!(validate_legs(&legs, &accounts(), entity_id()).is_err());
        let legs = vec![debit(1, -5), credit(5, -5)];
        assert!(validate_legs(&legs, &accounts(), entity_id()).is_err());
    }

    #[test]
    fn cross_entity_account_rejected() {
        let mut accs = accounts();
        // Account 7 belongs to a different entity.
        let foreign = Uuid::from_u128(7);
        accs.insert(
            foreign,
            AccountMeta {
                entity_id: Uuid::from_u128(999),
                kind: AccountKind::Asset,
                is_trust: false,
            },
        );
        let legs = vec![debit(7, 100), credit(5, 100)];
        let err = validate_legs(&legs, &accs, entity_id()).unwrap_err();
        assert!(err.to_string().contains("span two entities"));
    }

    #[test]
    fn deposit_into_trust_validates() {
        // Security deposit: Dr trust bank / Cr deposits held — escrow cash and
        // the amount owed back rise together.
        let legs = vec![debit(2, 250_000), credit(4, 250_000)];
        assert!(validate_legs(&legs, &accounts(), entity_id()).is_ok());
    }

    #[test]
    fn deposit_return_from_trust_validates() {
        // Returning the deposit reverses both sides.
        let legs = vec![debit(4, 250_000), credit(2, 250_000)];
        assert!(validate_legs(&legs, &accounts(), entity_id()).is_ok());
    }

    #[test]
    fn trust_cash_cannot_fund_operating() {
        // Moving escrow cash into rent income without touching the liability
        // is the commingling the invariant forbids.
        let legs = vec![debit(1, 250_000), credit(2, 250_000)];
        let err = validate_legs(&legs, &accounts(), entity_id()).unwrap_err();
        assert!(err.to_string().contains("trust integrity"));
    }

    #[test]
    fn operating_expense_posting_validates() {
        // Paying an expense: Dr expenses / Cr operating bank.
        let legs = vec![debit(6, 42_000), credit(1, 42_000)];
        assert!(validate_legs(&legs, &accounts(), entity_id()).is_ok());
    }

    #[test]
    fn normal_balances_by_kind() {
        assert!(AccountKind::Asset.debit_normal());
        assert!(AccountKind::Expense.debit_normal());
        assert!(!AccountKind::Liability.debit_normal());
        assert!(!AccountKind::Equity.debit_normal());
        assert!(!AccountKind::Income.debit_normal());
    }

    #[test]
    fn default_chart_is_wellformed() {
        // Unique codes + subtypes, every kind parseable, trust flags on the
        // escrow pair.
        let mut codes: Vec<&str> = DEFAULT_CHART.iter().map(|d| d.code).collect();
        codes.sort();
        codes.dedup();
        assert_eq!(codes.len(), DEFAULT_CHART.len());
        for def in DEFAULT_CHART {
            assert!(AccountKind::parse(def.kind).is_some(), "kind {}", def.kind);
        }
        let trust: Vec<&str> = DEFAULT_CHART
            .iter()
            .filter(|d| d.is_trust)
            .map(|d| d.subtype)
            .collect();
        assert_eq!(
            trust,
            vec![subtypes::TRUST_BANK, subtypes::SECURITY_DEPOSITS]
        );
    }
}
