//! **MFA (TOTP) endpoints** (issue #63): enrol an authenticator app, confirm +
//! enable it, disable it, report status, and complete a login step-up. Enrolment
//! is two-step — a secret is stored, then a valid code confirms it before the
//! account is challenged at every login.

use super::dto::{MfaStatusResp, MfaVerifyReq, TokenResp, TotpCodeReq, TotpSetupResp};
use super::helpers::build_token_resp;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use crate::{mfa, totp};
use chrono::Utc;
use entity::prelude::{User, UserTotp};
use rocket::serde::json::Json;
use rocket::{get, post, State};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};

const ISSUER: &str = "Acre Nexus";

fn now_secs() -> u64 {
    Utc::now().timestamp().max(0) as u64
}

/// `POST /auth/mfa/totp/setup` — begin enrolment: mint a secret and return it
/// (base32 + `otpauth` URI). Not yet active until confirmed.
#[rocket_okapi::openapi(tag = "Auth")]
#[post("/auth/mfa/totp/setup")]
pub async fn setup(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
) -> ApiResult<Json<TotpSetupResp>> {
    let existing = UserTotp::find_by_id(user.user_id).one(&db).await?;
    if existing.as_ref().map(|t| t.enabled).unwrap_or(false) {
        return Err(ApiError::Conflict(
            "MFA is already enabled — disable it first to re-enrol".into(),
        ));
    }
    let account = User::find_by_id(user.user_id)
        .one(&db)
        .await?
        .map(|u| u.email)
        .unwrap_or_else(|| user.user_id.to_string());

    let secret = totp::generate_secret();
    let (ct, nonce) = mfa::seal_secret(&state.config, &secret).map_err(ApiError::Internal)?;
    let now = Utc::now();
    match existing {
        Some(t) => {
            let mut am: entity::user_totp::ActiveModel = t.into();
            am.secret_ciphertext = Set(ct);
            am.secret_nonce = Set(nonce);
            am.enabled = Set(false);
            am.confirmed_at = Set(None);
            am.updated_at = Set(now.into());
            am.update(&db).await?;
        }
        None => {
            entity::user_totp::ActiveModel {
                user_id: Set(user.user_id),
                secret_ciphertext: Set(ct),
                secret_nonce: Set(nonce),
                enabled: Set(false),
                confirmed_at: Set(None),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }
            .insert(&db)
            .await?;
        }
    }

    Ok(Json(TotpSetupResp {
        otpauth_uri: totp::otpauth_uri(ISSUER, &account, &secret),
        secret,
    }))
}

/// `POST /auth/mfa/totp/confirm` — verify the first code and enable MFA.
#[rocket_okapi::openapi(tag = "Auth")]
#[post("/auth/mfa/totp/confirm", data = "<body>")]
pub async fn confirm(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    body: Json<TotpCodeReq>,
) -> ApiResult<Json<MfaStatusResp>> {
    let row = UserTotp::find_by_id(user.user_id)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::BadRequest("start an MFA enrolment first".into()))?;
    let secret = mfa::open_secret(&state.config, &row.secret_ciphertext, &row.secret_nonce)
        .map_err(ApiError::Internal)?;
    if !totp::verify(&secret, body.code.trim(), now_secs()) {
        return Err(ApiError::BadRequest(
            "that code isn't valid — try again".into(),
        ));
    }
    let now = Utc::now();
    let mut am: entity::user_totp::ActiveModel = row.into();
    am.enabled = Set(true);
    am.confirmed_at = Set(Some(now.into()));
    am.updated_at = Set(now.into());
    am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::AUTH_MFA_ENABLE,
        Some("user"),
        Some(user.user_id.to_string()),
        user.tenant_id,
        None,
    )
    .await;
    Ok(Json(MfaStatusResp { enabled: true }))
}

/// `POST /auth/mfa/totp/disable` — turn MFA off (requires a current code).
#[rocket_okapi::openapi(tag = "Auth")]
#[post("/auth/mfa/totp/disable", data = "<body>")]
pub async fn disable(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    body: Json<TotpCodeReq>,
) -> ApiResult<Json<MfaStatusResp>> {
    let row = UserTotp::find_by_id(user.user_id)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::BadRequest("MFA is not enabled".into()))?;
    // A live code is required to turn the second factor off.
    let secret = mfa::open_secret(&state.config, &row.secret_ciphertext, &row.secret_nonce)
        .map_err(ApiError::Internal)?;
    if !totp::verify(&secret, body.code.trim(), now_secs()) {
        return Err(ApiError::BadRequest("that code isn't valid".into()));
    }
    UserTotp::delete_by_id(user.user_id).exec(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::AUTH_MFA_DISABLE,
        Some("user"),
        Some(user.user_id.to_string()),
        user.tenant_id,
        None,
    )
    .await;
    Ok(Json(MfaStatusResp { enabled: false }))
}

/// `GET /auth/mfa/status` — whether the signed-in user has MFA enabled.
#[rocket_okapi::openapi(tag = "Auth")]
#[get("/auth/mfa/status")]
pub async fn status(db: crate::db::RequestDb, user: AuthUser) -> ApiResult<Json<MfaStatusResp>> {
    let enabled = super::helpers::mfa_enabled(&db, user.user_id).await?;
    Ok(Json(MfaStatusResp { enabled }))
}

/// `POST /auth/mfa/verify` — complete a login step-up: exchange the challenge
/// token + a current code for a full session.
#[rocket_okapi::openapi(tag = "Auth")]
#[post("/auth/mfa/verify", data = "<body>")]
pub async fn verify(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    body: Json<MfaVerifyReq>,
) -> ApiResult<Json<TokenResp>> {
    let user_id = mfa::verify_challenge_token(&state.config, &body.mfa_token)
        .ok_or(ApiError::Unauthorized)?;
    let row = UserTotp::find_by_id(user_id)
        .one(&db)
        .await?
        .filter(|t| t.enabled)
        .ok_or(ApiError::Unauthorized)?;
    let secret = mfa::open_secret(&state.config, &row.secret_ciphertext, &row.secret_nonce)
        .map_err(ApiError::Internal)?;
    if !totp::verify(&secret, body.code.trim(), now_secs()) {
        return Err(ApiError::Unauthorized);
    }
    let user = User::find_by_id(user_id)
        .one(&db)
        .await?
        .ok_or(ApiError::Unauthorized)?;
    if user.status != "active" {
        return Err(ApiError::Forbidden(format!(
            "account is {} — contact an administrator",
            user.status
        )));
    }
    let active = user.tenant_id;
    let resp = build_token_resp(state, &db, &user, active).await?;

    crate::audit::record(
        &db,
        Some(user.id),
        crate::audit::actions::AUTH_MFA_VERIFY,
        Some("user"),
        Some(user.id.to_string()),
        active,
        None,
    )
    .await;
    Ok(Json(resp))
}
