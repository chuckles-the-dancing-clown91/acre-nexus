//! `GET /properties/<id>/workflow` — the property's strategy, current stage,
//! the full stage template, and the transition history.

use super::dto::{actor_names, build, WorkflowResp};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{Property, WorkflowEvent};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

/// `GET /properties/<id>/workflow` — current workflow state + history.
///
/// Deliberately requires **no specific permission** beyond authentication:
/// every member of the workspace can see where a property stands in its
/// process and the steps it has been through (advancing still requires
/// `property:write`).
#[rocket_okapi::openapi(tag = "Workflow")]
#[get("/properties/<id>/workflow")]
pub async fn get_workflow(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    _user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<WorkflowResp>> {
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let property = Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;

    let history = WorkflowEvent::find()
        .filter(entity::workflow_event::Column::PropertyId.eq(pid))
        .order_by_desc(entity::workflow_event::Column::CreatedAt)
        .all(&db)
        .await?;

    let actors = actor_names(&db, &history).await;
    Ok(Json(build(
        &property.strategy,
        &property.workflow_stage,
        history,
        &actors,
    )))
}
