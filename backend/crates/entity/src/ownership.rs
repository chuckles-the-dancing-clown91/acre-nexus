//! An **ownership** record — who holds title (the deed) to a property and how it
//! is vested. Multiple rows model fractional ownership (tenancy-in-common etc.);
//! `percent_bps` is each holder's share in basis points (100% = 10000).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "ownership")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub property_id: Uuid,
    /// `llc` | `entity` | `individual` | `external`.
    pub owner_kind: String,
    /// Links to an `llc` or `counterparty` when `owner_kind` is `llc`/`entity`.
    pub owner_id: Option<Uuid>,
    pub owner_name: String,
    /// How title is held, e.g. "Sole ownership", "Joint tenants", "TIC".
    pub vesting: Option<String>,
    /// Ownership share in basis points (10000 = 100%).
    pub percent_bps: i32,
    /// `Warranty` | `Grant` | `Quitclaim` | …
    pub deed_type: Option<String>,
    pub deed_recorded_date: Option<String>,
    /// Recording reference (book/page or instrument number).
    pub deed_reference: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
