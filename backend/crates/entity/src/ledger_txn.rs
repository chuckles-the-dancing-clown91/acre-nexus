//! A **ledger transaction** is the header of one balanced double-entry
//! posting: its [`crate::ledger_entry`] legs always debit and credit equal
//! totals, enforced by the single posting path in `api::accounting`. The
//! `source_type`/`source_id` pair ties every dollar back to the domain event
//! that produced it (a payment, a late fee, a payout, a manual journal entry).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "ledger_txn")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// FK to `llc.id` — one transaction never spans two entities' books.
    pub entity_id: Uuid,
    /// Effective date, `YYYY-MM-DD`.
    pub txn_date: String,
    pub memo: String,
    /// `rent_due` | `payment` | `deposit` | `late_fee` | `payout` | `manual` | …
    pub source_type: String,
    /// The domain row that produced this posting, if any.
    pub source_id: Option<Uuid>,
    /// The user who posted it (`None` = the pipeline).
    pub posted_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
