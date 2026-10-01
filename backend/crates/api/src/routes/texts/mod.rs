//! **Two-way texts** (Vantedge phase 2): the console text inbox and the Twilio
//! webhooks. Read with `message:read`; reply, start, close and simulate with
//! `message:manage`. See [`crate::texts`] for the STOP rules and threading.

pub mod console;
pub mod replies;
pub mod twilio;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct TextThreadDto {
    pub id: Uuid,
    pub phone: String,
    pub display_name: Option<String>,
    pub lease_id: Option<Uuid>,
    /// `open` | `done`
    pub status: String,
    pub unread_count: i32,
    pub last_preview: Option<String>,
    pub last_message_at: Option<String>,
    /// True once the number texted STOP (until it texts START).
    pub opted_out: bool,
    /// The staff member who owns the conversation.
    pub assigned_user_id: Option<Uuid>,
    /// Consent on file for marketing texts.
    pub marketing_consent: bool,
}

impl From<entity::sms_thread::Model> for TextThreadDto {
    fn from(t: entity::sms_thread::Model) -> Self {
        TextThreadDto {
            id: t.id,
            phone: t.phone,
            display_name: t.display_name,
            lease_id: t.lease_id,
            status: t.status,
            unread_count: t.unread_count,
            last_preview: t.last_preview,
            last_message_at: t.last_message_at.map(|d| d.to_rfc3339()),
            opted_out: t.opted_out_at.is_some(),
            assigned_user_id: t.assigned_user_id,
            marketing_consent: t.marketing_opt_in_at.is_some(),
        }
    }
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct TextMessageDto {
    pub id: Uuid,
    /// `in` | `out`
    pub direction: String,
    pub body: String,
    /// `received` | `queued` | `sent` | `failed` | `blocked`
    pub status: String,
    /// The notification that produced an automatic text (e.g. `payment_receipt`).
    pub template_key: Option<String>,
    pub sent_by: Option<String>,
    pub media_count: i32,
    /// Photos that came with the text, once filed as documents.
    pub media: Vec<TextMediaDto>,
    pub error: Option<String>,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
pub struct TextMediaDto {
    pub document_id: Uuid,
    pub content_type: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct TextThreadDetailDto {
    pub thread: TextThreadDto,
    pub messages: Vec<TextMessageDto>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SendTextReq {
    pub body: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct StartTextReq {
    pub phone: String,
    pub body: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateTextThreadReq {
    /// `open` | `done`
    pub status: Option<String>,
    /// A staff member's user id to own the conversation; `""` to clear.
    pub assignee: Option<String>,
    /// Record (true) or withdraw (false) consent to marketing texts.
    pub marketing_consent: Option<bool>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct SavedReplyDto {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub updated_at: String,
}

impl From<entity::text_saved_reply::Model> for SavedReplyDto {
    fn from(r: entity::text_saved_reply::Model) -> Self {
        SavedReplyDto {
            id: r.id,
            title: r.title,
            body: r.body,
            updated_at: r.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SavedReplyReq {
    pub title: String,
    pub body: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SimulateInboundReq {
    pub phone: String,
    pub body: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct TextsStatusDto {
    /// True when texts really go out through Twilio (`LIVE_PROVIDERS` has `sms`).
    pub live: bool,
    /// True when the workspace has a Twilio provider set up.
    pub provider_configured: bool,
    /// Where to point the Twilio number's "A message comes in" webhook.
    pub inbound_webhook_url: String,
    /// Where to point Twilio's status callback.
    pub status_webhook_url: String,
    /// Open threads with unread texts.
    pub unread_threads: i64,
}
