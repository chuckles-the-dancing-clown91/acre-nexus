//! One checklist line on an [`super::inspection`] — an area/item pair with a
//! recorded condition.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "inspection_item")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub inspection_id: Uuid,
    pub area: String,
    pub item: String,
    /// `unrated` | `good` | `fair` | `poor` | `damaged`.
    pub condition: String,
    pub notes: Option<String>,
    pub sort_order: i32,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
