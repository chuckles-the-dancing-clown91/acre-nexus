//! `GET /documents/<id>/download` — issue a signed, expiring download URL.

use super::dto::DownloadDocumentResp;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::storage::{ObjectStore, SIGNED_URL_TTL_SECS};
use crate::tenancy::TenantScope;
use entity::prelude::Document;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

/// `GET /documents/<id>/download` — permission-checked issuance of a
/// short-lived signed URL for the blob. The API never proxies the bytes; the
/// client follows the URL straight to the store.
#[rocket_okapi::openapi(tag = "Documents")]
#[get("/documents/<id>/download")]
pub async fn download(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<DownloadDocumentResp>> {
    user.require(Permission::DocumentRead)?;
    crate::modules::require_enabled(&db, scope.tenant_id, "integrations").await?;
    let id = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;

    let doc = Document::find_by_id(id)
        .filter(entity::document::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("document not found".into()))?;

    let store = ObjectStore::from_env()?;
    let signed = store.signed_get_url(&doc.storage_key, SIGNED_URL_TTL_SECS)?;

    // Audit the fact of access, not the content (same discipline as
    // `pii.reveal`).
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::DOCUMENT_DOWNLOAD,
        Some("document"),
        Some(doc.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({
            "filename": doc.filename,
            "owner_type": doc.owner_type,
            "owner_id": doc.owner_id,
        })),
    )
    .await;

    Ok(Json(DownloadDocumentResp {
        url: signed.url,
        expires_at: signed.expires_at.to_rfc3339(),
    }))
}
