//! `GET /leases/<id>/document` — the latest generated lease document, if any.

use super::dto::LeaseDocDto;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::LeaseDocument;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

/// `GET /leases/<id>/document` — fetch the most recent lease document.
#[rocket_okapi::openapi(tag = "Lease Documents")]
#[get("/leases/<id>/document")]
pub async fn get(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<LeaseDocDto>> {
    user.require(Permission::LeaseRead)?;
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let doc = LeaseDocument::find()
        .filter(entity::lease_document::Column::LeaseId.eq(lid))
        .filter(entity::lease_document::Column::TenantId.eq(scope.tenant_id))
        .order_by_desc(entity::lease_document::Column::GeneratedAt)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("no document generated for this lease".into()))?;
    Ok(Json(LeaseDocDto::from(doc)))
}
