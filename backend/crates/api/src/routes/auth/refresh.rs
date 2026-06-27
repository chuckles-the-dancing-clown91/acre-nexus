use super::dto::{RefreshReq, TokenResp};
use super::helpers::{build_user_resp, issue_refresh_token, permissions_for};
use crate::auth::{hash_secret, issue_access_token};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use chrono::Utc;
use entity::prelude::{RefreshToken, User};
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

/// `POST /auth/refresh` — rotate a refresh token for a fresh access/refresh pair.
#[rocket_okapi::openapi(tag = "Auth")]
#[post("/auth/refresh", data = "<body>")]
pub async fn refresh(
    state: &State<AppState>,
    body: Json<RefreshReq>,
) -> ApiResult<Json<TokenResp>> {
    let hash = hash_secret(&body.refresh_token);
    let token = RefreshToken::find()
        .filter(entity::refresh_token::Column::TokenHash.eq(hash))
        .one(&state.db)
        .await?
        .ok_or(ApiError::Unauthorized)?;

    let now = Utc::now();
    if token.revoked_at.is_some() || token.expires_at < now {
        return Err(ApiError::Unauthorized);
    }

    // Rotate: revoke the old refresh token.
    let mut am: entity::refresh_token::ActiveModel = token.clone().into();
    am.revoked_at = Set(Some(now.into()));
    am.update(&state.db).await?;

    let user = User::find_by_id(token.user_id)
        .one(&state.db)
        .await?
        .ok_or(ApiError::Unauthorized)?;
    let active = user.tenant_id;
    let perms = permissions_for(&state.db, user.id, active).await?;
    let access = issue_access_token(
        &state.config,
        user.id,
        active,
        user.is_platform_staff,
        perms.clone(),
    )
    .map_err(ApiError::Internal)?;
    let new_refresh = issue_refresh_token(state, user.id).await?;
    let user_resp = build_user_resp(&state.db, &user, active, perms).await?;

    crate::audit::record(
        &state.db,
        Some(user.id),
        crate::audit::actions::AUTH_REFRESH,
        Some("user"),
        Some(user.id.to_string()),
        active,
        None,
    )
    .await;

    Ok(Json(TokenResp {
        access_token: access,
        refresh_token: new_refresh,
        token_type: "Bearer",
        expires_in: state.config.access_ttl_secs,
        user: user_resp,
    }))
}
