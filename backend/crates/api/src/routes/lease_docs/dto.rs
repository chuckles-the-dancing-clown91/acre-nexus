use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct LeaseDocDto {
    pub id: Uuid,
    pub lease_id: Uuid,
    pub title: String,
    pub body: String,
    pub format: String,
    /// `lease` | `renewal_addendum`.
    pub purpose: String,
    pub status: String,
    pub generated_at: String,
    pub signed_at: Option<String>,
    pub signed_by: Option<String>,
    /// SHA-256 (hex) of the signed body — tamper-evidence for the signature.
    pub signed_hash: Option<String>,
}

impl From<entity::lease_document::Model> for LeaseDocDto {
    fn from(d: entity::lease_document::Model) -> Self {
        LeaseDocDto {
            id: d.id,
            lease_id: d.lease_id,
            title: d.title,
            body: d.body,
            format: d.format,
            purpose: d.purpose,
            status: d.status,
            generated_at: d.generated_at.to_rfc3339(),
            signed_at: d.signed_at.map(|x| x.to_rfc3339()),
            signed_by: d.signed_by,
            signed_hash: d.signed_hash,
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SignReq {
    /// The typed signature name of the signer.
    pub signed_by: String,
}
