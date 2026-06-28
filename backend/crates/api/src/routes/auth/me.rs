use super::dto::UserResp;
use super::helpers::{build_user_resp, permissions_for};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use entity::prelude::User;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::EntityTrait;

/// `GET /auth/me` — the currently authenticated principal.
#[rocket_okapi::openapi(tag = "Auth")]
#[get("/auth/me")]
pub async fn me(state: &State<AppState>, user: AuthUser) -> ApiResult<Json<UserResp>> {
    let u = User::find_by_id(user.user_id)
        .one(&state.user_db)
        .await?
        .ok_or(ApiError::Unauthorized)?;
    // Reflect the workspace the current token is scoped to (from the JWT).
    let active = user.tenant_id;
    let perms = permissions_for(&state.user_db, u.id, active).await?;
    let resp = build_user_resp(&state.user_db, &u, active, perms).await?;
    Ok(Json(resp))
}
