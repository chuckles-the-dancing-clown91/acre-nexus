//! `GET /properties/<id>/documents` — the Documents tab for a property: the
//! latest version of every filed document, per-category tallies, and the
//! wet-ink originals with their physical storage locations.

use super::dto::{CategoryCount, DocumentDto, PropertyDocumentsResp};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{Document, Property};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use std::collections::BTreeMap;
use uuid::Uuid;

/// `GET /properties/<id>/documents` — property document dossier. Only the newest
/// version of each `(filename)` is returned; the wet-ink subset is broken out so
/// the UI can show where each paper original lives.
#[rocket_okapi::openapi(tag = "Documents")]
#[get("/properties/<id>/documents")]
pub async fn property_documents(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<PropertyDocumentsResp>> {
    user.require(Permission::DocumentRead)?;
    crate::modules::require_enabled(&db, scope.tenant_id, "integrations").await?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;

    // All versions, newest first; we then keep the newest per filename.
    let rows = Document::find()
        .filter(entity::document::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::document::Column::OwnerType.eq("property"))
        .filter(entity::document::Column::OwnerId.eq(pid))
        .order_by_desc(entity::document::Column::Version)
        .order_by_desc(entity::document::Column::CreatedAt)
        .all(&db)
        .await?;

    let mut seen: BTreeMap<String, ()> = BTreeMap::new();
    let mut documents: Vec<DocumentDto> = Vec::new();
    for row in rows {
        // First time we meet a filename it is (by the ordering) the newest version.
        if seen.insert(row.filename.clone(), ()).is_none() {
            documents.push(DocumentDto::from(row));
        }
    }
    // Present newest-created first for the tab.
    documents.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    // Per-category tally (stable order: unfiled last, otherwise alphabetical).
    let mut tally: BTreeMap<Option<String>, i64> = BTreeMap::new();
    for d in &documents {
        *tally.entry(d.category.clone()).or_insert(0) += 1;
    }
    let mut categories: Vec<CategoryCount> = tally
        .into_iter()
        .map(|(category, count)| CategoryCount { category, count })
        .collect();
    categories.sort_by(|a, b| match (&a.category, &b.category) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(_), None) => std::cmp::Ordering::Less,
        (Some(x), Some(y)) => x.cmp(y),
    });

    let wet_ink_originals: Vec<DocumentDto> = documents
        .iter()
        .filter(|d| d.requires_wet_ink)
        .cloned()
        .collect();

    Ok(Json(PropertyDocumentsResp {
        property_id: pid,
        total: documents.len() as i64,
        documents,
        categories,
        wet_ink_originals,
    }))
}
