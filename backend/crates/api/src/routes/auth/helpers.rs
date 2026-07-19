use super::dto::{MembershipSummary, TokenResp, UserResp, WorkspaceSummary};
use crate::auth::{hash_secret, issue_access_token, random_secret};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use chrono::{Duration, Utc};
use entity::prelude::{Membership, RolePermission, Tenant, UserRole, UserTotp};
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use std::collections::HashSet;
use uuid::Uuid;

/// Resolve the effective **workspace-wide** permission set for a user in a given
/// workspace. Platform-scoped assignments (`tenant_id IS NULL`) always apply;
/// tenant assignments apply only when they match `active_tenant`.
///
/// Only `platform`- and `tenant`-scoped grants flatten into this set (and thus
/// into the JWT). Narrower grants (`entity` / `portfolio` / `property`) are
/// resolved per-resource via [`crate::rbac::scope_covers`] — see
/// `crate::tenancy::resolve` — so a contract bookkeeper scoped to one LLC does
/// not gain firm-wide permissions.
pub(crate) async fn permissions_for(
    db: &impl ConnectionTrait,
    user_id: Uuid,
    active_tenant: Option<Uuid>,
) -> Result<Vec<String>, ApiError> {
    let assignments = UserRole::find()
        .filter(entity::user_role::Column::UserId.eq(user_id))
        .all(db)
        .await?;
    let role_ids: Vec<Uuid> = assignments
        .into_iter()
        .filter(|r| {
            // Only workspace-wide grants contribute to the flat permission set.
            r.scope == crate::rbac::scope::SCOPE_PLATFORM
                || r.scope == crate::rbac::scope::SCOPE_TENANT
        })
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
pub(crate) async fn load_memberships(
    db: &impl ConnectionTrait,
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
pub(crate) fn workspaces_from(
    memberships: &[MembershipSummary],
    is_staff: bool,
) -> Vec<WorkspaceSummary> {
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
pub(crate) async fn build_user_resp(
    db: &impl ConnectionTrait,
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

pub(crate) async fn issue_refresh_token(
    db: &impl ConnectionTrait,
    refresh_ttl_secs: i64,
    user_id: Uuid,
) -> ApiResult<String> {
    let secret = random_secret(32);
    let now = Utc::now();
    let model = entity::refresh_token::ActiveModel {
        id: Set(Uuid::new_v4()),
        user_id: Set(user_id),
        token_hash: Set(hash_secret(&secret)),
        expires_at: Set((now + Duration::seconds(refresh_ttl_secs)).into()),
        revoked_at: Set(None),
        created_at: Set(now.into()),
    };
    model.insert(db).await?;
    Ok(secret)
}

/// Mint a full session — access token + rotating refresh token + the user
/// snapshot — for `user` scoped to `active`. The shared tail of every
/// successful authentication (password, federated, MFA-verified).
pub(crate) async fn build_token_resp(
    state: &AppState,
    db: &impl ConnectionTrait,
    user: &entity::user::Model,
    active: Option<Uuid>,
) -> ApiResult<TokenResp> {
    let perms = permissions_for(db, user.id, active).await?;
    let access = issue_access_token(
        &state.config,
        user.id,
        active,
        user.is_platform_staff,
        perms.clone(),
    )
    .map_err(ApiError::Internal)?;
    let refresh = issue_refresh_token(db, state.config.refresh_ttl_secs, user.id).await?;
    let user_resp = build_user_resp(db, user, active, perms).await?;
    Ok(TokenResp {
        access_token: access,
        refresh_token: refresh,
        token_type: "Bearer",
        expires_in: state.config.access_ttl_secs,
        user: user_resp,
    })
}

/// Whether the user has confirmed, enabled TOTP MFA.
pub(crate) async fn mfa_enabled(db: &impl ConnectionTrait, user_id: Uuid) -> ApiResult<bool> {
    Ok(UserTotp::find_by_id(user_id)
        .one(db)
        .await?
        .map(|t| t.enabled)
        .unwrap_or(false))
}

/// A just-authenticated user either gets a session, or — if MFA stands in the
/// way — a challenge token to complete at `POST /auth/mfa/verify`.
pub(crate) enum AuthOutcome {
    Session(Box<TokenResp>),
    Mfa(String),
}

/// Resolve the outcome of a successful first factor (password or social):
/// a full session, unless the account has TOTP MFA enabled.
pub(crate) async fn auth_outcome(
    state: &AppState,
    db: &impl ConnectionTrait,
    user: &entity::user::Model,
    active: Option<Uuid>,
) -> ApiResult<AuthOutcome> {
    if mfa_enabled(db, user.id).await? {
        let token = crate::mfa::issue_challenge_token(&state.config, user.id)
            .map_err(ApiError::Internal)?;
        Ok(AuthOutcome::Mfa(token))
    } else {
        Ok(AuthOutcome::Session(Box::new(
            build_token_resp(state, db, user, active).await?,
        )))
    }
}
