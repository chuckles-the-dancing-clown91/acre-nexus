use super::dto::{ApplicationResp, UpdateApplicationReq};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Application;
use rocket::serde::json::Json;
use rocket::{patch, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

/// `PATCH /applications/<id>` — advance an application's status.
///
/// The transition is validated against the [`crate::app_workflow`] state machine,
/// recorded in `application_event`, and (when → `Approved`) enqueues the
/// automated welcome email. See also `POST /applications/<id>/advance`.
#[rocket_okapi::openapi(tag = "Applications")]
#[patch("/applications/<id>", data = "<body>")]
pub async fn update_status(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateApplicationReq>,
) -> ApiResult<Json<ApplicationResp>> {
    user.require(Permission::ApplicationWrite)?;
    let aid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let a = Application::find_by_id(aid)
        .filter(entity::application::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("application not found".into()))?;

    let saved = super::apply_transition(
        &db,
        scope.tenant_id,
        Some(user.user_id),
        a,
        &body.into_inner().status,
        None,
    )
    .await?;

    Ok(Json(ApplicationResp::from(saved)))
}
