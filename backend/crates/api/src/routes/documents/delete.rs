//! `DELETE /documents/<id>` — delete a document (blob + metadata).

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::storage::ObjectStore;
use crate::tenancy::TenantScope;
use entity::prelude::Document;
use rocket::serde::json::Json;
use rocket::{delete, State};
use sea_orm::{ColumnTrait, EntityTrait, ModelTrait, QueryFilter};
use uuid::Uuid;

/// `DELETE /documents/<id>` — remove one document version: the blob leaves the
/// object store and the metadata row is deleted. Earlier versions (linked via
/// `previous_version_id`) are untouched.
#[rocket_okapi::openapi(tag = "Documents")]
#[delete("/documents/<id>")]
pub async fn delete(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::DocumentManage)?;
    crate::modules::require_enabled(&db, scope.tenant_id, "integrations").await?;
    let id = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;

    let doc = Document::find_by_id(id)
        .filter(entity::document::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("document not found".into()))?;

    let store = ObjectStore::from_env()?;
    store.delete(&doc.storage_key).await?;

    let filename = doc.filename.clone();
    let owner_type = doc.owner_type.clone();
    let owner_id = doc.owner_id;
    doc.delete(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::DOCUMENT_DELETE,
        Some("document"),
        Some(id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({
            "filename": filename,
            "owner_type": owner_type,
            "owner_id": owner_id,
        })),
    )
    .await;

    Ok(Json(serde_json::json!({ "deleted": true, "id": id })))
}
