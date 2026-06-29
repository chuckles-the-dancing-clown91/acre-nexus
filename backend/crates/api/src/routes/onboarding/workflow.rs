//! `GET /onboarding/workflow` + `POST /onboarding/workflow/advance` — the
//! resumable per-tenant onboarding workflow (§9). Both recompute every step's
//! predicate from the live database and persist the derived state; `advance`
//! additionally writes an audit event (used by the "I've finished this step"
//! action that nudges the workflow forward).

use super::state::{compute, WorkflowSnapshot};
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::OnboardingWorkflow;
use rocket::serde::json::Json;
use rocket::{get, post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// Recompute the snapshot and persist it onto the tenant's workflow row (upsert).
async fn persist(state: &State<AppState>, tenant_id: Uuid) -> ApiResult<WorkflowSnapshot> {
    let snap = compute(state, tenant_id).await?;
    let now = Utc::now();
    let steps_json = serde_json::to_value(&snap.steps).unwrap_or(serde_json::json!([]));

    match OnboardingWorkflow::find()
        .filter(entity::onboarding_workflow::Column::TenantId.eq(tenant_id))
        .one(&state.db)
        .await?
    {
        Some(row) => {
            let mut am: entity::onboarding_workflow::ActiveModel = row.into();
            am.state = Set(snap.state.clone());
            am.steps = Set(steps_json);
            am.updated_at = Set(now.into());
            am.update(&state.db).await?;
        }
        None => {
            entity::onboarding_workflow::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                state: Set(snap.state.clone()),
                steps: Set(steps_json),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }
            .insert(&state.db)
            .await?;
        }
    }
    Ok(snap)
}

/// `GET /onboarding/workflow` — the current onboarding checklist + state.
#[rocket_okapi::openapi(tag = "Onboarding")]
#[get("/onboarding/workflow")]
pub async fn get_onboarding_workflow(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<WorkflowSnapshot>> {
    user.require(Permission::TenantManage)?;
    Ok(Json(persist(state, scope.tenant_id).await?))
}

/// `POST /onboarding/workflow/advance` — recompute + persist, audited.
#[rocket_okapi::openapi(tag = "Onboarding")]
#[post("/onboarding/workflow/advance")]
pub async fn advance_onboarding(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<WorkflowSnapshot>> {
    user.require(Permission::TenantManage)?;
    let snap = persist(state, scope.tenant_id).await?;
    crate::audit::record(
        &state.db,
        Some(user.user_id),
        crate::audit::actions::ONBOARDING_ADVANCE,
        Some("onboarding_workflow"),
        Some(scope.tenant_id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "state": snap.state, "live": snap.live })),
    )
    .await;
    Ok(Json(snap))
}
