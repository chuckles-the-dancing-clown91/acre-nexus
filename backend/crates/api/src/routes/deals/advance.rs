use super::dto::{AdvanceStageReq, DealDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, IntoActiveModel, Set};
use uuid::Uuid;

/// `POST /modules/flips/deals/<id>/stage` — move a deal to a new pipeline stage
/// (validated against the acquisition catalog) with an optional note, recording
/// the transition on the timeline.
#[rocket_okapi::openapi(tag = "Flips")]
#[post("/modules/flips/deals/<id>/stage", data = "<body>")]
pub async fn advance(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<AdvanceStageReq>,
) -> ApiResult<Json<DealDto>> {
    user.require(Permission::DealWrite)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "flips").await?;

    let b = body.into_inner();
    let to_stage = b.stage.trim().to_string();
    if !crate::deals::is_valid_stage(&to_stage) {
        return Err(ApiError::BadRequest(format!(
            "unknown deal stage: {to_stage}"
        )));
    }

    let deal = super::load_deal(&db, scope.tenant_id, id).await?;
    let from_stage = deal.stage.clone();
    if from_stage == to_stage {
        // No-op transition; return the deal unchanged.
        return Ok(Json(DealDto::build(&deal)));
    }

    let now = Utc::now();
    let mut m = deal.into_active_model();
    m.stage = Set(to_stage.clone());
    m.updated_at = Set(now.into());
    let saved = m.update(&db).await?;

    entity::deal_event::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        deal_id: Set(saved.id),
        kind: Set("stage_change".into()),
        from_stage: Set(Some(from_stage.clone())),
        to_stage: Set(Some(to_stage.clone())),
        body: Set(b.note.filter(|s| !s.trim().is_empty())),
        actor_user_id: Set(Some(user.user_id)),
        created_at: Set(now.into()),
    }
    .insert(&db)
    .await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::DEAL_STAGE_ADVANCE,
        Some("deal"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "from": from_stage, "to": to_stage })),
    )
    .await;

    Ok(Json(DealDto::build(&saved)))
}
