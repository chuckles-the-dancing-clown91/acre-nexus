use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct TokenSummary {
    pub id: Uuid,
    pub name: String,
    pub prefix: String,
    pub scopes: Vec<String>,
    pub last_used_at: Option<String>,
    pub revoked: bool,
    pub created_at: String,
}

impl From<entity::api_token::Model> for TokenSummary {
    fn from(t: entity::api_token::Model) -> Self {
        TokenSummary {
            id: t.id,
            name: t.name,
            prefix: t.prefix,
            scopes: serde_json::from_value(t.scopes).unwrap_or_default(),
            last_used_at: t.last_used_at.map(|d| d.to_rfc3339()),
            revoked: t.revoked_at.is_some(),
            created_at: t.created_at.to_rfc3339(),
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateTokenReq {
    pub name: String,
    /// Permission scopes, e.g. `["listing:read","property:read"]`.
    pub scopes: Vec<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct CreateTokenResp {
    #[serde(flatten)]
    pub summary: TokenSummary,
    /// The raw secret — shown exactly once, store it securely.
    pub token: String,
}
