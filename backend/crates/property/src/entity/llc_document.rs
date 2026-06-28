//! A file uploaded against an LLC during onboarding: the logo, formation docs
//! (articles of organization / operating agreement), the EIN / tax-ID letter,
//! a W-9, business licenses, insurance certificates, etc.
//!
//! The **bytes** live in the configured object store (local / S3 / GCS — see the
//! storage service); this row holds only the metadata and the `storage_key`
//! needed to fetch them back. `sha256` lets us verify integrity on download.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "llc_document")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub llc_id: Uuid,
    /// `logo` | `articles_of_organization` | `operating_agreement` | `ein_letter`
    /// | `w9` | `business_license` | `insurance` | `other`.
    pub kind: String,
    pub title: Option<String>,
    pub original_filename: String,
    pub mime_type: String,
    pub size_bytes: i64,
    /// Which storage backend holds the bytes: `platform` | `local` | `s3` | `gcs`.
    pub storage_provider: String,
    /// Opaque object key / path within that backend.
    pub storage_key: String,
    /// Hex SHA-256 of the bytes, for integrity verification on read.
    pub sha256: String,
    pub uploaded_by: Option<Uuid>,
    /// When an admin marked the document verified (NULL = unverified).
    pub verified_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
