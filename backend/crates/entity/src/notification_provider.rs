//! A tenant's configured **notification delivery provider** for one channel:
//! Resend / SendGrid / Postmark for email, Twilio for SMS, Slack / Discord for
//! chat. Non-secret settings (from address, account sid, …) live in `config`;
//! the API credential lives in the secrets vault under `secret_ref` and is
//! never stored or returned in plaintext. One provider per channel may be the
//! tenant's default; channels with no configured provider fall back to the
//! platform's simulated sender.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "notification_provider")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// `email` | `sms` | `chat`.
    pub channel: String,
    /// `resend` | `sendgrid` | `postmark` | `twilio` | `slack` | `discord`.
    pub kind: String,
    /// Non-secret provider settings, e.g. `{ "from": "hello@acme.com" }`.
    pub config: Json,
    /// Secrets-vault key holding the API credential (e.g.
    /// `provider.<id>.credential`).
    pub secret_ref: Option<String>,
    pub enabled: bool,
    /// The provider the channel routes through when several are configured.
    pub is_default: bool,
    pub created_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
