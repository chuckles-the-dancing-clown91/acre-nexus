//! `/entities/<id>/assignments` — assign staff to a legal entity (LLC). Creating
//! an assignment grants the person `entity:{id}`-scoped access, which (via
//! [`crate::rbac::scope`]) covers every property that LLC holds title to.

use super::{
    create_assignment_inner, list_for_subject, remove_assignment_inner, AssignmentDto,
    CreateAssignmentReq, SUBJECT_ENTITY,
};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Llc;
use rocket::serde::json::Json;
use rocket::{delete, get, post, State};
use sea_orm::EntityTrait;
use uuid::Uuid;

/// Validate the LLC belongs to the active tenant, returning its id.
async fn require_entity(db: &crate::db::RequestDb, tenant_id: Uuid, id: &str) -> ApiResult<Uuid> {
    let eid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid entity id".into()))?;
    Llc::find_by_id(eid)
        .one(db)
        .await?
        .filter(|l| l.tenant_id == tenant_id)
        .ok_or_else(|| ApiError::NotFound("legal entity not found".into()))?;
    Ok(eid)
}

/// `GET /entities/<id>/assignments` — the entity's assigned team.
#[rocket_okapi::openapi(tag = "Assignments")]
#[get("/entities/<id>/assignments")]
pub async fn list(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Vec<AssignmentDto>>> {
    user.require(Permission::EntityRead)?;
    let eid = require_entity(&db, scope.tenant_id, id).await?;
    Ok(Json(
        list_for_subject(&db, scope.tenant_id, SUBJECT_ENTITY, eid).await?,
    ))
}

/// `POST /entities/<id>/assignments` — assign a person to the legal entity.
#[rocket_okapi::openapi(tag = "Assignments")]
#[post("/entities/<id>/assignments", data = "<body>")]
pub async fn create(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<CreateAssignmentReq>,
) -> ApiResult<Json<AssignmentDto>> {
    user.require(Permission::EntityManage)?;
    let eid = require_entity(&db, scope.tenant_id, id).await?;
    let dto = create_assignment_inner(
        &db,
        scope.tenant_id,
        user.user_id,
        SUBJECT_ENTITY,
        eid,
        &body.into_inner(),
    )
    .await?;
    Ok(Json(dto))
}

/// `DELETE /entities/<id>/assignments/<assignment_id>` — unassign + revoke.
#[rocket_okapi::openapi(tag = "Assignments")]
#[delete("/entities/<id>/assignments/<assignment_id>")]
pub async fn delete(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    assignment_id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::EntityManage)?;
    let eid = require_entity(&db, scope.tenant_id, id).await?;
    let aid = Uuid::parse_str(assignment_id)
        .map_err(|_| ApiError::BadRequest("invalid assignment id".into()))?;
    remove_assignment_inner(&db, scope.tenant_id, user.user_id, SUBJECT_ENTITY, eid, aid).await?;
    Ok(Json(serde_json::json!({ "removed": true })))
}
