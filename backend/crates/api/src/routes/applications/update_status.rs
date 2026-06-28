use super::dto::{ApplicationResp, UpdateApplicationReq};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::scheduler;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Application;
use rocket::serde::json::Json;
use rocket::{patch, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::json;
use uuid::Uuid;

/// `PATCH /applications/<id>` — advance an application's status.
///
/// Approving an application enqueues an automated welcome email via the scheduler.
#[rocket_okapi::openapi(tag = "Applications")]
#[patch("/applications/<id>", data = "<body>")]
pub async fn update_status(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateApplicationReq>,
) -> ApiResult<Json<ApplicationResp>> {
    user.require(Permission::ApplicationWrite)?;
    let aid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let a = Application::find_by_id(aid)
        .filter(entity::application::Column::TenantId.eq(scope.tenant_id))
        .one(&state.client_db)
        .await?
        .ok_or_else(|| ApiError::NotFound("application not found".into()))?;

    let new_status = body.into_inner().status;
    let previous_status = a.status.clone();
    let mut am: entity::application::ActiveModel = a.clone().into();
    am.status = Set(new_status.clone());
    let saved = am.update(&state.client_db).await?;

    crate::audit::record(
        &state.user_db,
        Some(user.user_id),
        crate::audit::actions::APPLICATION_UPDATE,
        Some("application"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "from": previous_status, "to": new_status })),
    )
    .await;

    if new_status == "Approved" {
        let _ = scheduler::enqueue(
            &state.user_db,
            scope.tenant_id,
            "auto_email",
            json!({ "template": "application_approved", "to": saved.email }),
            0,
        )
        .await;
    }

    Ok(Json(ApplicationResp::from(saved)))
}
