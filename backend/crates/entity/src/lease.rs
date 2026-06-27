//! A **lease** is a tenancy agreement: who is renting, which unit, the terms, and
//! the current rental + payment status. Tenant identity is stored inline (like an
//! [`super::application`]) so residents need not be platform users.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "lease")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub property_id: Uuid,
    pub unit_id: Option<Uuid>,
    pub tenant_name: String,
    pub tenant_email: Option<String>,
    pub tenant_phone: Option<String>,
    /// Monthly rent in cents.
    pub rent_cents: i64,
    pub deposit_cents: Option<i64>,
    pub start_date: String,
    pub end_date: Option<String>,
    /// `upcoming` | `active` | `notice` | `expired` | `ended`.
    pub status: String,
    /// Resident's payment standing: `current` | `late` | `partial`.
    pub payment_status: String,
    /// Outstanding balance owed, in cents (negative = credit).
    pub balance_cents: i64,
    pub notes: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
