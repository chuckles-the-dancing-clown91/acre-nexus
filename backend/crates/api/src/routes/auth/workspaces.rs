use super::dto::WorkspaceSummary;
use super::helpers::{load_memberships, workspaces_from};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use entity::prelude::User;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::EntityTrait;

/// `GET /auth/workspaces` — the workspaces the current user can switch into.
#[rocket_okapi::openapi(tag = "Auth")]
#[get("/auth/workspaces")]
pub async fn workspaces(
    state: &State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Vec<WorkspaceSummary>>> {
    let u = User::find_by_id(user.user_id)
        .one(&state.user_db)
        .await?
        .ok_or(ApiError::Unauthorized)?;
    let memberships = load_memberships(&state.user_db, u.id).await?;
    Ok(Json(workspaces_from(&memberships, u.is_platform_staff)))
}
