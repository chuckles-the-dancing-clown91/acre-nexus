//! The **permission catalog**: every permission key the platform recognises,
//! with UI metadata. Seeded from `api::rbac::PERMISSION_CATALOG`. Roles grant
//! these keys via `role_permission`; the Acre dashboard reads this table to
//! present a permission picker, and custom permissions can be appended here.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "permission")]
pub struct Model {
    /// Dot/colon-keyed permission, e.g. `property:read`.
    #[sea_orm(primary_key, auto_increment = false)]
    pub key: String,
    pub category: String,
    pub label: String,
    pub description: String,
    /// `platform`, `tenant`, or `both`.
    pub scope: String,
    pub is_system: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
