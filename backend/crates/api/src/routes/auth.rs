//! Authentication endpoints: login, token refresh, current user, logout.

use crate::auth::{
    self, hash_secret, issue_access_token, random_secret, verify_password, AuthUser,
};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use chrono::{Duration, Utc};
use entity::prelude::{RefreshToken, Role, RolePermission, User, UserRole};
use rocket::serde::json::Json;
use rocket::{post, get, State};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct LoginReq {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct TokenResp {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: &'static str,
    pub expires_in: i64,
    pub user: UserResp,
}

#[derive(Serialize)]
pub struct UserResp {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub tenant_id: Option<Uuid>,
    pub is_platform_staff: bool,
    pub permissions: Vec<String>,
}

/// Resolve the full permission set for a user across their assigned roles.
pub async fn permissions_for(
    db: &sea_orm::DatabaseConnection,
    user_id: Uuid,
) -> Result<Vec<String>, ApiError> {
    let role_ids: Vec<Uuid> = UserRole::find()
        .filter(entity::user_role::Column::UserId.eq(user_id))
        .all(db)
        .await?
        .into_iter()
        .map(|r| r.role_id)
        .collect();
    if role_ids.is_empty() {
        return Ok(vec![]);
    }
    let _roles = Role::find()
        .filter(entity::role::Column::Id.is_in(role_ids.clone()))
        .all(db)
        .await?;
    let perms: Vec<String> = RolePermission::find()
        .filter(entity::role_permission::Column::RoleId.is_in(role_ids))
        .all(db)
        .await?
        .into_iter()
        .map(|p| p.permission)
        .collect();
    // Dedup.
    let mut set: Vec<String> = perms;
    set.sort();
    set.dedup();
    Ok(set)
}

/// `POST /auth/login` — exchange email + password for an access/refresh token pair.
#[post("/auth/login", data = "<body>")]
pub async fn login(state: &State<AppState>, body: Json<LoginReq>) -> ApiResult<Json<TokenResp>> {
    let user = User::find()
        .filter(entity::user::Column::Email.eq(body.email.to_lowercase()))
        .one(&state.db)
        .await?
        .ok_or(ApiError::Unauthorized)?;

    if !verify_password(&body.password, &user.password_hash) {
        return Err(ApiError::Unauthorized);
    }

    let perms = permissions_for(&state.db, user.id).await?;
    let access = issue_access_token(
        &state.config,
        user.id,
        user.tenant_id,
        user.is_platform_staff,
        perms.clone(),
    )
    .map_err(ApiError::Internal)?;

    let refresh = issue_refresh_token(state, user.id).await?;

    Ok(Json(TokenResp {
        access_token: access,
        refresh_token: refresh,
        token_type: "Bearer",
        expires_in: state.config.access_ttl_secs,
        user: UserResp {
            id: user.id,
            email: user.email,
            name: user.name,
            tenant_id: user.tenant_id,
            is_platform_staff: user.is_platform_staff,
            permissions: perms,
        },
    }))
}

async fn issue_refresh_token(state: &AppState, user_id: Uuid) -> ApiResult<String> {
    let secret = random_secret(32);
    let now = Utc::now();
    let model = entity::refresh_token::ActiveModel {
        id: Set(Uuid::new_v4()),
        user_id: Set(user_id),
        token_hash: Set(hash_secret(&secret)),
        expires_at: Set((now + Duration::seconds(state.config.refresh_ttl_secs)).into()),
        revoked_at: Set(None),
        created_at: Set(now.into()),
    };
    model.insert(&state.db).await?;
    Ok(secret)
}

#[derive(Deserialize)]
pub struct RefreshReq {
    pub refresh_token: String,
}

/// `POST /auth/refresh` — rotate a refresh token for a fresh access/refresh pair.
#[post("/auth/refresh", data = "<body>")]
pub async fn refresh(state: &State<AppState>, body: Json<RefreshReq>) -> ApiResult<Json<TokenResp>> {
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
    let perms = permissions_for(&state.db, user.id).await?;
    let access = issue_access_token(
        &state.config,
        user.id,
        user.tenant_id,
        user.is_platform_staff,
        perms.clone(),
    )
    .map_err(ApiError::Internal)?;
    let new_refresh = issue_refresh_token(state, user.id).await?;

    Ok(Json(TokenResp {
        access_token: access,
        refresh_token: new_refresh,
        token_type: "Bearer",
        expires_in: state.config.access_ttl_secs,
        user: UserResp {
            id: user.id,
            email: user.email,
            name: user.name,
            tenant_id: user.tenant_id,
            is_platform_staff: user.is_platform_staff,
            permissions: perms,
        },
    }))
}

/// `GET /auth/me` — the currently authenticated principal.
#[get("/auth/me")]
pub async fn me(state: &State<AppState>, user: AuthUser) -> ApiResult<Json<UserResp>> {
    let u = User::find_by_id(user.user_id)
        .one(&state.db)
        .await?
        .ok_or(ApiError::Unauthorized)?;
    let perms = permissions_for(&state.db, u.id).await?;
    Ok(Json(UserResp {
        id: u.id,
        email: u.email,
        name: u.name,
        tenant_id: u.tenant_id,
        is_platform_staff: u.is_platform_staff,
        permissions: perms,
    }))
}

#[derive(Deserialize)]
pub struct LogoutReq {
    pub refresh_token: String,
}

/// `POST /auth/logout` — revoke a refresh token. Access tokens expire naturally.
#[post("/auth/logout", data = "<body>")]
pub async fn logout(
    state: &State<AppState>,
    _user: AuthUser,
    body: Json<LogoutReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let hash = auth::hash_secret(&body.refresh_token);
    if let Some(tok) = RefreshToken::find()
        .filter(entity::refresh_token::Column::TokenHash.eq(hash))
        .one(&state.db)
        .await?
    {
        let mut am: entity::refresh_token::ActiveModel = tok.into();
        am.revoked_at = Set(Some(Utc::now().into()));
        am.update(&state.db).await?;
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}
