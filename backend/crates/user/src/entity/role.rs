//! A **role** is a named bundle of permissions. System roles (`tenant_id = NULL`)
//! ship with the platform (`platform_admin`, `pm_admin`, `landlord`,
//! `maintenance`, `tenant`); tenants may also define custom roles.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "role")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    /// `NULL` for built-in system roles shared across all tenants.
    pub tenant_id: Option<Uuid>,
    /// `platform` or `tenant` — which surface the role governs.
    pub scope: String,
    /// Stable machine key, e.g. `property_manager`.
    pub key: String,
    pub name: String,
    pub description: String,
    pub is_system: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
