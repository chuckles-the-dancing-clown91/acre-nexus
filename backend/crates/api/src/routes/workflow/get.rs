//! `GET /properties/<id>/workflow` — the property's strategy, current stage,
//! the full stage template, and the transition history.

use super::dto::{build, WorkflowResp};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{Property, WorkflowEvent};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

/// `GET /properties/<id>/workflow` — current workflow state + history.
#[rocket_okapi::openapi(tag = "Workflow")]
#[get("/properties/<id>/workflow")]
pub async fn get_workflow(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<WorkflowResp>> {
    user.require(Permission::PropertyRead)?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let property = Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&state.property_db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;

    let history = WorkflowEvent::find()
        .filter(entity::workflow_event::Column::PropertyId.eq(pid))
        .order_by_desc(entity::workflow_event::Column::CreatedAt)
        .all(&state.property_db)
        .await?;

    Ok(Json(build(
        &property.strategy,
        &property.workflow_stage,
        history,
    )))
}
