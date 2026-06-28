use super::dto::{SwitchReq, SwitchResp};
use super::helpers::{build_user_resp, load_memberships, permissions_for};
use crate::auth::{issue_access_token, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use entity::prelude::User;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::EntityTrait;

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
        .one(&state.user_db)
        .await?
        .ok_or(ApiError::Unauthorized)?;
    let target = body.tenant_id;

    // Authorize the switch: staff may enter any workspace; everyone else must
    // hold an active membership in the target.
    let memberships = load_memberships(&state.user_db, u.id).await?;
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

    let perms = permissions_for(&state.user_db, u.id, target).await?;
    let access = issue_access_token(
        &state.config,
        u.id,
        target,
        u.is_platform_staff,
        perms.clone(),
    )
    .map_err(ApiError::Internal)?;
    let user_resp = build_user_resp(&state.user_db, &u, target, perms).await?;

    crate::audit::record(
        &state.user_db,
        Some(u.id),
        crate::audit::actions::AUTH_SWITCH_WORKSPACE,
        Some("workspace"),
        target.map(|t| t.to_string()),
        target,
        None,
    )
    .await;

    Ok(Json(SwitchResp {
        access_token: access,
        token_type: "Bearer",
        expires_in: state.config.access_ttl_secs,
        user: user_resp,
    }))
}
