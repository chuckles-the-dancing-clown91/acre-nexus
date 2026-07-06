//! A **property** is a managed building/asset in a tenant's portfolio. Rent is
//! stored as integer cents in `monthly_rent_cents`.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "property")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub llc_id: Option<Uuid>,
    /// Optional [`crate::portfolio`] grouping (investor / strategy / region).
    pub portfolio_id: Option<Uuid>,
    pub name: String,
    pub address: String,
    pub city: String,
    pub units: i32,
    pub occupied_units: i32,
    /// Gross monthly rent roll, in cents.
    pub monthly_rent_cents: i64,
    /// `Stabilized` | `Vacant` | `Lease-up` | `Renovating`.
    pub status: String,
    pub year_built: i32,
    pub manager: String,
    /// Investor-entered property type: `single_family` | `multi_family` |
    /// `condo` | `townhome` | `commercial` | `land`.
    pub property_type: String,
    /// Investment strategy driving the workflow: `rental` | `flip` | `brrrr` |
    /// `hold` | `wholesale`.
    pub strategy: String,
    /// Current stage in the strategy's workflow (empty = not started).
    pub workflow_stage: String,
    /// Acquisition price, in cents (if owned/acquired).
    pub purchase_price_cents: Option<i64>,
    /// Acquisition date (`YYYY-MM-DD`).
    pub acquired_on: Option<String>,
    /// Hero photo shown top-left on the property profile (object-store / CDN URL).
    pub image_url: Option<String>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
