//! Per-tenant **object-storage configuration** — exactly one row per tenant.
//!
//! The platform offers managed storage (`provider = "platform"`, the default)
//! *or* a tenant can bring their own bucket: `local`, `s3`, or `gcs`. Non-secret
//! settings (bucket / region / prefix / endpoint) are stored in clear; the
//! credential blob (S3 access/secret keys, or a GCS service-account JSON) is
//! sealed with AES-256-GCM (same scheme as PII) into `secret_ciphertext` +
//! `secret_nonce`, so secrets are never persisted in the clear.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "tenant_storage_config")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    #[sea_orm(unique)]
    pub tenant_id: Uuid,
    /// `platform` (managed) | `local` | `s3` | `gcs`.
    pub provider: String,
    pub bucket: Option<String>,
    pub region: Option<String>,
    /// Key prefix / sub-path within the bucket (or base dir for `local`).
    pub prefix: Option<String>,
    /// Custom endpoint for S3-compatible stores (MinIO, R2, …).
    pub endpoint: Option<String>,
    /// Sealed credential blob (base64 ciphertext); NULL for `platform`/`local`.
    pub secret_ciphertext: Option<String>,
    pub secret_nonce: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
