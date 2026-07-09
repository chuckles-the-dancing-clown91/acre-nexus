//! A **homeowner member** of an [`crate::hoa_association`] (issue #13) — one
//! owner of one unit/lot in the community. Assessments, violations, and ARC
//! requests attach to a member.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "hoa_member")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// FK to `hoa_association.id`.
    pub association_id: Uuid,
    pub name: String,
    /// Unit / lot label within the community (e.g. "Unit 4B", "Lot 12").
    pub unit_label: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    /// `active` | `inactive`.
    pub status: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
