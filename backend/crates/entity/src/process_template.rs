//! A **process template**: the recipe for one kind of step process (a unit
//! turnover, a house onboarding). Its steps are in
//! [`super::process_template_step`].

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "process_template")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// `turnover` | `onboarding` | `site_turn`.
    pub kind: String,
    pub name: String,
    /// The template new runs of this kind start from.
    pub is_default: bool,
    pub active: bool,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
