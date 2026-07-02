//! One **outbound notification** (email, SMS, Web Push, chat message, or
//! in-app inbox entry): the rendered message, who it went to, and how delivery
//! went. Rows are written by the notification job handlers (and directly for
//! in-app) so the send history is durable and auditable; `idempotency_key`
//! keeps a retried job or a duplicate trigger from double-sending, and
//! `user_id` + `read_at` power the per-user in-app inbox.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "notification")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// `email` | `sms` | `push` | `chat` | `in_app`.
    pub channel: String,
    /// Template that produced the body, e.g. `application_approved`.
    pub template_key: String,
    /// Email address or phone number the message was sent to.
    pub recipient: String,
    /// `queued` → `sent` | `failed`.
    pub status: String,
    /// Message id returned by the (real or simulated) delivery provider.
    pub provider_message_id: Option<String>,
    /// Rendered subject (email only).
    pub subject: Option<String>,
    /// Rendered message body, kept for the audit/history trail.
    pub body: Option<String>,
    /// The `background_job` that performed (or is performing) the send.
    pub background_job_id: Option<Uuid>,
    /// Natural de-duplication key: `{template}:{owner_type}:{owner_id}:{trigger}`.
    pub idempotency_key: Option<String>,
    /// The addressed user, for user-directed channels (`in_app`, `push`).
    pub user_id: Option<Uuid>,
    /// When the user read this in-app notification (unread while `NULL`).
    pub read_at: Option<DateTimeWithTimeZone>,
    pub last_error: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
