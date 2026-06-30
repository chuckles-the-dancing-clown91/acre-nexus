//! A **bank account** scoped to one legal entity ([`crate::llc`]). Two kinds:
//! `operating` (the entity's own funds) and `trust`/escrow (funds held on behalf
//! of owners/renters). Trust accounts carry the **commingling invariant**: no
//! posting may move funds between two entities' trust ledgers — enforced in the
//! accounting domain (see `crate::accounting`), not just the UI.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "bank_account")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// FK to `llc.id` — the legal entity that holds this account.
    pub entity_id: Uuid,
    /// `operating` | `trust`.
    pub kind: String,
    pub institution: String,
    /// Masked account number for display (e.g. `••••4321`).
    pub masked_number: Option<String>,
    /// `active` | `closed` | `pending`.
    pub status: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
