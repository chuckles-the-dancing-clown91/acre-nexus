use super::dto::LogoutReq;
use crate::auth::{self, AuthUser};
use crate::error::ApiResult;
use crate::state::AppState;
use chrono::Utc;
use entity::prelude::RefreshToken;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

/// `POST /auth/logout` — revoke a refresh token. Access tokens expire naturally.
#[rocket_okapi::openapi(tag = "Auth")]
#[post("/auth/logout", data = "<body>")]
pub async fn logout(
    state: &State<AppState>,
    user: AuthUser,
    body: Json<LogoutReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let hash = auth::hash_secret(&body.refresh_token);
    if let Some(tok) = RefreshToken::find()
        .filter(entity::refresh_token::Column::TokenHash.eq(hash))
        .one(&state.user_db)
        .await?
    {
        let mut am: entity::refresh_token::ActiveModel = tok.into();
        am.revoked_at = Set(Some(Utc::now().into()));
        am.update(&state.user_db).await?;
    }
    crate::audit::record(
        &state.user_db,
        Some(user.user_id),
        crate::audit::actions::AUTH_LOGOUT,
        Some("user"),
        Some(user.user_id.to_string()),
        user.tenant_id,
        None,
    )
    .await;
    Ok(Json(serde_json::json!({ "ok": true })))
}
