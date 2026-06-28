//! A **listing** is a publicly advertised rental unit shown on the tenant's
//! white-label website. May reference a `property`. `is_public` gates visibility.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "listing")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub property_id: Option<Uuid>,
    pub title: String,
    pub address: String,
    pub city: String,
    /// `0` represents a studio.
    pub beds: i32,
    pub baths: i32,
    pub sqft: i32,
    pub rent_cents: i64,
    /// `Available` | `New` | `Pending` | `Leased`.
    pub status: String,
    /// Human label for availability, e.g. `Now`, `Jul 15`.
    pub available_on: String,
    pub description: String,
    pub is_public: bool,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
