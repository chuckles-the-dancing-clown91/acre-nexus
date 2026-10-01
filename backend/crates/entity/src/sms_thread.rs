//! A **text conversation** with one phone number in a workspace (Vantedge
//! phase 2). Holds the inbox state and the number's STOP opt-out.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sms_thread")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// E.164, e.g. `+17605551234`.
    pub phone: String,
    pub lease_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub display_name: Option<String>,
    /// `open` | `done`
    pub status: String,
    pub unread_count: i32,
    pub last_preview: Option<String>,
    pub last_message_at: Option<DateTimeWithTimeZone>,
    /// Set by STOP, cleared by START. No texts go out while set.
    pub opted_out_at: Option<DateTimeWithTimeZone>,
    /// The staff member who owns the conversation.
    pub assigned_user_id: Option<Uuid>,
    /// Separate consent for marketing texts (STOP still wins over it).
    pub marketing_opt_in_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
