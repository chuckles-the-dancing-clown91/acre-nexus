//! A user's **TOTP MFA** enrolment (issue #63): the authenticator shared secret
//! sealed at rest (AES-256-GCM, never plaintext) plus an `enabled` flag.
//! Enrolment is two-step — a secret is stored, then confirmed with a valid code
//! before `enabled` flips true and the user is challenged for it at login.
//! 1:1 with [`super::user`], keyed by `user_id` with no `tenant_id` so it is
//! readable during the (pre-tenant) login challenge.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "user_totp")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: Uuid,
    /// The base32 TOTP secret, sealed under the PII key.
    #[serde(skip_serializing)]
    pub secret_ciphertext: String,
    #[serde(skip_serializing)]
    pub secret_nonce: String,
    pub enabled: bool,
    pub confirmed_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
