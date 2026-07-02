use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// One in-app inbox entry for the signed-in user.
#[derive(Serialize, schemars::JsonSchema)]
pub struct InboxEntryDto {
    pub id: Uuid,
    pub template_key: String,
    pub subject: Option<String>,
    pub body: Option<String>,
    pub read_at: Option<String>,
    pub created_at: String,
}

impl From<entity::notification::Model> for InboxEntryDto {
    fn from(n: entity::notification::Model) -> Self {
        InboxEntryDto {
            id: n.id,
            template_key: n.template_key,
            subject: n.subject,
            body: n.body,
            read_at: n.read_at.map(|t| t.to_rfc3339()),
            created_at: n.created_at.to_rfc3339(),
        }
    }
}

/// A browser push subscription, exactly as `PushManager.subscribe()` shapes it.
#[derive(Deserialize, schemars::JsonSchema)]
pub struct PushSubscribeReq {
    pub endpoint: String,
    /// Client public key (base64url).
    pub p256dh: String,
    /// Client auth secret (base64url).
    pub auth: String,
}
