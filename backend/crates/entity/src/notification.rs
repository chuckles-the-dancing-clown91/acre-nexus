//! One **outbound notification** (transactional email or SMS): the rendered
//! message, who it went to, and how delivery went. Rows are written by the
//! `auto_email` / `auto_sms` background-job handlers so the send history is
//! durable and auditable, and `idempotency_key` keeps a retried job or a
//! duplicate trigger from double-sending.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "notification")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// `email` | `sms`.
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
    pub last_error: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
