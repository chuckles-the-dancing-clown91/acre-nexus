//! A **counterparty** is an external organisation or contact a tenant transacts
//! with — a bank/lender, insurer, title company, contractor, inspector,
//! appraiser, attorney, etc. This is the "entities registry": a place to hold who
//! everyone is and the running notes about them (see [`super::counterparty_note`]).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "counterparty")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// `bank` | `lender` | `insurer` | `title` | `contractor` | `inspector` |
    /// `appraiser` | `attorney` | `property_manager` | `utility` | `other`.
    pub kind: String,
    pub name: String,
    pub contact_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub website: Option<String>,
    pub address: Option<String>,
    /// A short summary note kept inline; longer history lives in `counterparty_note`.
    pub notes: Option<String>,
    /// Linked partner system, e.g. `alpha` (Alpha Power Wash); the API key is
    /// in the vault under `partner.<id>.api_key`.
    pub partner_kind: Option<String>,
    pub partner_base_url: Option<String>,
    /// Where the vendor's Alpha web app lives, for single sign-on.
    pub partner_web_url: Option<String>,
    pub partner_linked_at: Option<DateTimeWithTimeZone>,
    /// `ok` | `error` after the last call.
    pub partner_status: Option<String>,
    pub partner_error: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
