use super::dto::{LoginReq, LoginResp, MfaChallengeResp};
use super::helpers::{auth_outcome, AuthOutcome};
use crate::auth::verify_password;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use chrono::Utc;
use entity::prelude::User;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

/// `POST /auth/login` — exchange email + password for a session, or (when the
/// account has TOTP MFA) an MFA challenge to complete first. The no-MFA
/// response is exactly the historical access/refresh token pair.
#[rocket_okapi::openapi(tag = "Auth")]
#[post("/auth/login", data = "<body>")]
pub async fn login(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    body: Json<LoginReq>,
) -> ApiResult<Json<LoginResp>> {
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
    match auth_outcome(state, &db, &user, active).await? {
        AuthOutcome::Session(token) => {
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
            Ok(Json(LoginResp::Token(token)))
        }
        // MFA-enabled: the full login is recorded once the second factor clears.
        AuthOutcome::Mfa(mfa_token) => Ok(Json(LoginResp::Mfa(MfaChallengeResp {
            mfa_required: true,
            mfa_token,
        }))),
    }
}
