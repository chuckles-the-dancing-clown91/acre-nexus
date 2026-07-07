//! Request/response shapes for resident ↔ manager messaging.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct MessageDto {
    pub id: Uuid,
    pub thread_id: Uuid,
    pub sender_user_id: Uuid,
    /// `resident` | `staff`.
    pub sender_kind: String,
    pub sender_name: String,
    pub body: String,
    pub created_at: String,
}

impl From<entity::message::Model> for MessageDto {
    fn from(m: entity::message::Model) -> Self {
        MessageDto {
            id: m.id,
            thread_id: m.thread_id,
            sender_user_id: m.sender_user_id,
            sender_kind: m.sender_kind,
            sender_name: m.sender_name,
            body: m.body,
            created_at: m.created_at.to_rfc3339(),
        }
    }
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ThreadDto {
    pub id: Uuid,
    pub lease_id: Uuid,
    pub property_id: Uuid,
    pub subject: String,
    /// `open` | `closed`.
    pub status: String,
    pub last_message_at: String,
    pub created_at: String,
    /// Display context for console lists (resident + property).
    pub resident_name: Option<String>,
    pub property_address: Option<String>,
    pub message_count: i64,
    /// The last message's sender + a short excerpt, for list previews.
    pub last_sender_kind: Option<String>,
    pub last_preview: Option<String>,
}

/// A thread plus its full message timeline (oldest-first, chat order).
#[derive(Serialize, schemars::JsonSchema)]
pub struct ThreadDetailDto {
    #[serde(flatten)]
    pub thread: ThreadDto,
    pub messages: Vec<MessageDto>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateThreadReq {
    pub subject: String,
    pub body: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SendMessageReq {
    pub body: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateThreadReq {
    /// `open` | `closed`.
    pub status: String,
}

/// Build a [`ThreadDto`] from a thread + its aggregates.
pub fn thread_dto(
    t: entity::message_thread::Model,
    resident_name: Option<String>,
    property_address: Option<String>,
    message_count: i64,
    last: Option<&entity::message::Model>,
) -> ThreadDto {
    ThreadDto {
        id: t.id,
        lease_id: t.lease_id,
        property_id: t.property_id,
        subject: t.subject,
        status: t.status,
        last_message_at: t.last_message_at.to_rfc3339(),
        created_at: t.created_at.to_rfc3339(),
        resident_name,
        property_address,
        message_count,
        last_sender_kind: last.map(|m| m.sender_kind.clone()),
        last_preview: last.map(|m| super::preview(&m.body)),
    }
}
