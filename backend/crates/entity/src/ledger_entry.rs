//! A **ledger entry** is one debit or credit leg of a [`crate::ledger_txn`].
//! Amounts are always positive; `side` says which way the account moves.
//! Optional property/lease dimensions make per-asset reporting cheap without
//! denormalizing the chart of accounts.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "ledger_entry")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub txn_id: Uuid,
    pub account_id: Uuid,
    /// `debit` | `credit`.
    pub side: String,
    /// Always positive.
    pub amount_cents: i64,
    pub property_id: Option<Uuid>,
    pub lease_id: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
