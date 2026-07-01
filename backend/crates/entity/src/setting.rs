//! A per-tenant **system setting**: one JSON-valued configuration key for a
//! firm. The set of recognized keys, their types, and defaults live in the
//! `crate::settings` catalog in the `api` crate; a row here is an override of the
//! default. One row per (tenant, key).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "setting")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// Dotted setting key, e.g. `application_reuse.enabled`.
    pub key: String,
    /// The value as JSON (bool / number / string / object per the catalog).
    pub value: Json,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
