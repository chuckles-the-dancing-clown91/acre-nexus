//! **Property media** (roadmap Phase 7, issue #11) — photos and floorplans for a
//! property, rendered as a gallery on the profile.
//!
//! Media rides the existing polymorphic [`document`](entity::document) service
//! (`owner_type = "property"`, category `photo` / `floorplan`); this surface just
//! filters to the image documents and hands back **fresh signed GET URLs** so the
//! console can render them inline. The **hero** photo is stored as a stable
//! `doc:{id}` sentinel in `property.image_url`, which [`resolve_hero_url`] turns
//! into a fresh signed URL wherever the hero is shown — so the hero never points
//! at a URL that has since expired.

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::routes::documents::MEDIA_CATEGORIES;
use crate::state::AppState;
use crate::storage::{ObjectStore, SIGNED_URL_TTL_SECS};
use crate::tenancy::TenantScope;
use entity::prelude::{Document, Property};
use rocket::serde::json::Json;
use rocket::{get, patch, State};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, IntoActiveModel, QueryFilter,
    QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The `doc:{uuid}` sentinel prefix used to store a hero that references a
/// document rather than a bare URL.
const HERO_PREFIX: &str = "doc:";

#[derive(Serialize, schemars::JsonSchema)]
pub struct MediaItemDto {
    pub document_id: Uuid,
    pub filename: String,
    pub category: Option<String>,
    pub mime_type: String,
    pub size_bytes: i64,
    /// Short-lived signed GET URL for the bytes (renderable in an `<img>`).
    pub url: Option<String>,
    pub is_hero: bool,
    pub created_at: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct PropertyMediaResp {
    pub hero_document_id: Option<Uuid>,
    /// Freshly-signed hero URL (mirrors the profile header), if a hero is set.
    pub hero_url: Option<String>,
    pub items: Vec<MediaItemDto>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SetHeroReq {
    /// The media document to promote to hero, or `null` to clear the hero.
    pub document_id: Option<Uuid>,
}

/// Parse the hero document id out of a `doc:{uuid}` sentinel (if that's what the
/// property's `image_url` holds).
fn hero_doc_id(image_url: Option<&str>) -> Option<Uuid> {
    image_url
        .and_then(|s| s.strip_prefix(HERO_PREFIX))
        .and_then(|s| Uuid::parse_str(s).ok())
}

/// Resolve a property's `image_url` to a renderable URL: a `doc:{id}` sentinel
/// becomes a fresh signed GET URL for the stored document; a plain URL is
/// returned as-is; anything unresolvable becomes `None`.
pub async fn resolve_hero_url<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    image_url: Option<&str>,
) -> Option<String> {
    let raw = image_url?;
    match hero_doc_id(Some(raw)) {
        Some(did) => {
            let doc = Document::find_by_id(did)
                .filter(entity::document::Column::TenantId.eq(tenant_id))
                .one(db)
                .await
                .ok()??;
            if doc.status != "stored" {
                return None;
            }
            let store = ObjectStore::from_env().ok()?;
            store
                .signed_get_url(&doc.storage_key, SIGNED_URL_TTL_SECS)
                .ok()
                .map(|s| s.url)
        }
        None => Some(raw.to_string()),
    }
}

/// Build the full media view for a property (gallery items + hero).
async fn build_media<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    property: &entity::property::Model,
) -> ApiResult<PropertyMediaResp> {
    let hero_id = hero_doc_id(property.image_url.as_deref());
    let rows = Document::find()
        .filter(entity::document::Column::TenantId.eq(tenant_id))
        .filter(entity::document::Column::OwnerType.eq("property"))
        .filter(entity::document::Column::OwnerId.eq(property.id))
        .filter(entity::document::Column::Category.is_in(MEDIA_CATEGORIES.to_vec()))
        .filter(entity::document::Column::Status.eq("stored"))
        .order_by_desc(entity::document::Column::CreatedAt)
        .all(db)
        .await?;

    let store = ObjectStore::from_env().ok();
    let items = rows
        .into_iter()
        .map(|d| {
            let url = store
                .as_ref()
                .and_then(|s| s.signed_get_url(&d.storage_key, SIGNED_URL_TTL_SECS).ok())
                .map(|s| s.url);
            MediaItemDto {
                is_hero: Some(d.id) == hero_id,
                document_id: d.id,
                filename: d.filename,
                category: d.category,
                mime_type: d.mime_type,
                size_bytes: d.size_bytes,
                url,
                created_at: d.created_at.to_rfc3339(),
            }
        })
        .collect();

    let hero_url = resolve_hero_url(db, tenant_id, property.image_url.as_deref()).await;
    Ok(PropertyMediaResp {
        hero_document_id: hero_id,
        hero_url,
        items,
    })
}

async fn load_property<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    id: &str,
) -> ApiResult<entity::property::Model> {
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))
}

/// `GET /properties/<id>/media` — the property's photos + floorplans, each with a
/// fresh signed URL, newest first.
#[rocket_okapi::openapi(tag = "Properties")]
#[get("/properties/<id>/media")]
pub async fn list_media(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<PropertyMediaResp>> {
    user.require(Permission::PropertyRead)?;
    let property = load_property(&db, scope.tenant_id, id).await?;
    Ok(Json(build_media(&db, scope.tenant_id, &property).await?))
}

/// `PATCH /properties/<id>/hero` — promote a media document to the hero photo
/// (stored as a `doc:{id}` sentinel), or clear it with `{ "document_id": null }`.
#[rocket_okapi::openapi(tag = "Properties")]
#[patch("/properties/<id>/hero", data = "<body>")]
pub async fn set_hero(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<SetHeroReq>,
) -> ApiResult<Json<PropertyMediaResp>> {
    user.require(Permission::PropertyWrite)?;
    let property = load_property(&db, scope.tenant_id, id).await?;
    let pid = property.id;

    let new_image_url = match body.into_inner().document_id {
        Some(doc_id) => {
            // The document must be a media file attached to this property.
            let doc = Document::find_by_id(doc_id)
                .filter(entity::document::Column::TenantId.eq(scope.tenant_id))
                .one(&db)
                .await?
                .ok_or_else(|| ApiError::NotFound("document not found".into()))?;
            let is_media = doc
                .category
                .as_deref()
                .is_some_and(|c| MEDIA_CATEGORIES.contains(&c));
            if doc.owner_type != "property" || doc.owner_id != pid || !is_media {
                return Err(ApiError::BadRequest(
                    "document is not a media file for this property".into(),
                ));
            }
            Some(format!("{HERO_PREFIX}{doc_id}"))
        }
        None => None,
    };

    let mut am = property.into_active_model();
    am.image_url = Set(new_image_url.clone());
    let saved = am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::PROPERTY_UPDATE,
        Some("property"),
        Some(pid.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "hero": new_image_url })),
    )
    .await;

    Ok(Json(build_media(&db, scope.tenant_id, &saved).await?))
}
