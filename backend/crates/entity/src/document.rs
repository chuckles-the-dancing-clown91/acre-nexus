//! A stored **document**: one file in the object store plus its metadata row.
//!
//! Documents are polymorphic — `owner_type` + `owner_id` attach a file to a
//! property, lease, application, entity (counterparty), deal, unit, or ticket —
//! so e-signed PDFs, due-diligence rooms, rehab photos, and notices all share
//! this one service. The blob is an opaque object keyed by `storage_key`; this
//! row is the only source of truth for metadata.
//!
//! Re-uploading the same filename against the same owner creates a **new
//! version** (`version` + `previous_version_id`) instead of destroying history.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "document")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// Owning record kind: `property` | `lease` | `application` | `entity` |
    /// `deal` | `unit` | `maintenance_ticket` | `tenant`.
    pub owner_type: String,
    pub owner_id: Uuid,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: i64,
    /// SHA-256 of the stored bytes (hex). Client-declared until the store
    /// confirms receipt; the local backend overwrites it on upload.
    pub checksum: Option<String>,
    /// 1-based version among documents sharing (owner, filename).
    pub version: i32,
    /// The version this upload superseded, if any.
    pub previous_version_id: Option<Uuid>,
    /// Object-store key the blob lives under (`{tenant_id}/{document_id}`).
    pub storage_key: String,
    /// `pending_upload` until bytes land in the store, then `stored`.
    pub status: String,
    /// When set, a retention job hard-deletes the document after this instant.
    pub retention_expires_at: Option<DateTimeWithTimeZone>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
