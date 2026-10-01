//! One **text** in an [`super::sms_thread`]: `in` from the person, `out` from the
//! console or an automatic notification.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sms_message")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub thread_id: Uuid,
    /// `in` | `out`
    pub direction: String,
    #[sea_orm(column_type = "Text")]
    pub body: String,
    /// `received` | `queued` | `sent` | `failed` | `blocked`
    pub status: String,
    pub provider_message_id: Option<String>,
    pub template_key: Option<String>,
    pub sent_by_user_id: Option<Uuid>,
    pub media_count: i32,
    /// Photos that came with the text, once filed:
    /// `[{ "document_id": …, "content_type": … }]`.
    pub media: Json,
    pub error: Option<String>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
