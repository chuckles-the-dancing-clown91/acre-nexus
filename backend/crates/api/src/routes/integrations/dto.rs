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

/// A configured notification delivery provider, credential masked.
#[derive(Serialize, schemars::JsonSchema)]
pub struct ProviderDto {
    pub id: Uuid,
    /// `email` | `sms` | `chat`.
    pub channel: String,
    /// `resend` | `sendgrid` | `postmark` | `twilio` | `slack` | `discord`.
    pub kind: String,
    /// Non-secret settings (from address, account sid, …).
    pub config: serde_json::Value,
    pub enabled: bool,
    pub is_default: bool,
    /// Last four characters of the vaulted credential, when one is stored.
    pub credential_last4: Option<String>,
    pub created_at: String,
}

impl ProviderDto {
    pub fn from_model(p: entity::notification_provider::Model, last4: Option<String>) -> Self {
        ProviderDto {
            id: p.id,
            channel: p.channel,
            kind: p.kind,
            config: p.config,
            enabled: p.enabled,
            is_default: p.is_default,
            credential_last4: last4,
            created_at: p.created_at.to_rfc3339(),
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateProviderReq {
    /// `email` | `sms` | `chat`.
    pub channel: String,
    /// Provider service, e.g. `resend`, `twilio`, `slack`.
    pub kind: String,
    /// Non-secret settings, e.g. `{ "from": "hello@acme.com" }`.
    pub config: Option<serde_json::Value>,
    /// The API key / auth token / webhook URL. Stored in the secrets vault.
    pub credential: Option<String>,
    pub is_default: Option<bool>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateProviderReq {
    pub config: Option<serde_json::Value>,
    /// Rotates the vaulted credential when set.
    pub credential: Option<String>,
    pub enabled: Option<bool>,
    pub is_default: Option<bool>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct TestProviderReq {
    /// Recipient override: defaults to your account email for email tests;
    /// required for SMS (a phone number). Ignored for chat.
    pub to: Option<String>,
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
