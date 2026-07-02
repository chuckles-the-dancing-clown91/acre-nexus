//! An encrypted **integration credential** (ESP API key, payment-processor
//! secret, webhook signing secret …), sealed with AES-256-GCM under the
//! dedicated `SECRETS_ENC_KEY` — never the PII key, so the two are independently
//! rotatable. `tenant_id` is `NULL` for platform-wide secrets; a tenant row with
//! the same `key` shadows the platform default.
//!
//! Plaintext is never serialized into an API response: reads go through
//! `api::secrets::reveal` (server-side only) and the settings UI only ever sees
//! `last4`.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "secret")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    /// `NULL` = platform-wide secret (Acre HQ); otherwise tenant-scoped.
    pub tenant_id: Option<Uuid>,
    /// Dotted credential key, e.g. `stripe.api_key` or `webhook.stripe.secret`.
    pub key: String,
    /// Base64 AES-256-GCM ciphertext of the credential value.
    pub ciphertext: String,
    /// Base64 96-bit nonce used for this ciphertext.
    pub nonce: String,
    /// Last four characters in clear, for masked display (`••••1234`).
    pub last4: String,
    pub created_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    /// Set whenever the value is replaced after initial creation.
    pub rotated_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
