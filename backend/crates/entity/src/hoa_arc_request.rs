//! An **architectural review (ARC) request** submitted by an
//! [`crate::hoa_member`] (issue #13): a proposed exterior change the board
//! reviews — `submitted → approved` / `denied` (or `withdrawn`).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "hoa_arc_request")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// FK to `hoa_association.id`.
    pub association_id: Uuid,
    /// FK to `hoa_member.id`.
    pub member_id: Uuid,
    pub title: String,
    pub description: String,
    /// `submitted` | `approved` | `denied` | `withdrawn`.
    pub status: String,
    pub decision_note: Option<String>,
    pub decided_by: Option<Uuid>,
    pub decided_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
