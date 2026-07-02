//! `POST /documents` — create a document (or a new version) and hand back a
//! signed upload URL.

use super::dto::{DocumentDto, UploadDocumentReq, UploadDocumentResp};
use super::{MAX_SIZE_BYTES, OWNER_TYPES};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::storage::{ObjectStore, SIGNED_URL_TTL_SECS};
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::Document;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use uuid::Uuid;

/// `POST /documents` — register a file against an owner record and receive a
/// short-lived signed `PUT` URL for the bytes. Re-uploading the same filename
/// against the same owner creates the **next version** and links the previous
/// one instead of overwriting it.
#[rocket_okapi::openapi(tag = "Documents")]
#[post("/documents", data = "<body>")]
pub async fn upload(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<UploadDocumentReq>,
) -> ApiResult<Json<UploadDocumentResp>> {
    user.require(Permission::DocumentManage)?;
    crate::modules::require_enabled(&db, scope.tenant_id, "integrations").await?;

    let b = body.into_inner();
    let owner_type = b.owner_type.trim().to_lowercase();
    if !OWNER_TYPES.contains(&owner_type.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "invalid owner_type: {owner_type} (expected one of {})",
            OWNER_TYPES.join(", ")
        )));
    }
    let filename = b.filename.trim().to_string();
    if filename.is_empty() || filename.contains('/') || filename.contains('\\') {
        return Err(ApiError::BadRequest("invalid filename".into()));
    }
    let mime_type = b.mime_type.trim().to_string();
    if mime_type.is_empty() {
        return Err(ApiError::BadRequest("mime_type is required".into()));
    }
    let size = b.size_bytes.unwrap_or(0);
    if !(0..=MAX_SIZE_BYTES).contains(&size) {
        return Err(ApiError::BadRequest(format!(
            "size_bytes must be between 0 and {MAX_SIZE_BYTES}"
        )));
    }
    if let Some(days) = b.retention_days {
        if days <= 0 {
            return Err(ApiError::BadRequest(
                "retention_days must be positive when set".into(),
            ));
        }
    }

    // Versioning: the newest existing (owner, filename) row is the predecessor.
    let previous = Document::find()
        .filter(entity::document::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::document::Column::OwnerType.eq(owner_type.clone()))
        .filter(entity::document::Column::OwnerId.eq(b.owner_id))
        .filter(entity::document::Column::Filename.eq(filename.clone()))
        .order_by_desc(entity::document::Column::Version)
        .one(&db)
        .await?;
    let (version, previous_version_id) = match &previous {
        Some(p) => (p.version + 1, Some(p.id)),
        None => (1, None),
    };

    let id = Uuid::new_v4();
    let storage_key = format!("{}/{}", scope.tenant_id, id);
    let now = Utc::now();
    let retention_expires_at = b
        .retention_days
        .map(|days| now + chrono::Duration::days(days));

    let saved = entity::document::ActiveModel {
        id: Set(id),
        tenant_id: Set(scope.tenant_id),
        owner_type: Set(owner_type.clone()),
        owner_id: Set(b.owner_id),
        filename: Set(filename.clone()),
        mime_type: Set(mime_type),
        size_bytes: Set(size),
        checksum: Set(b.checksum.clone()),
        version: Set(version),
        previous_version_id: Set(previous_version_id),
        storage_key: Set(storage_key.clone()),
        status: Set("pending_upload".into()),
        retention_expires_at: Set(retention_expires_at.map(Into::into)),
        created_by: Set(Some(user.user_id)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;

    // Retention rides the job queue: one job per document, due at expiry.
    if let Some(expiry) = retention_expires_at {
        let delay = (expiry - now).num_seconds().max(1);
        let _ = crate::scheduler::enqueue(
            &db,
            scope.tenant_id,
            "document_retention",
            serde_json::json!({ "document_id": id }),
            delay,
        )
        .await;
    }

    let store = ObjectStore::from_env()?;
    let signed = store.signed_put_url(&storage_key, SIGNED_URL_TTL_SECS)?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::DOCUMENT_UPLOAD,
        Some("document"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({
            "owner_type": owner_type,
            "owner_id": b.owner_id,
            "filename": filename,
            "version": version,
        })),
    )
    .await;

    Ok(Json(UploadDocumentResp {
        document: DocumentDto::from(saved),
        upload_url: signed.url,
        upload_url_expires_at: signed.expires_at.to_rfc3339(),
    }))
}
