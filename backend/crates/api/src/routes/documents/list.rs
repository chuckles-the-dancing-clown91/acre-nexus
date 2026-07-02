//! `GET /documents?owner_type&owner_id` — list documents, newest first.

use super::dto::DocumentDto;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Document;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

/// `GET /documents` — list this workspace's documents, optionally filtered to
/// one owning record. All versions are returned; the newest sorts first.
#[rocket_okapi::openapi(tag = "Documents")]
#[get("/documents?<owner_type>&<owner_id>&<limit>")]
pub async fn list(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    owner_type: Option<String>,
    owner_id: Option<String>,
    limit: Option<u64>,
) -> ApiResult<Json<Vec<DocumentDto>>> {
    user.require(Permission::DocumentRead)?;
    crate::modules::require_enabled(&db, scope.tenant_id, "integrations").await?;

    let mut q = Document::find().filter(entity::document::Column::TenantId.eq(scope.tenant_id));
    if let Some(t) = owner_type {
        q = q.filter(entity::document::Column::OwnerType.eq(t.trim().to_lowercase()));
    }
    if let Some(id) = owner_id {
        let id =
            Uuid::parse_str(&id).map_err(|_| ApiError::BadRequest("invalid owner_id".into()))?;
        q = q.filter(entity::document::Column::OwnerId.eq(id));
    }
    let rows = q
        .order_by_desc(entity::document::Column::CreatedAt)
        .limit(limit.unwrap_or(100).min(500))
        .all(&db)
        .await?;
    Ok(Json(rows.into_iter().map(DocumentDto::from).collect()))
}
