//! Application **workflow** endpoints — the pipeline catalog, a single
//! application's stage snapshot + history, and a stage-advance action. Mirrors
//! the property investment workflow, over the [`crate::app_workflow`] catalog.

use super::dto::ApplicationResp;
use crate::app_workflow;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{Application, ApplicationEvent};
use rocket::serde::json::Json;
use rocket::{get, post, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---- DTOs ------------------------------------------------------------------

#[derive(Serialize, schemars::JsonSchema)]
pub struct StageDto {
    pub key: String,
    pub label: String,
    pub terminal: bool,
    /// True for stages at or before the current one on the main path.
    pub reached: bool,
    /// True for the application's current stage.
    pub current: bool,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct WorkflowCatalogResp {
    pub stages: Vec<CatalogStage>,
    pub offramps: Vec<CatalogStage>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct CatalogStage {
    pub key: String,
    pub label: String,
    pub terminal: bool,
    /// Stages this one may transition to.
    pub transitions: Vec<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ApplicationEventDto {
    pub id: Uuid,
    pub from_status: Option<String>,
    pub to_status: String,
    pub note: Option<String>,
    pub actor_user_id: Option<Uuid>,
    pub created_at: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ApplicationWorkflowResp {
    pub current_status: String,
    pub stages: Vec<StageDto>,
    pub offramps: Vec<StageDto>,
    /// Statuses the application may move to next.
    pub allowed_next: Vec<String>,
    pub history: Vec<ApplicationEventDto>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct AdvanceReq {
    pub to_status: String,
    pub note: Option<String>,
}

fn catalog_stage(s: &app_workflow::StageDef) -> CatalogStage {
    CatalogStage {
        key: s.key.to_string(),
        label: s.label.to_string(),
        terminal: s.terminal,
        transitions: app_workflow::allowed_transitions(s.key)
            .iter()
            .map(|t| t.to_string())
            .collect(),
    }
}

// ---- Routes ----------------------------------------------------------------

/// `GET /applications/workflow/catalog` — the pipeline template + transitions.
#[rocket_okapi::openapi(tag = "Applications")]
#[get("/applications/workflow/catalog")]
pub async fn catalog(
    _state: &State<AppState>,
    user: AuthUser,
    _scope: TenantScope,
) -> ApiResult<Json<WorkflowCatalogResp>> {
    user.require(Permission::ApplicationRead)?;
    Ok(Json(WorkflowCatalogResp {
        stages: app_workflow::STAGES.iter().map(catalog_stage).collect(),
        offramps: app_workflow::OFFRAMPS.iter().map(catalog_stage).collect(),
    }))
}

/// `GET /applications/<id>/workflow` — one application's stage + history.
#[rocket_okapi::openapi(tag = "Applications")]
#[get("/applications/<id>/workflow")]
pub async fn get_workflow(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<ApplicationWorkflowResp>> {
    user.require(Permission::ApplicationRead)?;
    let aid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let app = Application::find_by_id(aid)
        .filter(entity::application::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("application not found".into()))?;

    let current = app.status.clone();
    // Index of the current stage on the main path (if it's on it).
    let current_idx = app_workflow::STAGES.iter().position(|s| s.key == current);
    let stage_dto = |s: &app_workflow::StageDef, idx: usize| StageDto {
        key: s.key.to_string(),
        label: s.label.to_string(),
        terminal: s.terminal,
        reached: current_idx.map(|ci| idx <= ci).unwrap_or(false),
        current: s.key == current,
    };
    let stages = app_workflow::STAGES
        .iter()
        .enumerate()
        .map(|(i, s)| stage_dto(s, i))
        .collect();
    let offramps = app_workflow::OFFRAMPS
        .iter()
        .map(|s| StageDto {
            key: s.key.to_string(),
            label: s.label.to_string(),
            terminal: s.terminal,
            reached: s.key == current,
            current: s.key == current,
        })
        .collect();

    let history = ApplicationEvent::find()
        .filter(entity::application_event::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::application_event::Column::ApplicationId.eq(aid))
        .order_by_asc(entity::application_event::Column::CreatedAt)
        .all(&db)
        .await?
        .into_iter()
        .map(|e| ApplicationEventDto {
            id: e.id,
            from_status: e.from_status,
            to_status: e.to_status,
            note: e.note,
            actor_user_id: e.actor_user_id,
            created_at: e.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(ApplicationWorkflowResp {
        allowed_next: app_workflow::allowed_transitions(&current)
            .iter()
            .map(|t| t.to_string())
            .collect(),
        current_status: current,
        stages,
        offramps,
        history,
    }))
}

/// `POST /applications/<id>/advance` — move the application to a new stage.
#[rocket_okapi::openapi(tag = "Applications")]
#[post("/applications/<id>/advance", data = "<body>")]
pub async fn advance(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<AdvanceReq>,
) -> ApiResult<Json<ApplicationResp>> {
    user.require(Permission::ApplicationWrite)?;
    let aid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let app = Application::find_by_id(aid)
        .filter(entity::application::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("application not found".into()))?;
    let b = body.into_inner();
    let saved = super::apply_transition(
        &db,
        scope.tenant_id,
        user.user_id,
        app,
        &b.to_status,
        b.note,
    )
    .await?;
    Ok(Json(ApplicationResp::from(saved)))
}
