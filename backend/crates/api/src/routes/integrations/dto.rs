use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A stored credential, masked for display. The plaintext value is **never**
/// serialized into any response — this is the whole surface.
#[derive(Serialize, schemars::JsonSchema)]
pub struct SecretDto {
    pub id: Uuid,
    /// Dotted credential key, e.g. `stripe.api_key`.
    pub key: String,
    /// Last four characters for display (`••••1234`).
    pub last4: String,
    pub created_at: String,
    /// Set when the value has been rotated since creation.
    pub rotated_at: Option<String>,
}

impl From<entity::secret::Model> for SecretDto {
    fn from(s: entity::secret::Model) -> Self {
        SecretDto {
            id: s.id,
            key: s.key,
            last4: s.last4,
            created_at: s.created_at.to_rfc3339(),
            rotated_at: s.rotated_at.map(|t| t.to_rfc3339()),
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SetSecretReq {
    /// Dotted credential key, e.g. `stripe.api_key` or `webhook.stripe.secret`.
    pub key: String,
    /// The credential value. Stored encrypted; only `last4` is ever shown back.
    pub value: String,
}

/// One outbound notification (email/SMS) from the send history.
#[derive(Serialize, schemars::JsonSchema)]
pub struct NotificationDto {
    pub id: Uuid,
    pub channel: String,
    pub template_key: String,
    pub recipient: String,
    pub status: String,
    pub provider_message_id: Option<String>,
    pub subject: Option<String>,
    pub body: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
}

impl From<entity::notification::Model> for NotificationDto {
    fn from(n: entity::notification::Model) -> Self {
        NotificationDto {
            id: n.id,
            channel: n.channel,
            template_key: n.template_key,
            recipient: n.recipient,
            status: n.status,
            provider_message_id: n.provider_message_id,
            subject: n.subject,
            body: n.body,
            last_error: n.last_error,
            created_at: n.created_at.to_rfc3339(),
        }
    }
}
