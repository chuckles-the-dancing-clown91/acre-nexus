//! `PATCH /documents/<id>` — update a document's filing metadata: its category,
//! whether it needs a wet-ink signature, and where the wet-ink original is
//! physically stored. The blob and versioning are untouched.

use super::dto::{DocumentDto, UpdateDocumentReq};
use super::normalize_category;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::Document;
use rocket::serde::json::Json;
use rocket::{patch, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `PATCH /documents/<id>` — set the filing category, wet-ink flag, and/or the
/// physical storage location of the original. Only provided fields change.
#[rocket_okapi::openapi(tag = "Documents")]
#[patch("/documents/<id>", data = "<body>")]
pub async fn update(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateDocumentReq>,
) -> ApiResult<Json<DocumentDto>> {
    user.require(Permission::DocumentManage)?;
    crate::modules::require_enabled(&db, scope.tenant_id, "integrations").await?;
    let id = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;

    let doc = Document::find_by_id(id)
        .filter(entity::document::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("document not found".into()))?;

    let b = body.into_inner();
    let mut am: entity::document::ActiveModel = doc.into();
    if let Some(cat) = b.category {
        // An empty string clears the category; anything else must be in the catalog.
        let normalized = normalize_category(Some(cat)).map_err(ApiError::BadRequest)?;
        am.category = Set(normalized);
    }
    if let Some(wet) = b.requires_wet_ink {
        am.requires_wet_ink = Set(wet);
    }
    if let Some(loc) = b.physical_location {
        let loc = loc.trim().to_string();
        am.physical_location = Set(if loc.is_empty() { None } else { Some(loc) });
    }
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::DOCUMENT_UPDATE,
        Some("document"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({
            "category": saved.category,
            "requires_wet_ink": saved.requires_wet_ink,
            "has_physical_location": saved.physical_location.is_some(),
        })),
    )
    .await;

    Ok(Json(DocumentDto::from(saved)))
}
