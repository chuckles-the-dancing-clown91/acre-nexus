use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct DocumentDto {
    pub id: Uuid,
    pub owner_type: String,
    pub owner_id: Uuid,
    pub filename: String,
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
