use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Serialize, schemars::JsonSchema)]
pub struct DocumentDto {
    pub id: Uuid,
    pub owner_type: String,
    pub owner_id: Uuid,
    pub filename: String,
    /// Filing bucket: `insurance` | `loan` | `title` | `tax` | `lease` |
    /// `inspection` | `permit` | `other` (`null` = unfiled).
    pub category: Option<String>,
    /// The original needs a physical ("wet ink") signature.
    pub requires_wet_ink: bool,
    /// Where the wet-ink original is physically filed.
    pub physical_location: Option<String>,
    pub mime_type: String,
    pub size_bytes: i64,
    /// SHA-256 (hex) of the stored bytes, once the store has them.
    pub checksum: Option<String>,
    pub version: i32,
    pub previous_version_id: Option<Uuid>,
    /// `pending_upload` until the bytes land, then `stored`.
    pub status: String,
    pub retention_expires_at: Option<String>,
    pub created_at: String,
}

impl From<entity::document::Model> for DocumentDto {
    fn from(d: entity::document::Model) -> Self {
        DocumentDto {
            id: d.id,
            owner_type: d.owner_type,
            owner_id: d.owner_id,
            filename: d.filename,
            category: d.category,
            requires_wet_ink: d.requires_wet_ink,
            physical_location: d.physical_location,
            mime_type: d.mime_type,
            size_bytes: d.size_bytes,
            checksum: d.checksum,
            version: d.version,
            previous_version_id: d.previous_version_id,
            status: d.status,
            retention_expires_at: d.retention_expires_at.map(|t| t.to_rfc3339()),
            created_at: d.created_at.to_rfc3339(),
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UploadDocumentReq {
    /// `property` | `lease` | `application` | `entity` | `deal` | `unit` |
    /// `maintenance_ticket` | `tenant`.
    pub owner_type: String,
    pub owner_id: Uuid,
    pub filename: String,
    pub mime_type: String,
    /// Declared size; the local store overwrites it with the received size.
    pub size_bytes: Option<i64>,
    /// SHA-256 (hex) declared by the client; the local store overwrites it.
    pub checksum: Option<String>,
    /// Hard-delete the document this many days after upload (compliance
    /// retention). Omit to keep forever.
    pub retention_days: Option<i64>,
    /// Filing bucket (`insurance` | `loan` | `title` | …). Omit to leave unfiled.
    pub category: Option<String>,
    /// The original needs a physical ("wet ink") signature (default false).
    pub requires_wet_ink: Option<bool>,
    /// Where the wet-ink original is stored (e.g. "Fireproof safe — HQ").
    pub physical_location: Option<String>,
}

/// Patch a document's filing metadata — notably where the wet-ink original is
/// stored, which is recorded/updated over the document's life.
#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateDocumentReq {
    pub category: Option<String>,
    pub requires_wet_ink: Option<bool>,
    /// Pass an empty string to clear the recorded storage location.
    pub physical_location: Option<String>,
}

/// The created metadata row plus a short-lived signed URL to `PUT` the bytes.
#[derive(Serialize, schemars::JsonSchema)]
pub struct UploadDocumentResp {
    pub document: DocumentDto,
    pub upload_url: String,
    pub upload_url_expires_at: String,
}

/// A short-lived signed URL to `GET` the bytes.
#[derive(Serialize, schemars::JsonSchema)]
pub struct DownloadDocumentResp {
    pub url: String,
    pub expires_at: String,
}

/// How many documents are filed under one category.
#[derive(Serialize, schemars::JsonSchema)]
pub struct CategoryCount {
    /// `null` = unfiled.
    pub category: Option<String>,
    pub count: i64,
}

/// The Documents tab for a property: the latest version of each document, a
/// per-category tally, and the subset of wet-ink originals with where each is
/// physically stored.
#[derive(Serialize, schemars::JsonSchema)]
pub struct PropertyDocumentsResp {
    pub property_id: Uuid,
    /// Count of distinct documents (latest version of each).
    pub total: i64,
    /// Latest version of every document filed against the property, newest first.
    pub documents: Vec<DocumentDto>,
    /// Per-category tallies for grouping (insurance, loan, title, …).
    pub categories: Vec<CategoryCount>,
    /// Documents whose original needs a wet-ink signature — the "where is the
    /// paper" area, each carrying its `physical_location`.
    pub wet_ink_originals: Vec<DocumentDto>,
}
