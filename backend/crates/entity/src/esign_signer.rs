//! One **signer** on an e-signature envelope: a named party (resident,
//! landlord, guarantor, …) who receives a tokenized signing link by email/SMS.
//! Only a SHA-256 hash of the signing token is stored — the raw token exists
//! solely inside the link sent to the signer, so possession of the link *is*
//! the credential (like a presigned URL). The signer's row carries the full
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
    /// SHA-256 (hex) of the signing-link token.
    pub token_hash: String,
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
