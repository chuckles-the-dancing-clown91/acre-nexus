//! A **CC&R violation** logged against an [`crate::hoa_member`] (issue #13): the
//! covenant-enforcement lifecycle `open → cured` / `fined → closed`, with an
//! optional fine. Money is integer cents.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "hoa_violation")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// FK to `hoa_association.id`.
    pub association_id: Uuid,
    /// FK to `hoa_member.id`.
    pub member_id: Uuid,
    /// Category, e.g. "landscaping", "parking", "noise".
    pub kind: String,
    pub description: String,
    /// `open` | `cured` | `fined` | `closed`.
    pub status: String,
    /// Fine assessed, in cents (0 until fined).
    pub fine_cents: i64,
    pub resolved_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
