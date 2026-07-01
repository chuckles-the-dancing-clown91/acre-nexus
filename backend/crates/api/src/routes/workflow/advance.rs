//! `POST /properties/<id>/workflow/advance` — move a property to a new stage in
//! its strategy's workflow, recording the transition.

use super::dto::{build, AdvanceReq, WorkflowResp};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{Property, WorkflowEvent};
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use uuid::Uuid;

/// `POST /properties/<id>/workflow/advance` — transition to `to_stage`.
#[rocket_okapi::openapi(tag = "Workflow")]
#[post("/properties/<id>/workflow/advance", data = "<body>")]
pub async fn advance(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<AdvanceReq>,
) -> ApiResult<Json<WorkflowResp>> {
    user.require(Permission::PropertyWrite)?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let property = Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;

    let req = body.into_inner();
    if !crate::workflow::is_valid_stage(&property.strategy, &req.to_stage) {
        return Err(ApiError::BadRequest(format!(
            "'{}' is not a valid stage for strategy '{}'",
            req.to_stage, property.strategy
        )));
    }

    let from_stage = (!property.workflow_stage.is_empty()).then(|| property.workflow_stage.clone());
    let strategy = property.strategy.clone();

    // Update the property's current stage.
    let mut am: entity::property::ActiveModel = property.into();
    am.workflow_stage = Set(req.to_stage.clone());
    am.update(&db).await?;

    // Record the transition.
    entity::workflow_event::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        property_id: Set(pid),
        strategy: Set(strategy.clone()),
        from_stage: Set(from_stage.clone()),
        to_stage: Set(req.to_stage.clone()),
        note: Set(req.note.clone()),
        actor_user_id: Set(Some(user.user_id)),
        created_at: Set(Utc::now().into()),
    }
    .insert(&db)
    .await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::WORKFLOW_ADVANCE,
        Some("property"),
        Some(pid.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "from": from_stage, "to": req.to_stage })),
    )
    .await;

    let history = WorkflowEvent::find()
        .filter(entity::workflow_event::Column::PropertyId.eq(pid))
        .order_by_desc(entity::workflow_event::Column::CreatedAt)
        .all(&db)
        .await?;

    Ok(Json(build(&strategy, &req.to_stage, history)))
}
