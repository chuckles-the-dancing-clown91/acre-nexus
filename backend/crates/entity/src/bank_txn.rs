//! A **bank transaction** is one line of a linked [`crate::bank_account`]'s
//! feed (Plaid or simulated). Reconciliation matches incoming deposits against
//! settled [`crate::lease_payment`]s — auto-matched by amount + timing, or
//! manually from the console; `ignored` parks noise (bank fees, transfers).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "bank_txn")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub bank_account_id: Uuid,
    /// `YYYY-MM-DD`.
    pub posted_date: String,
    pub description: String,
    /// Signed: positive = deposit into the account.
    pub amount_cents: i64,
    /// Provider transaction id — dedupes re-syncs.
    pub external_id: String,
    /// `unmatched` | `matched` | `ignored`.
    pub status: String,
    /// The settled payment this deposit reconciled against.
    pub matched_payment_id: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
