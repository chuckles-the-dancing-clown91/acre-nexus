//! LLC document endpoints: list, **multipart upload**, binary download, delete.
//!
//! Upload and download are plain Rocket routes (not in the OpenAPI doc) because
//! they carry `multipart/form-data` and raw bytes respectively, which the JSON
//! schema generator doesn't model.

use super::dto::LlcDocumentDto;
use super::helpers::{parse_uuid, require_llc, sha256_hex};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::storage;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::LlcDocument;
use rocket::form::Form;
use rocket::fs::TempFile;
use rocket::http::ContentType;
use rocket::serde::json::Json;
use rocket::{delete, get, post, FromForm, State};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, ModelTrait, QueryFilter, QueryOrder, Set,
};
use tokio::io::AsyncReadExt;
use uuid::Uuid;

/// `GET /llcs/<id>/documents` — list an LLC's uploaded documents.
#[rocket_okapi::openapi(tag = "LLCs")]
#[get("/llcs/<id>/documents")]
pub async fn list_documents(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Vec<LlcDocumentDto>>> {
    user.require(Permission::LlcRead)?;
    let llc_id = parse_uuid(id)?;
    require_llc(state, scope.tenant_id, llc_id).await?;
    let rows = LlcDocument::find()
        .filter(entity::llc_document::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::llc_document::Column::LlcId.eq(llc_id))
        .order_by_desc(entity::llc_document::Column::CreatedAt)
        .all(&state.property_db)
        .await?;
    Ok(Json(rows.into_iter().map(LlcDocumentDto::from).collect()))
}

/// Multipart body for an upload: the file plus its `kind` and optional `title`.
#[derive(FromForm)]
pub struct UploadForm<'r> {
    /// `logo` | `articles_of_organization` | `operating_agreement` | `ein_letter`
    /// | `w9` | `business_license` | `insurance` | `other`.
    pub kind: String,
    pub title: Option<String>,
    pub file: TempFile<'r>,
}

/// `POST /llcs/<id>/documents` — upload a document (multipart/form-data).
#[post("/llcs/<id>/documents", data = "<form>")]
pub async fn upload_document(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    form: Form<UploadForm<'_>>,
) -> ApiResult<Json<LlcDocumentDto>> {
    user.require(Permission::LlcManage)?;
    let llc_id = parse_uuid(id)?;
    require_llc(state, scope.tenant_id, llc_id).await?;

    let f = &form.file;
    if f.len() == 0 {
        return Err(ApiError::BadRequest("empty file".into()));
    }
    let mime = f
        .content_type()
        .map(|c| c.to_string())
        .unwrap_or_else(|| "application/octet-stream".into());
    let ext = f
        .content_type()
        .and_then(|c| c.extension())
        .map(|e| e.as_str().to_ascii_lowercase())
        .or_else(|| {
            f.raw_name()
                .and_then(|n| n.as_str())
                .and_then(|s| s.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()))
        })
        .unwrap_or_else(|| "bin".into());
    let original = f
        .raw_name()
        .and_then(|n| n.as_str())
        .map(String::from)
        .unwrap_or_else(|| format!("upload.{ext}"));

    // Read the uploaded bytes.
    let mut reader = f
        .open()
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;
    let mut bytes = Vec::with_capacity(f.len() as usize);
    reader
        .read_to_end(&mut bytes)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;
    let size = bytes.len() as i64;
    let sha = sha256_hex(&bytes);

    // Store the bytes in the tenant's configured backend.
    let store = storage::resolve_for_tenant(state, scope.tenant_id).await?;
    let rel = format!(
        "tenants/{}/llc/{}/docs/{}.{}",
        scope.tenant_id,
        llc_id,
        Uuid::new_v4(),
        ext
    );
    let key = store.object_key(&rel);
    store.put(&key, bytes).await?;

    let now = Utc::now();
    let doc = entity::llc_document::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        llc_id: Set(llc_id),
        kind: Set(form.kind.clone()),
        title: Set(form.title.clone()),
        original_filename: Set(original),
        mime_type: Set(mime),
        size_bytes: Set(size),
        storage_provider: Set(store.provider_label.clone()),
        storage_key: Set(key),
        sha256: Set(sha),
        uploaded_by: Set(Some(user.user_id)),
        verified_at: Set(None),
        created_at: Set(now.into()),
    }
    .insert(&state.property_db)
    .await?;

    crate::audit::record(
        &state.user_db,
        Some(user.user_id),
        crate::audit::actions::LLC_DOCUMENT_UPLOAD,
        Some("llc_document"),
        Some(doc.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "llc_id": llc_id, "kind": doc.kind, "bytes": size })),
    )
    .await;

    Ok(Json(LlcDocumentDto::from(doc)))
}

/// `GET /llcs/<id>/documents/<doc_id>` — download the raw bytes of a document.
#[get("/llcs/<id>/documents/<doc_id>")]
pub async fn download_document(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    doc_id: &str,
) -> ApiResult<(ContentType, Vec<u8>)> {
    user.require(Permission::LlcRead)?;
    let llc_id = parse_uuid(id)?;
    let did = parse_uuid(doc_id)?;
    let doc = LlcDocument::find_by_id(did)
        .filter(entity::llc_document::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::llc_document::Column::LlcId.eq(llc_id))
        .one(&state.property_db)
        .await?
        .ok_or_else(|| ApiError::NotFound("document not found".into()))?;
    let store = storage::resolve_for_tenant(state, scope.tenant_id).await?;
    let bytes = store.get(&doc.storage_key).await?;
    let ct = ContentType::parse_flexible(&doc.mime_type).unwrap_or(ContentType::Binary);
    Ok((ct, bytes))
}

/// `DELETE /llcs/<id>/documents/<doc_id>` — remove a document (bytes + metadata).
#[rocket_okapi::openapi(tag = "LLCs")]
#[delete("/llcs/<id>/documents/<doc_id>")]
pub async fn delete_document(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    doc_id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::LlcManage)?;
    let llc_id = parse_uuid(id)?;
    let did = parse_uuid(doc_id)?;
    let doc = LlcDocument::find_by_id(did)
        .filter(entity::llc_document::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::llc_document::Column::LlcId.eq(llc_id))
        .one(&state.property_db)
        .await?
        .ok_or_else(|| ApiError::NotFound("document not found".into()))?;
    // Best-effort delete of the stored bytes, then the row.
    if let Ok(store) = storage::resolve_for_tenant(state, scope.tenant_id).await {
        let _ = store.delete(&doc.storage_key).await;
    }
    let doc_id_val = doc.id;
    doc.delete(&state.property_db).await?;
    crate::audit::record(
        &state.user_db,
        Some(user.user_id),
        crate::audit::actions::LLC_DOCUMENT_DELETE,
        Some("llc_document"),
        Some(doc_id_val.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "llc_id": llc_id })),
    )
    .await;
    Ok(Json(serde_json::json!({ "deleted": true })))
}
