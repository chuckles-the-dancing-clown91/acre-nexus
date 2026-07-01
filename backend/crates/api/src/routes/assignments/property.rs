//! `/properties/<id>/assignments` — assign staff (property managers, landlords,
//! maintenance, leasing agents, back-office) to a property. Creating an
//! assignment also grants the person `property:{id}`-scoped access.

use super::{
    create_assignment_inner, list_for_subject, remove_assignment_inner, AssignmentDto,
    CreateAssignmentReq, SUBJECT_PROPERTY,
};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Property;
use rocket::serde::json::Json;
use rocket::{delete, get, post, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

/// Validate the property belongs to the active tenant, returning its id.
async fn require_property(db: &crate::db::RequestDb, tenant_id: Uuid, id: &str) -> ApiResult<Uuid> {
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    Ok(pid)
}

/// `GET /properties/<id>/assignments` — the property's assigned team.
#[rocket_okapi::openapi(tag = "Assignments")]
#[get("/properties/<id>/assignments")]
pub async fn list(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Vec<AssignmentDto>>> {
    user.require(Permission::PropertyRead)?;
    let pid = require_property(&db, scope.tenant_id, id).await?;
    Ok(Json(
        list_for_subject(&db, scope.tenant_id, SUBJECT_PROPERTY, pid).await?,
    ))
}

/// `POST /properties/<id>/assignments` — assign a person to the property.
#[rocket_okapi::openapi(tag = "Assignments")]
#[post("/properties/<id>/assignments", data = "<body>")]
pub async fn create(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<CreateAssignmentReq>,
) -> ApiResult<Json<AssignmentDto>> {
    user.require(Permission::PropertyWrite)?;
    let pid = require_property(&db, scope.tenant_id, id).await?;
    let dto = create_assignment_inner(
        &db,
        scope.tenant_id,
        user.user_id,
        SUBJECT_PROPERTY,
        pid,
        &body.into_inner(),
    )
    .await?;
    Ok(Json(dto))
}

/// `DELETE /properties/<id>/assignments/<assignment_id>` — unassign + revoke.
#[rocket_okapi::openapi(tag = "Assignments")]
#[delete("/properties/<id>/assignments/<assignment_id>")]
pub async fn delete(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    assignment_id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::PropertyWrite)?;
    let pid = require_property(&db, scope.tenant_id, id).await?;
    let aid = Uuid::parse_str(assignment_id)
        .map_err(|_| ApiError::BadRequest("invalid assignment id".into()))?;
    remove_assignment_inner(
        &db,
        scope.tenant_id,
        user.user_id,
        SUBJECT_PROPERTY,
        pid,
        aid,
    )
    .await?;
    Ok(Json(serde_json::json!({ "removed": true })))
}
