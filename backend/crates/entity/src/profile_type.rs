//! The **persona catalog** ("profile types"). A persona describes what kind of
//! actor a [`crate::membership`] represents — an Acre employee (platform scope)
//! or a person inside a client workspace (tenant scope: landlord, back-office,
//! property manager, renter, …). Seeded from `api::rbac::PROFILE_TYPES`.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "profile_type")]
pub struct Model {
    /// Stable key, e.g. `landlord`, `acre_support`.
    #[sea_orm(primary_key, auto_increment = false)]
    pub key: String,
    /// `platform` or `tenant`.
    pub scope: String,
    pub label: String,
    pub description: String,
    /// Default system-role key granted when a member is created with this persona.
    pub default_role: String,
    pub is_system: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
