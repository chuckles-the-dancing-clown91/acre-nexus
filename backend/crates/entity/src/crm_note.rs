//! A CRM timeline entry about an owner, owner lead, vendor or property, optionally pinned and with a follow-up date.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "crm_note")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// `owner` | `owner_lead` | `counterparty` | `property`
    pub subject_type: String,
    pub subject_id: Uuid,
    pub property_id: Option<Uuid>,
    /// `note` | `call` | `email` | `meeting` | `issue` | `text` | `update`
    pub kind: String,
    #[sea_orm(column_type = "Text")]
    pub body: String,
    pub pinned: bool,
    pub follow_up_on: Option<String>,
    pub follow_up_done_at: Option<DateTimeWithTimeZone>,
    pub follow_up_done_by: Option<Uuid>,
    pub author_id: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
