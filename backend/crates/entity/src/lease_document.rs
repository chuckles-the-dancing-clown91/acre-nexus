//! A **lease document** is a generated residential-lease agreement: the tenant's
//! `theme.legal_templates` rendered against the lease, its charges (fees /
//! discounts / amenities), the resident's attributes (pets), and their vehicles.
//! It carries a simple signing state (`draft` → `sent` → `signed`) with a typed
//! signature name + timestamp.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "lease_document")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub lease_id: Uuid,
    pub title: String,
    /// The fully rendered document body.
    pub body: String,
    /// `text` | `html`.
    pub format: String,
    /// `draft` | `sent` | `signed`.
    pub status: String,
    pub generated_at: DateTimeWithTimeZone,
    pub signed_at: Option<DateTimeWithTimeZone>,
    /// The typed signature name of the signer.
    pub signed_by: Option<String>,
    /// SHA-256 (hex) of `body` at signing time — proves the signed text is unchanged.
    pub signed_hash: Option<String>,
    /// The signer's IP address, for the e-signature audit trail.
    pub signed_ip: Option<String>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
