//! Authentication endpoints: login, token refresh, current user, logout.

use crate::auth::{
    self, hash_secret, issue_access_token, random_secret, verify_password, AuthUser,
};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use chrono::{Duration, Utc};
use entity::prelude::{Membership, RefreshToken, RolePermission, Tenant, User, UserRole};
use rocket::serde::json::Json;
use rocket::{get, post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Deserialize, schemars::JsonSchema)]
pub struct LoginReq {
    pub email: String,
    pub password: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct TokenResp {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: &'static str,
    pub expires_in: i64,
    pub user: UserResp,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct UserResp {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    /// Primary tenant of the account (back-compat).
    pub tenant_id: Option<Uuid>,
    /// The workspace the current token is scoped to (`None` = Acre HQ / platform).
    pub active_tenant_id: Option<Uuid>,
    pub is_platform_staff: bool,
    pub permissions: Vec<String>,
    /// Every persona the user holds, across platform and tenants.
    pub memberships: Vec<MembershipSummary>,
    /// Workspaces the user can switch into (drives the workspace switcher).
    pub workspaces: Vec<WorkspaceSummary>,
}

/// One of a user's personas, with the owning workspace resolved for display.
#[derive(Serialize, schemars::JsonSchema)]
pub struct MembershipSummary {
    pub scope: String,
    pub tenant_id: Option<Uuid>,
    pub tenant_slug: Option<String>,
    pub tenant_name: Option<String>,
    pub profile_type: String,
    pub title: Option<String>,
    pub status: String,
    pub is_primary: bool,
}

/// A workspace the user can operate in.
#[derive(Serialize, schemars::JsonSchema, Clone)]
pub struct WorkspaceSummary {
    /// `platform` (Acre HQ) or `tenant` (a client workspace).
    pub kind: String,
    pub tenant_id: Option<Uuid>,
    pub slug: Option<String>,
    pub name: String,
}

/// Resolve the effective permission set for a user **in a given workspace**.
/// Platform-scoped role assignments (`tenant_id IS NULL`) always apply; tenant
/// assignments apply only when they match `active_tenant`.
pub async fn permissions_for(
    db: &sea_orm::DatabaseConnection,
    user_id: Uuid,
    active_tenant: Option<Uuid>,
) -> Result<Vec<String>, ApiError> {
    let assignments = UserRole::find()
        .filter(entity::user_role::Column::UserId.eq(user_id))
        .all(db)
        .await?;
    let role_ids: Vec<Uuid> = assignments
        .into_iter()
        .filter(|r| match (r.tenant_id, active_tenant) {
            (None, _) => true,            // platform / global assignment
            (Some(t), Some(a)) => t == a, // tenant assignment in the active workspace
            (Some(_), None) => false,     // tenant assignment, but not in this workspace
        })
        .map(|r| r.role_id)
        .collect();
    if role_ids.is_empty() {
        return Ok(vec![]);
    }
    let perms: Vec<String> = RolePermission::find()
        .filter(entity::role_permission::Column::RoleId.is_in(role_ids))
        .all(db)
        .await?
        .into_iter()
        .map(|p| p.permission)
        .collect();
    let mut set: Vec<String> = perms;
    set.sort();
    set.dedup();
    Ok(set)
}

/// Load a user's personas, resolving tenant slug/name for display.
pub async fn load_memberships(
    db: &sea_orm::DatabaseConnection,
    user_id: Uuid,
) -> Result<Vec<MembershipSummary>, ApiError> {
    let rows = Membership::find()
        .filter(entity::membership::Column::UserId.eq(user_id))
        .all(db)
        .await?;
    let mut out = Vec::new();
    for m in rows {
        let (slug, name) = match m.tenant_id {
            Some(tid) => match Tenant::find_by_id(tid).one(db).await? {
                Some(t) => (Some(t.slug), Some(t.name)),
                None => (None, None),
            },
            None => (None, None),
        };
        out.push(MembershipSummary {
            scope: m.scope,
            tenant_id: m.tenant_id,
            tenant_slug: slug,
            tenant_name: name,
            profile_type: m.profile_type,
            title: m.title,
            status: m.status,
            is_primary: m.is_primary,
        });
    }
    Ok(out)
}

/// Derive the distinct workspaces a user can switch into from their memberships.
fn workspaces_from(memberships: &[MembershipSummary], is_staff: bool) -> Vec<WorkspaceSummary> {
    let mut out = Vec::new();
    if is_staff || memberships.iter().any(|m| m.scope == "platform") {
        out.push(WorkspaceSummary {
            kind: "platform".into(),
            tenant_id: None,
            slug: None,
            name: "Acre HQ".into(),
        });
    }
    let mut seen = HashSet::new();
    for m in memberships.iter().filter(|m| m.scope == "tenant") {
        if let Some(tid) = m.tenant_id {
            if seen.insert(tid) {
                out.push(WorkspaceSummary {
                    kind: "tenant".into(),
                    tenant_id: Some(tid),
                    slug: m.tenant_slug.clone(),
                    name: m.tenant_name.clone().unwrap_or_else(|| "Workspace".into()),
                });
            }
        }
    }
    out
}

/// Assemble a [`UserResp`] for `user` scoped to `active_tenant`.
async fn build_user_resp(
    db: &sea_orm::DatabaseConnection,
    user: &entity::user::Model,
    active_tenant: Option<Uuid>,
    perms: Vec<String>,
) -> Result<UserResp, ApiError> {
    let memberships = load_memberships(db, user.id).await?;
    let workspaces = workspaces_from(&memberships, user.is_platform_staff);
    Ok(UserResp {
        id: user.id,
        email: user.email.clone(),
        name: user.name.clone(),
        tenant_id: user.tenant_id,
        active_tenant_id: active_tenant,
        is_platform_staff: user.is_platform_staff,
        permissions: perms,
        memberships,
        workspaces,
    })
}

/// `POST /auth/login` — exchange email + password for an access/refresh token pair.
#[rocket_okapi::openapi(tag = "Auth")]
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
        if let Err(e) = am.update(&state.db).await {
            tracing::warn!("failed to update last_login_at: {e}");
        }
    }

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

    let refresh = issue_refresh_token(state, user.id).await?;
    let user_resp = build_user_resp(&state.db, &user, active, perms).await?;

    Ok(Json(TokenResp {
        access_token: access,
        refresh_token: refresh,
        token_type: "Bearer",
        expires_in: state.config.access_ttl_secs,
        user: user_resp,
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

#[derive(Deserialize, schemars::JsonSchema)]
pub struct RefreshReq {
    pub refresh_token: String,
}

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

    Ok(Json(TokenResp {
        access_token: access,
        refresh_token: new_refresh,
        token_type: "Bearer",
        expires_in: state.config.access_ttl_secs,
        user: user_resp,
    }))
}

/// `GET /auth/me` — the currently authenticated principal.
#[rocket_okapi::openapi(tag = "Auth")]
#[get("/auth/me")]
pub async fn me(state: &State<AppState>, user: AuthUser) -> ApiResult<Json<UserResp>> {
    let u = User::find_by_id(user.user_id)
        .one(&state.db)
        .await?
        .ok_or(ApiError::Unauthorized)?;
    // Reflect the workspace the current token is scoped to (from the JWT).
    let active = user.tenant_id;
    let perms = permissions_for(&state.db, u.id, active).await?;
    let resp = build_user_resp(&state.db, &u, active, perms).await?;
    Ok(Json(resp))
}

/// `GET /auth/workspaces` — the workspaces the current user can switch into.
#[rocket_okapi::openapi(tag = "Auth")]
#[get("/auth/workspaces")]
pub async fn workspaces(
    state: &State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Vec<WorkspaceSummary>>> {
    let u = User::find_by_id(user.user_id)
        .one(&state.db)
        .await?
        .ok_or(ApiError::Unauthorized)?;
    let memberships = load_memberships(&state.db, u.id).await?;
    Ok(Json(workspaces_from(&memberships, u.is_platform_staff)))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SwitchReq {
    /// Target workspace; `null` selects the platform (Acre HQ) context.
    pub tenant_id: Option<Uuid>,
}

/// Response from a workspace switch — a fresh access token scoped to the chosen
/// workspace, with permissions re-resolved for it.
#[derive(Serialize, schemars::JsonSchema)]
pub struct SwitchResp {
    pub access_token: String,
    pub token_type: &'static str,
    pub expires_in: i64,
    pub user: UserResp,
}

/// `POST /auth/switch` — re-scope the session to another workspace the user
/// belongs to. Issues a new access token whose permissions are resolved for the
/// target workspace. The refresh token is unchanged.
#[rocket_okapi::openapi(tag = "Auth")]
#[post("/auth/switch", data = "<body>")]
pub async fn switch_workspace(
    state: &State<AppState>,
    user: AuthUser,
    body: Json<SwitchReq>,
) -> ApiResult<Json<SwitchResp>> {
    let u = User::find_by_id(user.user_id)
        .one(&state.db)
        .await?
        .ok_or(ApiError::Unauthorized)?;
    let target = body.tenant_id;

    // Authorize the switch: staff may enter any workspace; everyone else must
    // hold an active membership in the target.
    let memberships = load_memberships(&state.db, u.id).await?;
    let authorized = match target {
        Some(tid) => {
            u.is_platform_staff
                || memberships.iter().any(|m| {
                    m.scope == "tenant" && m.tenant_id == Some(tid) && m.status == "active"
                })
        }
        None => u.is_platform_staff || memberships.iter().any(|m| m.scope == "platform"),
    };
    if !authorized {
        return Err(ApiError::Forbidden(
            "you are not a member of that workspace".into(),
        ));
    }

    let perms = permissions_for(&state.db, u.id, target).await?;
    let access = issue_access_token(
        &state.config,
        u.id,
        target,
        u.is_platform_staff,
        perms.clone(),
    )
    .map_err(ApiError::Internal)?;
    let user_resp = build_user_resp(&state.db, &u, target, perms).await?;

    Ok(Json(SwitchResp {
        access_token: access,
        token_type: "Bearer",
        expires_in: state.config.access_ttl_secs,
        user: user_resp,
    }))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct LogoutReq {
    pub refresh_token: String,
}

/// `POST /auth/logout` — revoke a refresh token. Access tokens expire naturally.
#[rocket_okapi::openapi(tag = "Auth")]
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
