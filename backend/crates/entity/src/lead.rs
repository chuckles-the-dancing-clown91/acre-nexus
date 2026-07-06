//! A **lead** is a leasing prospect — the seed of the CRM (issue #46). Leads
//! arrive from the tenant's monitored leasing inbox (`api::mail` routes
//! inbound email here), or are entered manually, and progress
//! `new → contacted → toured → applied → closed`.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "lead")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    /// `inbound_email` | `manual` | `website`.
    pub source: String,
    /// `new` | `contacted` | `toured` | `applied` | `closed`.
    pub status: String,
    pub notes: Option<String>,
    /// The latest inbound message (subject + excerpt).
    pub last_message: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
