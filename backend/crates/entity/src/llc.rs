//! An **LLC** is a legal holding entity (the tenancy spec's `legal_entities`)
//! owned by a tenant. Properties are grouped under LLCs (e.g. "Maple Holdings
//! LLC" owns The Maple Court & Birchwood Lofts), each LLC has its own cap table
//! ([`crate::entity_ownership`]) and bank accounts ([`crate::bank_account`]), and
//! holds title to its properties. LLC separation lives in the accounting +
//! permission layers, not the RLS wall — see `docs/TENANCY.md`.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "llc")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub ein: String,
    /// Two-letter state of registration (the spec's `formation_state`).
    pub state: String,
    /// `llc` | `lp` | `s_corp` | `c_corp` | `sole_prop`.
    pub entity_type: String,
    /// Registered agent of record, if tracked.
    pub registered_agent: Option<String>,
    /// `active` | `dissolved` | `pending`.
    pub status: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
