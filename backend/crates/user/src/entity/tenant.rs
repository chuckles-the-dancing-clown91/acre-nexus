//! A **tenant** is a client company on the platform — e.g. a property-management
//! firm such as "Northwind Property Group". It is the root of data isolation:
//! almost every other row references a `tenant_id`.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "tenant")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    /// URL-safe identifier used for subdomain / `X-Tenant` routing (e.g. `northwind`).
    #[sea_orm(unique)]
    pub slug: String,
    pub name: String,
    /// Subscription plan: `starter` | `growth` | `enterprise`.
    pub plan: String,
    /// Lifecycle status: `active` | `trial` | `suspended`.
    pub status: String,
    /// Optional custom domain for white-label hosting.
    pub custom_domain: Option<String>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
