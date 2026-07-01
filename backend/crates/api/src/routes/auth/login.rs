use super::dto::{LoginReq, TokenResp};
use super::helpers::{build_user_resp, issue_refresh_token, permissions_for};
use crate::auth::{issue_access_token, verify_password};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use chrono::Utc;
use entity::prelude::User;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

/// `POST /auth/login` — exchange email + password for an access/refresh token pair.
#[rocket_okapi::openapi(tag = "Auth")]
#[post("/auth/login", data = "<body>")]
pub async fn login(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    body: Json<LoginReq>,
) -> ApiResult<Json<TokenResp>> {
    let user = User::find()
        .filter(entity::user::Column::Email.eq(body.email.to_lowercase()))
        .one(&db)
        .await?
        .ok_or(ApiError::Unauthorized)?;

    if !verify_password(&body.password, &user.password_hash) {
        return Err(ApiError::Unauthorized);
    }

    // Only active accounts may sign in (invited / suspended / disabled cannot).
    if user.status != "active" {
        return Err(ApiError::Forbidden(format!(
            "account is {} — contact an administrator",
            user.status
        )));
    }

    // Record the sign-in timestamp (best-effort).
    {
        let mut am: entity::user::ActiveModel = user.clone().into();
        am.last_login_at = Set(Some(Utc::now().into()));
        if let Err(e) = am.update(&db).await {
            tracing::warn!("failed to update last_login_at: {e}");
        }
    }

    let active = user.tenant_id;
    let perms = permissions_for(&db, user.id, active).await?;
    let access = issue_access_token(
        &state.config,
        user.id,
        active,
        user.is_platform_staff,
        perms.clone(),
    )
    .map_err(ApiError::Internal)?;

    let refresh = issue_refresh_token(&db, state.config.refresh_ttl_secs, user.id).await?;
    let user_resp = build_user_resp(&db, &user, active, perms).await?;

    crate::audit::record(
        &db,
        Some(user.id),
        crate::audit::actions::AUTH_LOGIN,
        Some("user"),
        Some(user.id.to_string()),
        active,
        None,
    )
    .await;

    Ok(Json(TokenResp {
        access_token: access,
        refresh_token: refresh,
        token_type: "Bearer",
        expires_in: state.config.access_ttl_secs,
        user: user_resp,
    }))
}
