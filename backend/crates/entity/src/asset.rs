//! An **asset** — a serviceable piece of equipment registered against a
//! property (optionally a unit): AC units, water heaters, appliances and
//! other utilities. Work orders reference the asset being serviced; manuals
//! and photos ride the document service (`owner_type = "asset"`).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "asset")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub property_id: Uuid,
    pub unit_id: Option<Uuid>,
    /// `hvac` | `appliance` | `plumbing` | `electrical` | `safety` |
    /// `structural` | `other`.
    pub kind: String,
    /// Display name, e.g. "AC — living room".
    pub name: String,
    pub make: Option<String>,
    pub model: Option<String>,
    pub serial_number: Option<String>,
    /// ISO date (`YYYY-MM-DD`).
    pub install_date: Option<String>,
    /// ISO date the manufacturer/extended warranty lapses.
    pub warranty_expires: Option<String>,
    pub notes: Option<String>,
    /// `active` | `retired`.
    pub status: String,
    pub created_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
