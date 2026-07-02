//! One **signer** on an e-signature envelope: a named party (resident,
//! landlord, guarantor, …) who receives a tokenized signing link by email/SMS.
//! Possession of the link *is* the credential (like a presigned URL). The
//! token is stored two ways: a SHA-256 hash for lookup, and an AES-256-GCM
//! seal under the integration-secrets key so reminders can re-send the *same*
//! link (never plaintext at rest). The signer's row carries the full
//! per-party signature record: typed name, timestamp, IP, and user agent.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "esign_signer")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub envelope_id: Uuid,
    /// `resident` | `landlord` | `guarantor` | `other`.
    pub role: String,
    pub name: String,
    pub email: String,
    /// Optional mobile number — when present the signing link also goes by SMS.
    pub phone: Option<String>,
    /// SHA-256 (hex) of the signing-link token — the lookup key.
    pub token_hash: String,
    /// The raw token sealed with AES-256-GCM under `SECRETS_ENC_KEY`
    /// (base64), so reminders re-send the original link.
    pub token_ciphertext: String,
    /// The seal's 96-bit nonce (base64).
    pub token_nonce: String,
    /// `sent` | `viewed` | `signed` | `declined`.
    pub status: String,
    pub viewed_at: Option<DateTimeWithTimeZone>,
    pub signed_at: Option<DateTimeWithTimeZone>,
    /// The typed signature captured at signing (ESIGN/UETA "intent to sign").
    pub signed_name: Option<String>,
    pub signed_ip: Option<String>,
    pub signed_user_agent: Option<String>,
    pub decline_reason: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
