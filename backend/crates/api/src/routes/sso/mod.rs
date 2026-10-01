//! **Alpha ↔ Vantedge single sign-on** over HTTP. A workspace turns it on
//! (a shared secret, shown once), pastes the secret into Alpha, and from then
//! on a person signed in on one side opens the other without a password. The
//! assertion rules are in [`crate::sso`].

use crate::audit::actions as act;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::routes::auth::dto::{MfaChallengeResp, OauthCallbackResp};
use crate::routes::auth::helpers::{auth_outcome, AuthOutcome};
use crate::sso::{self, ISS_ALPHA, ISS_VANTEDGE, SECRET_KEY};
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{Counterparty, Membership, SsoAssertion, Tenant, User};
use rocket::serde::json::Json;
use rocket::{delete, get, post, State};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

fn web_url() -> String {
    std::env::var("PUBLIC_WEB_URL")
        .ok()
        .map(|s| s.trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "http://localhost:3000".into())
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct SsoStatus {
    pub enabled: bool,
    /// What Alpha sets as `aud` when it signs a sign-in token for us.
    pub audience: String,
    /// What Alpha sets as `iss`.
    pub issuer: String,
    /// The workspace slug Alpha puts in the `tenant` claim.
    pub tenant: String,
    /// Where Alpha sends the browser (with `&token=…`).
    pub login_url: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct EnableResp {
    #[serde(flatten)]
    pub status: SsoStatus,
    /// Shown once. Paste it into Alpha's Vantedge connection.
    pub secret: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct EnableReq {
    /// Replace an existing secret (signs out nothing, but old tokens stop working).
    pub rotate: Option<bool>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct LaunchReq {
    pub counterparty_id: Uuid,
    /// The vendor's Alpha web address; remembered once given.
    pub web_url: Option<String>,
    /// A path on Alpha to land on.
    pub next: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct LaunchResp {
    pub url: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct AssertionReq {
    pub token: String,
}

async fn slug_of(db: &impl ConnectionTrait, tenant_id: Uuid) -> ApiResult<String> {
    Ok(Tenant::find_by_id(tenant_id)
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("workspace not found".into()))?
        .slug)
}

async fn status(db: &impl ConnectionTrait, tenant_id: Uuid) -> ApiResult<SsoStatus> {
    let enabled = crate::secrets::reveal(db, Some(tenant_id), SECRET_KEY)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!(e.to_string())))?
        .is_some();
    Ok(SsoStatus {
        enabled,
        audience: "vantedge".into(),
        issuer: ISS_ALPHA.into(),
        tenant: slug_of(db, tenant_id).await?,
        login_url: format!("{}/auth/callback?sso=alpha", web_url()),
    })
}

/// `GET /sso/alpha` — whether single sign-on with Alpha is on, and what Alpha needs.
#[rocket_okapi::openapi(tag = "SSO")]
#[get("/sso/alpha")]
pub async fn get_status(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<SsoStatus>> {
    user.require(Permission::IntegrationsManage)?;
    Ok(Json(status(&db, scope.tenant_id).await?))
}

/// `POST /sso/alpha/enable` — create (or rotate) the shared secret. The secret
/// is returned this once.
#[rocket_okapi::openapi(tag = "SSO")]
#[post("/sso/alpha/enable", data = "<body>")]
pub async fn enable(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<EnableReq>,
) -> ApiResult<Json<EnableResp>> {
    user.require(Permission::IntegrationsManage)?;
    let have = status(&db, scope.tenant_id).await?.enabled;
    if have && !body.rotate.unwrap_or(false) {
        return Err(ApiError::Conflict(
            "single sign-on is already on; rotate to get a new secret".into(),
        ));
    }
    let secret = format!(
        "ssosec_{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    );
    crate::secrets::store(
        &db,
        Some(scope.tenant_id),
        SECRET_KEY,
        &secret,
        Some(user.user_id),
    )
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!(e.to_string())))?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        if have {
            act::SSO_ROTATE
        } else {
            act::SSO_ENABLE
        },
        Some("sso"),
        Some("alpha".into()),
        Some(scope.tenant_id),
        None,
    )
    .await;
    Ok(Json(EnableResp {
        status: status(&db, scope.tenant_id).await?,
        secret,
    }))
}

/// `DELETE /sso/alpha` — turn single sign-on off.
#[rocket_okapi::openapi(tag = "SSO")]
#[delete("/sso/alpha")]
pub async fn disable(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<SsoStatus>> {
    user.require(Permission::IntegrationsManage)?;
    let removed = crate::secrets::remove(&db, Some(scope.tenant_id), SECRET_KEY)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!(e.to_string())))?;
    if removed {
        crate::audit::record(
            &db,
            Some(user.user_id),
            act::SSO_DISABLE,
            Some("sso"),
            Some("alpha".into()),
            Some(scope.tenant_id),
            None,
        )
        .await;
    }
    Ok(Json(status(&db, scope.tenant_id).await?))
}

/// `POST /sso/alpha/launch` — a one-time link that opens the vendor's Alpha,
/// signed in as the person clicking.
#[rocket_okapi::openapi(tag = "SSO")]
#[post("/sso/alpha/launch", data = "<body>")]
pub async fn launch(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<LaunchReq>,
) -> ApiResult<Json<LaunchResp>> {
    user.require(Permission::MaintenanceRead)?;
    let b = body.into_inner();
    let secret = crate::secrets::reveal(&db, Some(scope.tenant_id), SECRET_KEY)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!(e.to_string())))?
        .ok_or_else(|| ApiError::Conflict("single sign-on with Alpha is not turned on".into()))?;
    let c = Counterparty::find_by_id(b.counterparty_id)
        .filter(entity::counterparty::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("vendor not found".into()))?;
    if c.partner_kind.is_none() {
        return Err(ApiError::Conflict(
            "this vendor is not linked to Alpha".into(),
        ));
    }
    let given = b
        .web_url
        .as_deref()
        .map(|u| u.trim().trim_end_matches('/'))
        .filter(|u| !u.is_empty());
    if let Some(u) = given {
        if !(u.starts_with("https://") || u.starts_with("http://")) {
            return Err(ApiError::BadRequest(
                "web_url must start with https://".into(),
            ));
        }
    }
    let base = given
        .map(str::to_string)
        .or_else(|| c.partner_web_url.clone())
        .ok_or_else(|| ApiError::BadRequest("enter the vendor's Alpha web address first".into()))?;
    if given.is_some() && c.partner_web_url.as_deref() != Some(base.as_str()) {
        let mut am: entity::counterparty::ActiveModel = c.into();
        am.partner_web_url = Set(Some(base.clone()));
        am.update(&db).await?;
    }
    let me = User::find_by_id(user.user_id)
        .one(&db)
        .await?
        .ok_or(ApiError::Unauthorized)?;
    let slug = slug_of(&db, scope.tenant_id).await?;
    let claims = sso::claims(
        ISS_VANTEDGE,
        "alpha",
        &me.email,
        &slug,
        sso::safe_next(b.next.as_deref()),
        Utc::now().timestamp(),
        60,
    );
    let token = sso::sign(&secret, &claims).map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        act::SSO_LAUNCH,
        Some("sso"),
        Some("alpha".into()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "counterparty_id": b.counterparty_id })),
    )
    .await;
    let mut url = format!("{base}/sso/vantedge?token={token}");
    if let Some(n) = claims.next {
        url.push_str(&format!("&next={}", urlenc(&n)));
    }
    Ok(Json(LaunchResp { url }))
}

fn urlenc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// The one answer for every refusal, so nothing says which part was wrong.
fn refuse(why: &str) -> ApiError {
    tracing::warn!("sso refused: {why}");
    ApiError::Unauthorized
}

/// `POST /auth/sso/alpha` — complete a sign-in that Alpha started. The token
/// is Alpha's signed assertion; the answer is the same shape as a social login
/// (a session, or the two-step code prompt).
#[rocket_okapi::openapi(tag = "Auth")]
#[post("/auth/sso/alpha", data = "<body>")]
pub async fn sign_in(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    body: Json<AssertionReq>,
) -> ApiResult<Json<OauthCallbackResp>> {
    let token = body.token.trim();
    let slug = sso::peek_tenant(token).ok_or_else(|| refuse("no tenant claim"))?;
    let tenant = Tenant::find()
        .filter(entity::tenant::Column::Slug.eq(&slug))
        .one(&db)
        .await?
        .ok_or_else(|| refuse("unknown workspace"))?;
    let secret = crate::secrets::reveal(&db, Some(tenant.id), SECRET_KEY)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!(e.to_string())))?
        .ok_or_else(|| refuse("single sign-on is off for this workspace"))?;
    let claims = sso::verify(
        token,
        &secret,
        ISS_ALPHA,
        "vantedge",
        Utc::now().timestamp(),
    )
    .map_err(|e| refuse(&e))?;

    // Each assertion once. A second use hits the primary key.
    let fresh = entity::sso_assertion::ActiveModel {
        jti: Set(claims.jti.clone()),
        tenant_id: Set(Some(tenant.id)),
        issuer: Set(claims.iss.clone()),
        subject: Set(claims.sub.clone()),
        used_at: Set(Utc::now().into()),
    }
    .insert(&db)
    .await;
    if fresh.is_err() {
        return Err(refuse("token already used"));
    }
    // Old rows are only there to catch replays inside the lifetime.
    let _ = SsoAssertion::delete_many()
        .filter(
            entity::sso_assertion::Column::UsedAt.lt(Utc::now() - chrono::Duration::minutes(15)),
        )
        .exec(&db)
        .await;

    let user = User::find()
        .filter(entity::user::Column::Email.eq(&claims.sub))
        .one(&db)
        .await?
        .ok_or_else(|| refuse("no such person"))?;
    if user.status != "active" {
        return Err(refuse("account is not active"));
    }
    let belongs = user.tenant_id == Some(tenant.id)
        || Membership::find()
            .filter(entity::membership::Column::UserId.eq(user.id))
            .filter(entity::membership::Column::TenantId.eq(tenant.id))
            .filter(entity::membership::Column::Status.eq("active"))
            .limit(1)
            .one(&db)
            .await?
            .is_some();
    if !belongs {
        return Err(refuse("not a member of this workspace"));
    }

    let outcome = auth_outcome(state, &db, &user, Some(tenant.id)).await?;
    crate::audit::record(
        &db,
        Some(user.id),
        act::SSO_LOGIN,
        Some("user"),
        Some(user.id.to_string()),
        Some(tenant.id),
        Some(serde_json::json!({ "from": "alpha" })),
    )
    .await;
    Ok(Json(match outcome {
        AuthOutcome::Session(t) => OauthCallbackResp {
            outcome: "session".into(),
            session: Some(t),
            mfa: None,
            provider: None,
            email: None,
        },
        AuthOutcome::Mfa(mfa_token) => OauthCallbackResp {
            outcome: "mfa".into(),
            session: None,
            mfa: Some(MfaChallengeResp {
                mfa_required: true,
                mfa_token,
            }),
            provider: None,
            email: None,
        },
    }))
}
