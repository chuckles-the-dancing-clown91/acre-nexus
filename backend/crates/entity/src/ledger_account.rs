//! A **ledger account** is one line of an entity's chart of accounts. The GL is
//! partitioned per legal entity ([`crate::llc`]): each LLC keeps its own books.
//! Seeded *system* accounts carry a stable `subtype` (e.g. `operating_bank`,
//! `accounts_receivable`, `rent_income`) that posting rules resolve by;
//! `is_trust` marks segregated escrow accounts guarded by the no-commingling
//! invariant (see `api::accounting`).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "ledger_account")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// FK to `llc.id` — the legal entity whose books this account belongs to.
    pub entity_id: Uuid,
    /// Short numeric code, e.g. `1000`. Unique per (tenant, entity).
    pub code: String,
    pub name: String,
    /// `asset` | `liability` | `equity` | `income` | `expense`.
    pub kind: String,
    /// Stable hook for posting rules (`operating_bank` | `trust_bank` |
    /// `accounts_receivable` | `security_deposits` | `rent_income` | …).
    pub subtype: Option<String>,
    /// Trust/escrow account — no commingling with operating funds.
    pub is_trust: bool,
    /// Seeded default-chart account (vs a custom addition).
    pub system: bool,
    pub active: bool,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
