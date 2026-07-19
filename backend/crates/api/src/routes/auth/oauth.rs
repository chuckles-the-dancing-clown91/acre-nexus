//! **Federated login endpoints** (issue #63): start a social-login flow, handle
//! the provider callback (resolving or provisioning the user + linking the
//! identity, then minting a session or MFA challenge), and the hermetic sandbox
//! provider's authorize step. The OAuth engine is [`crate::oauth`].

use super::dto::{
    MfaChallengeResp, OauthCallbackReq, OauthCallbackResp, OauthStartReq, OauthStartResp,
};
use super::helpers::{auth_outcome, AuthOutcome};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::oauth::{self, ExternalIdentity, FlowState};
use crate::routes::iam::dto::NewMembership;
use crate::routes::iam::dto::ProfileInput;
use crate::routes::iam::helpers::{add_membership_inner, upsert_profile_inner};
use crate::state::AppState;
use chrono::Utc;
use entity::prelude::{FederatedIdentity, Tenant, User};
use rocket::response::Redirect;
use rocket::serde::json::Json;
use rocket::{get, post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `POST /auth/oauth/<provider>/start` — begin a social login (or, with
/// `intent=link`, attach a provider to the signed-in account).
#[rocket_okapi::openapi(tag = "Auth")]
#[post("/auth/oauth/<provider>/start", data = "<body>")]
pub async fn start(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    // Optional: present only for the `link` intent (the user is signed in).
    user: Option<AuthUser>,
    provider: &str,
    body: Json<OauthStartReq>,
) -> ApiResult<Json<OauthStartResp>> {
    if !oauth::is_valid_provider(provider) {
        return Err(ApiError::BadRequest(format!(
            "unsupported provider '{provider}'"
        )));
    }
    let b = body.into_inner();
    let intent = b.intent.as_deref().unwrap_or("login");

    let (tenant_id, link_user_id) = match intent {
        "link" => {
            let u = user.ok_or(ApiError::Unauthorized)?;
            (None, Some(u.user_id))
        }
        "login" => {
            let slug = b
                .tenant
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    ApiError::BadRequest("a workspace (tenant) is required to sign in".into())
                })?;
            let tenant = Tenant::find()
                .filter(entity::tenant::Column::Slug.eq(slug))
                .one(&db)
                .await?
                .ok_or_else(|| ApiError::NotFound("workspace not found".into()))?;
            (Some(tenant.id), None)
        }
        other => {
            return Err(ApiError::BadRequest(format!(
                "invalid intent '{other}' (expected login | link)"
            )))
        }
    };

    let res = oauth::start(
        &state.config,
        &db,
        provider,
        intent,
        tenant_id,
        link_user_id,
    )
    .await?;
    Ok(Json(OauthStartResp {
        authorize_url: res.authorize_url,
        sandbox: res.sandbox,
    }))
}

/// `GET /auth/oauth/<provider>/sandbox?state&email` — the hermetic sandbox
/// provider's "consent": mint a code for the simulated account and redirect
/// back to the app callback. Only reachable when the provider isn't live.
#[rocket_okapi::openapi(tag = "Auth")]
#[get("/auth/oauth/<provider>/sandbox?<state>&<email>")]
pub async fn sandbox(
    app_state: &State<AppState>,
    provider: &str,
    state: &str,
    email: Option<&str>,
) -> ApiResult<Redirect> {
    if oauth::is_live(provider) {
        return Err(ApiError::BadRequest(
            "the sandbox provider is disabled (this provider is live)".into(),
        ));
    }
    let url = oauth::sandbox_redirect(&app_state.config, provider, state, email)?;
    Ok(Redirect::to(url))
}

/// `POST /auth/oauth/<provider>/callback` — complete the flow the browser
/// landed back from.
#[rocket_okapi::openapi(tag = "Auth")]
#[post("/auth/oauth/<provider>/callback", data = "<body>")]
pub async fn callback(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    provider: &str,
    body: Json<OauthCallbackReq>,
) -> ApiResult<Json<OauthCallbackResp>> {
    if !oauth::is_valid_provider(provider) {
        return Err(ApiError::BadRequest(format!(
            "unsupported provider '{provider}'"
        )));
    }
    let b = body.into_inner();
    let (identity, flow) = oauth::exchange(&state.config, &db, provider, &b.code, &b.state).await?;
    complete(state, &db, identity, flow).await.map(Json)
}

/// Resolve or provision the user for a completed flow, then mint the outcome.
async fn complete(
    state: &AppState,
    db: &crate::db::RequestDb,
    identity: ExternalIdentity,
    flow: FlowState,
) -> ApiResult<OauthCallbackResp> {
    let existing_link = FederatedIdentity::find()
        .filter(entity::federated_identity::Column::Provider.eq(&identity.provider))
        .filter(entity::federated_identity::Column::Subject.eq(&identity.subject))
        .one(db)
        .await?;

    // ---- Link intent: attach to the already-signed-in user ----
    if flow.intent == "link" {
        let link_user_id = flow
            .link_user_id
            .ok_or_else(|| ApiError::BadRequest("link flow is missing its user".into()))?;
        if let Some(l) = existing_link {
            if l.user_id == link_user_id {
                return Ok(linked_resp(&identity)); // idempotent re-link
            }
            return Err(ApiError::Conflict(format!(
                "this {} account is already linked to another user",
                identity.provider
            )));
        }
        create_link(db, link_user_id, &identity).await?;
        audit(
            db,
            link_user_id,
            crate::audit::actions::AUTH_OAUTH_LINK,
            None,
        )
        .await;
        return Ok(linked_resp(&identity));
    }

    // ---- Login / signup intent ----
    let user = match existing_link {
        Some(l) => User::find_by_id(l.user_id)
            .one(db)
            .await?
            .ok_or(ApiError::Unauthorized)?,
        None => {
            // No linked identity yet — attach to an existing account with the
            // same (provider-verified) email, or provision a fresh one.
            match User::find()
                .filter(entity::user::Column::Email.eq(identity.email.to_lowercase()))
                .one(db)
                .await?
            {
                Some(u) => {
                    create_link(db, u.id, &identity).await?;
                    audit(db, u.id, crate::audit::actions::AUTH_OAUTH_LINK, None).await;
                    u
                }
                None => provision_user(state, db, &identity, flow.tenant_id).await?,
            }
        }
    };

    if user.status != "active" {
        return Err(ApiError::Forbidden(format!(
            "account is {} — contact an administrator",
            user.status
        )));
    }
    // Best-effort sign-in timestamp.
    {
        let mut am: entity::user::ActiveModel = user.clone().into();
        am.last_login_at = Set(Some(Utc::now().into()));
        let _ = am.update(db).await;
    }

    let active = user.tenant_id;
    match auth_outcome(state, db, &user, active).await? {
        AuthOutcome::Session(token) => {
            audit(db, user.id, crate::audit::actions::AUTH_LOGIN, active).await;
            Ok(OauthCallbackResp {
                outcome: "session".into(),
                session: Some(token),
                mfa: None,
                provider: None,
                email: None,
            })
        }
        AuthOutcome::Mfa(mfa_token) => Ok(OauthCallbackResp {
            outcome: "mfa".into(),
            session: None,
            mfa: Some(MfaChallengeResp {
                mfa_required: true,
                mfa_token,
            }),
            provider: None,
            email: None,
        }),
    }
}

/// Provision a brand-new user from a first-time social login: an `app_user`
/// (with an unusable random password — they authenticate via the provider), a
/// renter `membership` in the workspace, a pending `user_profile`, and the
/// federated-identity link.
async fn provision_user(
    state: &AppState,
    db: &crate::db::RequestDb,
    identity: &ExternalIdentity,
    tenant_id: Option<Uuid>,
) -> ApiResult<entity::user::Model> {
    let tenant_id = tenant_id
        .ok_or_else(|| ApiError::BadRequest("a workspace is required to sign up".into()))?;
    let name = identity
        .name
        .clone()
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| {
            identity
                .email
                .split('@')
                .next()
                .unwrap_or("New User")
                .to_string()
        });
    let uid = Uuid::new_v4();
    let password_hash =
        crate::auth::hash_password(&crate::auth::random_secret(24)).map_err(ApiError::Internal)?;
    let now = Utc::now();
    let user = entity::user::ActiveModel {
        id: Set(uid),
        tenant_id: Set(Some(tenant_id)),
        email: Set(identity.email.to_lowercase()),
        username: Set(None),
        password_hash: Set(password_hash),
        name: Set(name.clone()),
        is_platform_staff: Set(false),
        status: Set("active".into()),
        last_login_at: Set(Some(now.into())),
        created_at: Set(now.into()),
    }
    .insert(db)
    .await?;

    // Renter persona → grants the `renter` role, and makes this their home
    // workspace (drives the switcher).
    add_membership_inner(
        db,
        uid,
        &NewMembership {
            scope: "tenant".into(),
            tenant_id: Some(tenant_id),
            profile_type: "renter".into(),
            title: None,
        },
        true,
    )
    .await?;

    // Pending profile — the name from the provider; the rest is filled in the
    // portal. Best-effort: a profile hiccup must not sink the signup.
    let (first, last) = split_name(&name);
    let _ = upsert_profile_inner(
        db,
        &state.config.pii_key,
        uid,
        &ProfileInput {
            legal_first_name: first,
            legal_last_name: last,
            ..Default::default()
        },
    )
    .await;

    create_link(db, uid, identity).await?;
    audit(
        db,
        uid,
        crate::audit::actions::AUTH_OAUTH_SIGNUP,
        Some(tenant_id),
    )
    .await;
    Ok(user)
}

async fn create_link(
    db: &crate::db::RequestDb,
    user_id: Uuid,
    identity: &ExternalIdentity,
) -> ApiResult<()> {
    let now = Utc::now();
    entity::federated_identity::ActiveModel {
        id: Set(Uuid::new_v4()),
        user_id: Set(user_id),
        provider: Set(identity.provider.clone()),
        subject: Set(identity.subject.clone()),
        email: Set(identity.email.clone()),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    Ok(())
}

fn linked_resp(identity: &ExternalIdentity) -> OauthCallbackResp {
    OauthCallbackResp {
        outcome: "linked".into(),
        session: None,
        mfa: None,
        provider: Some(identity.provider.clone()),
        email: Some(identity.email.clone()),
    }
}

fn split_name(name: &str) -> (Option<String>, Option<String>) {
    let name = name.trim();
    match name.split_once(' ') {
        Some((first, rest)) => (
            Some(first.to_string()),
            Some(rest.trim().to_string()).filter(|s| !s.is_empty()),
        ),
        None if !name.is_empty() => (Some(name.to_string()), None),
        None => (None, None),
    }
}

async fn audit(db: &crate::db::RequestDb, user_id: Uuid, action: &str, tenant_id: Option<Uuid>) {
    crate::audit::record(
        db,
        Some(user_id),
        action,
        Some("user"),
        Some(user_id.to_string()),
        tenant_id,
        None,
    )
    .await;
}
