//! Per-tenant **module enablement**. The platform is composed of pluggable
//! [modules](../../api/src/modules); each tenant can switch a module on or off
//! from their software settings. A row here is an explicit override — when no
//! row exists for a `(tenant_id, module_key)` pair, the module's own
//! `default_enabled` flag applies.
//!
//! This is what makes the product *modular and sellable per-feature*: enabling
//! the "flips" module or a future "maintenance" module is a single toggle that
//! gates both its HTTP routes and its background-job handlers for that tenant.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "tenant_module")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// Stable module key, e.g. `properties`, `leasing`, `flips`.
    pub module_key: String,
    pub enabled: bool,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
