//! **Passwords**: forgot → reset link, set a password from an invite or reset
//! link, and change it while signed in. See [`crate::password_links`].

use crate::auth::{hash_password, verify_password, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::password_links::{self, PURPOSE_INVITE, PURPOSE_RESET};
use crate::state::AppState;
use chrono::Utc;
use entity::prelude::{RefreshToken, User};
use rocket::serde::json::Json;
use rocket::{get, post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ForgotReq {
    pub email: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SetPasswordReq {
    pub token: String,
    pub password: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ChangePasswordReq {
    pub current_password: String,
    pub new_password: String,
}

/// What the set-password page shows before the person types anything.
#[derive(Serialize, schemars::JsonSchema)]
pub struct PasswordLinkInfo {
    /// `invite` | `reset`
    pub purpose: String,
    pub email: String,
    pub name: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct SetPasswordResp {
    pub ok: bool,
    /// The account's email, so the page can sign the person straight in.
    pub email: String,
}

/// Revoke every live refresh token of a user — other devices sign out.
async fn revoke_sessions(db: &impl ConnectionTrait, user_id: Uuid) -> Result<(), ApiError> {
    let live = RefreshToken::find()
        .filter(entity::refresh_token::Column::UserId.eq(user_id))
        .filter(entity::refresh_token::Column::RevokedAt.is_null())
        .all(db)
        .await?;
    let now = Utc::now();
    for t in live {
        let mut am: entity::refresh_token::ActiveModel = t.into();
        am.revoked_at = Set(Some(now.into()));
        am.update(db).await?;
    }
    Ok(())
}

/// `POST /auth/password/forgot` — email (and text) a reset link. Always answers
/// the same way, so it never reveals whether an address has an account.
#[rocket_okapi::openapi(tag = "Auth")]
#[post("/auth/password/forgot", data = "<body>")]
pub async fn forgot(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    body: Json<ForgotReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let email = body.email.trim().to_lowercase();
    let user = User::find()
        .filter(entity::user::Column::Email.eq(email))
        .one(&db)
        .await?;
    // Suspended / disabled accounts get nothing; invited ones get a fresh
    // invite (that is what they were missing).
    if let Some(user) = user.filter(|u| u.status == "active" || u.status == "invited") {
        let purpose = if user.status == "invited" {
            PURPOSE_INVITE
        } else {
            PURPOSE_RESET
        };
        let (token, row) = password_links::issue(&db, user.id, purpose).await?;
        let tenant = password_links::home_tenant(&db, &user).await;
        password_links::deliver(&db, tenant, &user, &token, &row).await;
        crate::audit::record(
            &db,
            Some(user.id),
            crate::audit::actions::AUTH_PASSWORD_RESET_REQUEST,
            Some("user"),
            Some(user.id.to_string()),
            tenant,
            Some(serde_json::json!({ "purpose": purpose })),
        )
        .await;
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// `GET /auth/password/link/<token>` — check a link before showing the form:
/// who it is for and whether it's an invite or a reset. 404 once used/expired.
#[rocket_okapi::openapi(tag = "Auth")]
#[get("/auth/password/link/<token>")]
pub async fn link_info(db: crate::db::RequestDb, token: &str) -> ApiResult<Json<PasswordLinkInfo>> {
    let gone = || ApiError::NotFound("this link has expired or was already used".into());
    let row = password_links::find_valid(&db, token)
        .await?
        .ok_or_else(gone)?;
    let user = User::find_by_id(row.user_id)
        .one(&db)
        .await?
        .ok_or_else(gone)?;
    Ok(Json(PasswordLinkInfo {
        purpose: row.purpose,
        email: user.email,
        name: user.name,
    }))
}

/// `POST /auth/password/set` — choose a password from an invite or reset link.
/// Activates an invited account, uses up the link, and signs out other devices.
#[rocket_okapi::openapi(tag = "Auth")]
#[post("/auth/password/set", data = "<body>")]
pub async fn set_password(
    db: crate::db::RequestDb,
    body: Json<SetPasswordReq>,
) -> ApiResult<Json<SetPasswordResp>> {
    let gone = || ApiError::NotFound("this link has expired or was already used".into());
    let row = password_links::find_valid(&db, &body.token)
        .await?
        .ok_or_else(gone)?;
    let user = User::find_by_id(row.user_id)
        .one(&db)
        .await?
        .ok_or_else(gone)?;
    if user.status == "suspended" || user.status == "disabled" {
        return Err(ApiError::Forbidden(format!(
            "account is {} — contact an administrator",
            user.status
        )));
    }
    if let Some(problem) = password_links::password_problem(&body.password, &user.email) {
        return Err(ApiError::BadRequest(problem));
    }

    let now = Utc::now();
    let was_invited = user.status == "invited";
    let hash = hash_password(&body.password).map_err(ApiError::Internal)?;
    let mut am: entity::user::ActiveModel = user.clone().into();
    am.password_hash = Set(hash);
    am.status = Set("active".into());
    am.update(&db).await?;

    // An invite also counts as accepting every pending membership.
    if was_invited {
        let pending = entity::prelude::Membership::find()
            .filter(entity::membership::Column::UserId.eq(user.id))
            .filter(entity::membership::Column::Status.eq("invited"))
            .all(&db)
            .await?;
        for m in pending {
            let mut mm: entity::membership::ActiveModel = m.into();
            mm.status = Set("active".into());
            mm.update(&db).await?;
        }
    }

    let purpose = row.purpose.clone();
    let mut tm: entity::password_token::ActiveModel = row.into();
    tm.used_at = Set(Some(now.into()));
    tm.update(&db).await?;
    revoke_sessions(&db, user.id).await?;

    let action = if purpose == PURPOSE_INVITE {
        crate::audit::actions::AUTH_INVITE_ACCEPT
    } else {
        crate::audit::actions::AUTH_PASSWORD_RESET
    };
    let tenant = password_links::home_tenant(&db, &user).await;
    crate::audit::record(
        &db,
        Some(user.id),
        action,
        Some("user"),
        Some(user.id.to_string()),
        tenant,
        None,
    )
    .await;

    Ok(Json(SetPasswordResp {
        ok: true,
        email: user.email,
    }))
}

/// `POST /auth/password/change` — change the password while signed in. Needs
/// the current one; other devices are signed out.
#[rocket_okapi::openapi(tag = "Auth")]
#[post("/auth/password/change", data = "<body>")]
pub async fn change_password(
    db: crate::db::RequestDb,
    user: AuthUser,
    body: Json<ChangePasswordReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let account = User::find_by_id(user.user_id)
        .one(&db)
        .await?
        .ok_or(ApiError::Unauthorized)?;
    if !verify_password(&body.current_password, &account.password_hash) {
        return Err(ApiError::BadRequest(
            "your current password isn't right".into(),
        ));
    }
    if let Some(problem) = password_links::password_problem(&body.new_password, &account.email) {
        return Err(ApiError::BadRequest(problem));
    }
    let hash = hash_password(&body.new_password).map_err(ApiError::Internal)?;
    let mut am: entity::user::ActiveModel = account.into();
    am.password_hash = Set(hash);
    am.update(&db).await?;
    revoke_sessions(&db, user.user_id).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::AUTH_PASSWORD_CHANGE,
        Some("user"),
        Some(user.user_id.to_string()),
        user.tenant_id,
        None,
    )
    .await;
    Ok(Json(serde_json::json!({ "ok": true })))
}
