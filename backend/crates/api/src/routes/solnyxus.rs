//! The **Solnyxus product link** — what the platform that hosts, monitors and
//! sells Vantedge calls on it. The contract is the platform's
//! (`daedalus-it/docs/platform/provisioning.md` §3, "The product's side");
//! Vantedge is a `tenant` product, so it answers `tenants` and not `provision`.
//!
//! Mounted at `/.well-known/solnyxus/` on the API root (outside the OpenAPI
//! document):
//! * `GET health` — public: status, product, version, commit, start time; with
//!   the platform key also the `checks` (a real `SELECT 1`). `503` + `down` when
//!   the database can't be reached. Called every minute, so it stays cheap.
//! * `GET version` — key: version, commit, build time, migrations.
//! * `POST tenants` — key: create a workspace, its owner and its branding
//!   through [`crate::provisioning`] (the same work as staff provisioning), and
//!   answer with a one-time set-password link. Idempotent by slug + owner email.
//!
//! The key is `Authorization: Bearer <SOLNYXUS_PLATFORM_KEY>`, compared in
//! constant time. It is not a JWT, so a request carrying it resolves no tenant;
//! `tenants` runs in its own transaction on the platform plane (no tenant GUC),
//! the same plane staff provisioning uses. Errors are the contract's
//! `{"error": "<code>", "detail": "…"}`, not the app's usual envelope.

use crate::auth::{hash_password, random_secret};
use crate::password_links::{self, PURPOSE_INVITE, PURPOSE_RESET};
use crate::provisioning::{self, NewWorkspace, Owner};
use crate::state::AppState;
use chrono::{DateTime, Duration, Utc};
use entity::prelude::{BusinessProfile, Tenant, Theme, User};
use migration::{Migrator, MigratorTrait};
use rocket::data::{Data, ToByteUnit};
use rocket::http::{ContentType, Status};
use rocket::request::{FromRequest, Outcome, Request};
use rocket::response::{self, Responder, Response};
use rocket::serde::json::Json;
use rocket::{get, post, State};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait,
    QueryFilter, Set, Statement, TransactionTrait,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;
use uuid::Uuid;

/// Where the routes are mounted.
pub const BASE: &str = "/.well-known/solnyxus";
/// The monitoring probe — kept out of rate limiting, the audit feed and the
/// request log like `/health`.
pub const HEALTH_PATH: &str = "/.well-known/solnyxus/health";

const PRODUCT: &str = "vantedge";
const VERSION: &str = env!("CARGO_PKG_VERSION");
/// The longest a set-password link handed to Solnyxus may live (the contract
/// says within 72 hours; resets are shorter still).
const LINK_HOURS: i64 = 72;
/// Largest `tenants` body read.
const MAX_BODY_KIB: u64 = 64;
/// How long the health probe waits on the database.
const DB_CHECK_SECS: u64 = 3;

static STARTED: OnceLock<DateTime<Utc>> = OnceLock::new();

/// Record when this process started serving (called while building the app).
pub fn mark_started() {
    STARTED.get_or_init(Utc::now);
}

fn started_at() -> String {
    STARTED.get_or_init(Utc::now).to_rfc3339()
}

fn env_value(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// The deployed commit: `APP_COMMIT` / `GIT_COMMIT` at run time, else stamped
/// at build time, else `unknown`.
fn commit() -> String {
    env_value("APP_COMMIT")
        .or_else(|| env_value("GIT_COMMIT"))
        .or_else(|| option_env!("APP_COMMIT").map(str::to_string))
        .or_else(|| option_env!("GIT_COMMIT").map(str::to_string))
        .unwrap_or_else(|| "unknown".into())
}

/// When the running binary was built: `APP_BUILT_AT` (run or build time), else
/// the executable's modification time.
fn built_at() -> Option<String> {
    env_value("APP_BUILT_AT")
        .or_else(|| option_env!("APP_BUILT_AT").map(str::to_string))
        .or_else(|| {
            let modified = std::env::current_exe()
                .and_then(std::fs::metadata)
                .and_then(|m| m.modified())
                .ok()?;
            Some(DateTime::<Utc>::from(modified).to_rfc3339())
        })
}

// ---------------------------------------------------------------------------
// Errors (the contract's shape)
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct LinkError {
    status: Status,
    code: &'static str,
    detail: String,
}

impl LinkError {
    fn new(status: Status, code: &'static str, detail: impl Into<String>) -> Self {
        LinkError {
            status,
            code,
            detail: detail.into(),
        }
    }
    fn unauthorized() -> Self {
        Self::new(
            Status::Unauthorized,
            "unauthorized",
            "send the platform key as Authorization: Bearer <key>",
        )
    }
    fn not_configured() -> Self {
        Self::new(
            Status::ServiceUnavailable,
            "not_configured",
            "SOLNYXUS_PLATFORM_KEY is not set on this deployment",
        )
    }
    fn invalid(detail: impl Into<String>) -> Self {
        Self::new(Status::BadRequest, "invalid", detail)
    }
    fn internal(e: impl std::fmt::Display) -> Self {
        // Logged in full; the caller only learns that it failed.
        tracing::error!("solnyxus link: {e}");
        crate::metrics::report_error(None, "solnyxus", &e.to_string());
        Self::new(
            Status::InternalServerError,
            "internal",
            "internal error — see the API log",
        )
    }
}

impl From<sea_orm::DbErr> for LinkError {
    fn from(e: sea_orm::DbErr) -> Self {
        LinkError::internal(e)
    }
}

impl From<crate::error::ApiError> for LinkError {
    fn from(e: crate::error::ApiError) -> Self {
        LinkError::internal(e)
    }
}

impl<'r> Responder<'r, 'static> for LinkError {
    fn respond_to(self, _req: &'r Request<'_>) -> response::Result<'static> {
        let body = json!({ "error": self.code, "detail": self.detail }).to_string();
        Response::build()
            .status(self.status)
            .header(ContentType::JSON)
            .sized_body(body.len(), std::io::Cursor::new(body))
            .ok()
    }
}

type LinkResult<T> = Result<T, LinkError>;

// ---------------------------------------------------------------------------
// Platform key
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCheck {
    Valid,
    /// No key, or the wrong one.
    Refused,
    /// This deployment has no `SOLNYXUS_PLATFORM_KEY`.
    NotConfigured,
}

/// Compare SHA-256 digests in constant time: neither the position of the first
/// differing byte nor the key's length shows in the timing.
fn same_key(presented: &str, configured: &str) -> bool {
    let a = Sha256::digest(presented.as_bytes());
    let b = Sha256::digest(configured.as_bytes());
    a.iter().zip(b.iter()).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

/// Pure decision behind [`PlatformKey`].
pub fn check_key(authorization: Option<&str>, configured: Option<&str>) -> KeyCheck {
    let Some(configured) = configured.map(str::trim).filter(|k| !k.is_empty()) else {
        return KeyCheck::NotConfigured;
    };
    let presented = authorization
        .and_then(|h| {
            h.strip_prefix("Bearer ")
                .or_else(|| h.strip_prefix("bearer "))
        })
        .map(str::trim)
        .unwrap_or("");
    if !presented.is_empty() && same_key(presented, configured) {
        KeyCheck::Valid
    } else {
        KeyCheck::Refused
    }
}

/// The outcome of checking the request's platform key. Always succeeds as a
/// guard so the handler can answer in the contract's error shape.
pub struct PlatformKey(KeyCheck);

impl PlatformKey {
    fn require(&self) -> LinkResult<()> {
        match self.0 {
            KeyCheck::Valid => Ok(()),
            KeyCheck::Refused => Err(LinkError::unauthorized()),
            KeyCheck::NotConfigured => Err(LinkError::not_configured()),
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for PlatformKey {
    type Error = ();
    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let configured = std::env::var("SOLNYXUS_PLATFORM_KEY").ok();
        Outcome::Success(PlatformKey(check_key(
            req.headers().get_one("Authorization"),
            configured.as_deref(),
        )))
    }
}

// ---------------------------------------------------------------------------
// GET health / GET version
// ---------------------------------------------------------------------------

/// `SELECT 1` through the pool, bounded so a hung database reads as down.
async fn database_check(db: &DatabaseConnection) -> (bool, String) {
    let probe = db.execute(Statement::from_string(DbBackend::Postgres, "SELECT 1"));
    match tokio::time::timeout(std::time::Duration::from_secs(DB_CHECK_SECS), probe).await {
        Ok(Ok(_)) => (true, String::new()),
        Ok(Err(e)) => (false, e.to_string()),
        Err(_) => (false, format!("no answer within {DB_CHECK_SECS}s")),
    }
}

/// `GET /.well-known/solnyxus/health`
#[get("/health")]
pub async fn health(state: &State<AppState>, key: PlatformKey) -> (Status, Json<Value>) {
    let (db_ok, db_detail) = database_check(&state.db).await;
    let mut body = json!({
        "status": if db_ok { "ok" } else { "down" },
        "product": PRODUCT,
        "version": VERSION,
        "commit": commit(),
        "startedAt": started_at(),
    });
    // The checks (and any error text) only for the platform.
    if key.0 == KeyCheck::Valid {
        body["checks"] = json!([{ "name": "database", "ok": db_ok, "detail": db_detail }]);
    }
    let status = if db_ok {
        Status::Ok
    } else {
        Status::ServiceUnavailable
    };
    (status, Json(body))
}

/// The newest applied migration and how many known ones aren't applied yet;
/// `null` when the database can't say.
async fn migrations(db: &DatabaseConnection) -> Value {
    let rows = db
        .query_all(Statement::from_string(
            DbBackend::Postgres,
            "SELECT version FROM seaql_migrations",
        ))
        .await;
    let Ok(rows) = rows else {
        return Value::Null;
    };
    let applied: std::collections::HashSet<String> = rows
        .iter()
        .filter_map(|r| r.try_get::<String>("", "version").ok())
        .collect();
    let pending = Migrator::migrations()
        .iter()
        .filter(|m| !applied.contains(m.name()))
        .count();
    json!({ "latest": applied.iter().max(), "pending": pending })
}

/// `GET /.well-known/solnyxus/version`
#[get("/version")]
pub async fn version(state: &State<AppState>, key: PlatformKey) -> LinkResult<Json<Value>> {
    key.require()?;
    Ok(Json(json!({
        "product": PRODUCT,
        "version": VERSION,
        "commit": commit(),
        "builtAt": built_at(),
        "migrations": migrations(&state.db).await,
    })))
}

// ---------------------------------------------------------------------------
// POST tenants
// ---------------------------------------------------------------------------

/// The branding Solnyxus collects at sign-up. Empty fields are left as they
/// are.
#[derive(Debug, Default, PartialEq)]
pub struct Branding {
    company_name: Option<String>,
    tagline: Option<String>,
    primary_color: Option<String>,
    accent_color: Option<String>,
    logo_url: Option<String>,
    email: Option<String>,
    phone: Option<String>,
    website: Option<String>,
}

#[derive(Debug, PartialEq)]
pub struct TenantReq {
    slug: String,
    name: String,
    plan: Option<String>,
    owner_email: String,
    owner_name: Option<String>,
    branding: Branding,
}

fn at<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(v, |v, k| v.get(k))
}

/// An optional text field: absent, `null` and blank all read as `None`.
fn text(v: &Value, path: &str, max: usize) -> LinkResult<Option<String>> {
    match at(v, path) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => {
            let s = s.trim();
            if s.chars().count() > max {
                return Err(LinkError::invalid(format!(
                    "{path} is longer than {max} characters"
                )));
            }
            Ok(Some(s.to_string()).filter(|s| !s.is_empty()))
        }
        Some(_) => Err(LinkError::invalid(format!("{path} must be a string"))),
    }
}

fn required(v: &Value, path: &str, max: usize) -> LinkResult<String> {
    text(v, path, max)?.ok_or_else(|| LinkError::invalid(format!("{path} is required")))
}

fn object_or_null(v: &Value, path: &str) -> LinkResult<()> {
    match at(v, path) {
        None | Some(Value::Null) | Some(Value::Object(_)) => Ok(()),
        Some(_) => Err(LinkError::invalid(format!("{path} must be an object"))),
    }
}

fn is_email(s: &str) -> bool {
    let Some((local, domain)) = s.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.contains('@')
        && !s.chars().any(char::is_whitespace)
}

fn is_hex_color(s: &str) -> bool {
    s.strip_prefix('#')
        .is_some_and(|h| (h.len() == 3 || h.len() == 6) && h.chars().all(|c| c.is_ascii_hexdigit()))
}

fn is_http_url(s: &str) -> bool {
    (s.starts_with("https://") || s.starts_with("http://")) && !s.contains(char::is_whitespace)
}

fn check(ok: bool, path: &str, what: &str) -> LinkResult<()> {
    if ok {
        Ok(())
    } else {
        Err(LinkError::invalid(format!("{path} must be {what}")))
    }
}

impl TenantReq {
    /// Read and validate the body; nothing is written until all of it passes.
    pub fn parse(raw: &str) -> LinkResult<Self> {
        let v: Value = serde_json::from_str(raw)
            .map_err(|e| LinkError::invalid(format!("body is not valid JSON: {e}")))?;
        if !v.is_object() {
            return Err(LinkError::invalid("body must be a JSON object"));
        }
        let slug = provisioning::normalize_slug(&required(&v, "slug", 63)?)
            .map_err(|e| LinkError::invalid(format!("slug: {e}")))?;
        let name = required(&v, "name", 200)?;
        let plan = text(&v, "plan", 40)?;
        if !matches!(at(&v, "owner"), Some(Value::Object(_))) {
            return Err(LinkError::invalid("owner is required"));
        }
        let owner_email = required(&v, "owner.email", 254)?.to_lowercase();
        check(is_email(&owner_email), "owner.email", "an email address")?;
        let owner_name = text(&v, "owner.name", 200)?;

        object_or_null(&v, "branding")?;
        let branding = Branding {
            company_name: text(&v, "branding.companyName", 200)?,
            tagline: text(&v, "branding.tagline", 300)?,
            primary_color: text(&v, "branding.primaryColor", 7)?,
            accent_color: text(&v, "branding.accentColor", 7)?,
            logo_url: text(&v, "branding.logoUrl", 2048)?,
            email: text(&v, "branding.email", 254)?,
            phone: text(&v, "branding.phone", 40)?,
            website: text(&v, "branding.website", 2048)?,
        };
        let b = &branding;
        for (path, value) in [
            ("branding.primaryColor", &b.primary_color),
            ("branding.accentColor", &b.accent_color),
        ] {
            if let Some(c) = value {
                check(is_hex_color(c), path, "a hex colour like #0F766E")?;
            }
        }
        for (path, value) in [
            ("branding.logoUrl", &b.logo_url),
            ("branding.website", &b.website),
        ] {
            if let Some(u) = value {
                check(is_http_url(u), path, "an http(s) URL")?;
            }
        }
        if let Some(e) = &b.email {
            check(is_email(e), "branding.email", "an email address")?;
        }

        Ok(TenantReq {
            slug,
            name,
            plan,
            owner_email,
            owner_name,
            branding,
        })
    }
}

/// Where the owner signs in. Sign-in is by email (the workspace comes from the
/// membership), so it is the app's one sign-in page until per-workspace hosts
/// exist.
fn login_url() -> String {
    format!("{}/login", crate::oauth::public_app_url())
}

/// A fresh one-time link to the app's set-password page: an invite while the
/// owner has never chosen a password, otherwise a reset. Either is capped at
/// [`LINK_HOURS`] and retires the previous link of its kind.
async fn set_password_url(
    db: &impl ConnectionTrait,
    user: &entity::user::Model,
) -> Result<String, sea_orm::DbErr> {
    let purpose = if user.status == "invited" {
        PURPOSE_INVITE
    } else {
        PURPOSE_RESET
    };
    let (token, _) =
        password_links::issue_valid_for(db, user.id, purpose, Duration::hours(LINK_HOURS)).await?;
    Ok(password_links::link_url(&token))
}

/// Put the sign-up branding where the workspace keeps it:
/// * `companyName`, `logoUrl`, `primaryColor`, `accentColor` → the theme (the
///   console and public site's brand);
/// * `companyName`, `email`, `phone`, `website` → the business profile (the
///   public contact details), and `tagline` → its description, the short
///   public blurb — the theme has no tagline of its own.
///
/// Every field has a home; nothing is dropped.
async fn apply_branding(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    b: &Branding,
) -> Result<(), sea_orm::DbErr> {
    let now = Utc::now();
    if let Some(theme) = Theme::find()
        .filter(entity::theme::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
    {
        let mut am: entity::theme::ActiveModel = theme.into();
        if let Some(v) = &b.company_name {
            am.company_name = Set(v.clone());
        }
        if let Some(v) = &b.logo_url {
            am.logo_url = Set(Some(v.clone()));
        }
        if let Some(v) = &b.primary_color {
            am.primary_color = Set(v.clone());
        }
        if let Some(v) = &b.accent_color {
            am.accent_color = Set(v.clone());
        }
        am.updated_at = Set(now.into());
        am.update(db).await?;
    }

    let public = [&b.company_name, &b.email, &b.phone, &b.website, &b.tagline];
    if public.iter().all(|v| v.is_none()) {
        return Ok(());
    }
    let existing = BusinessProfile::find()
        .filter(entity::business_profile::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?;
    let (mut am, is_new): (entity::business_profile::ActiveModel, bool) = match existing {
        Some(p) => (p.into(), false),
        // Review, embed and refresh settings keep their column defaults.
        None => (
            entity::business_profile::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                ..Default::default()
            },
            true,
        ),
    };
    if let Some(v) = &b.company_name {
        am.business_name = Set(Some(v.clone()));
    }
    if let Some(v) = &b.email {
        am.email = Set(Some(v.clone()));
    }
    if let Some(v) = &b.phone {
        am.phone = Set(Some(v.clone()));
    }
    if let Some(v) = &b.website {
        am.website = Set(Some(v.clone()));
    }
    if let Some(v) = &b.tagline {
        am.description = Set(Some(v.clone()));
    }
    am.updated_at = Set(now.into());
    if is_new {
        am.insert(db).await?;
    } else {
        am.update(db).await?;
    }
    Ok(())
}

/// The body of `POST tenants`, run inside one transaction.
async fn provision_tenant(
    db: &impl ConnectionTrait,
    req: TenantReq,
) -> LinkResult<(Status, Json<Value>)> {
    let answer = |status: Status, tenant_id: Uuid, owner_id: Uuid, created: bool, url: String| {
        (
            status,
            Json(json!({
                "tenantId": tenant_id,
                "slug": req.slug,
                "ownerId": owner_id,
                "created": created,
                "setPasswordUrl": url,
                "loginUrl": login_url(),
            })),
        )
    };
    let account = User::find()
        .filter(entity::user::Column::Email.eq(req.owner_email.clone()))
        .one(db)
        .await?;

    // Asked before: the same slug with the same owner is a repeat, answered
    // with the existing ids and a fresh link; anyone else's slug is taken.
    if let Some(tenant) = Tenant::find()
        .filter(entity::tenant::Column::Slug.eq(req.slug.clone()))
        .one(db)
        .await?
    {
        if let Some(user) = account {
            if provisioning::is_owner(db, tenant.id, user.id).await? {
                let url = set_password_url(db, &user).await?;
                return Ok(answer(Status::Ok, tenant.id, user.id, false, url));
            }
        }
        return Err(LinkError::new(
            Status::Conflict,
            "slug_taken",
            format!("the slug '{}' belongs to a different owner", req.slug),
        ));
    }

    let owner = match account {
        Some(u) if u.status == "suspended" || u.status == "disabled" => {
            return Err(LinkError::invalid(format!(
                "owner.email: that account is {}",
                u.status
            )));
        }
        // Already has a login (e.g. owns another workspace): add this one.
        Some(u) => Owner::Existing(u),
        // A new login nobody can use until the owner chooses a password from
        // the link — no password crosses the wire.
        None => Owner::New {
            email: req.owner_email.clone(),
            name: req.owner_name.clone().unwrap_or_else(|| req.name.clone()),
            password_hash: hash_password(&random_secret(24)).map_err(LinkError::internal)?,
            status: "invited",
        },
    };
    let made = provisioning::create_workspace(
        db,
        NewWorkspace {
            slug: req.slug.clone(),
            name: req.name.clone(),
            plan: req.plan.clone(),
            // Solnyxus has already approved the sign-up; the workspace is live.
            status: "active",
            owner,
        },
    )
    .await?;
    apply_branding(db, made.tenant_id, &req.branding).await?;

    crate::audit::record(
        db,
        None,
        crate::audit::actions::TENANT_PROVISION,
        Some("tenant"),
        Some(made.tenant_id.to_string()),
        Some(made.tenant_id),
        Some(json!({
            "slug": req.slug,
            "owner_email": req.owner_email,
            "plan": made.plan,
            "via": "solnyxus",
        })),
    )
    .await;

    let user = User::find_by_id(made.owner_user_id)
        .one(db)
        .await?
        .ok_or_else(|| LinkError::internal("the new owner vanished"))?;
    let url = set_password_url(db, &user).await?;
    Ok(answer(
        Status::Created,
        made.tenant_id,
        made.owner_user_id,
        true,
        url,
    ))
}

/// `POST /.well-known/solnyxus/tenants`
#[post("/tenants", data = "<data>")]
pub async fn tenants(
    state: &State<AppState>,
    key: PlatformKey,
    data: Data<'_>,
) -> LinkResult<(Status, Json<Value>)> {
    key.require()?;
    let body = data
        .open(MAX_BODY_KIB.kibibytes())
        .into_string()
        .await
        .map_err(|e| LinkError::invalid(format!("body could not be read: {e}")))?;
    if !body.is_complete() {
        return Err(LinkError::invalid(format!(
            "body is larger than {MAX_BODY_KIB} KiB"
        )));
    }
    let req = TenantReq::parse(&body)?;

    // Its own transaction on the platform plane: everything or nothing.
    let txn = state.db.begin().await?;
    let out = provision_tenant(&txn, req).await?;
    txn.commit().await?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_checks() {
        assert_eq!(check_key(None, None), KeyCheck::NotConfigured);
        assert_eq!(
            check_key(Some("Bearer k"), Some("  ")),
            KeyCheck::NotConfigured
        );
        assert_eq!(check_key(None, Some("secret")), KeyCheck::Refused);
        assert_eq!(
            check_key(Some("Bearer "), Some("secret")),
            KeyCheck::Refused
        );
        assert_eq!(check_key(Some("secret"), Some("secret")), KeyCheck::Refused);
        assert_eq!(
            check_key(Some("Bearer secreT"), Some("secret")),
            KeyCheck::Refused
        );
        assert_eq!(
            check_key(Some("Bearer secret"), Some("secret")),
            KeyCheck::Valid
        );
        assert_eq!(
            check_key(Some("bearer secret"), Some("secret\n")),
            KeyCheck::Valid
        );
    }

    #[test]
    fn parses_a_full_request() {
        let r = TenantReq::parse(
            r##"{"slug":" Acme ","name":"Acme Property Group","plan":"growth",
                "owner":{"email":"Dana@Acme.org","name":"Dana Ortiz"},
                "branding":{"companyName":"Acme","primaryColor":"#0F766E","accentColor":"",
                            "logoUrl":"https://cdn.example/logo.png","website":"https://acme.org",
                            "email":"hello@acme.org","phone":"+1 555 0100","tagline":"Clean"}}"##,
        )
        .unwrap();
        assert_eq!(r.slug, "acme");
        assert_eq!(r.owner_email, "dana@acme.org");
        assert_eq!(r.plan.as_deref(), Some("growth"));
        assert_eq!(r.branding.primary_color.as_deref(), Some("#0F766E"));
        // Empty fields are left as they are.
        assert_eq!(r.branding.accent_color, None);
    }

    #[test]
    fn names_the_bad_field() {
        let detail = |raw: &str| TenantReq::parse(raw).unwrap_err().detail;
        let owner = r#""owner":{"email":"a@b.co"}"#;
        assert!(detail("[]").contains("JSON object"));
        assert!(detail("{").contains("valid JSON"));
        assert!(detail(&format!(r#"{{"name":"A",{owner}}}"#)).contains("slug"));
        assert!(detail(&format!(r#"{{"slug":"a b","name":"A",{owner}}}"#)).contains("slug"));
        assert!(detail(r#"{"slug":"a","name":"A"}"#).contains("owner"));
        assert!(
            detail(r#"{"slug":"a","name":"A","owner":{"email":"nope"}}"#).contains("owner.email")
        );
        assert!(detail(r#"{"slug":"a","name":7,"owner":{"email":"a@b.co"}}"#).contains("name"));
        assert!(detail(&format!(
            r#"{{"slug":"a","name":"A",{owner},"branding":{{"primaryColor":"teal"}}}}"#
        ))
        .contains("branding.primaryColor"));
        assert!(detail(&format!(
            r#"{{"slug":"a","name":"A",{owner},"branding":{{"logoUrl":"ftp://x"}}}}"#
        ))
        .contains("branding.logoUrl"));
        assert!(detail(&format!(
            r#"{{"slug":"a","name":"A",{owner},"branding":3}}"#
        ))
        .contains("branding"));
    }

    #[test]
    fn colours_and_urls() {
        assert!(is_hex_color("#0F766E"));
        assert!(is_hex_color("#fff"));
        assert!(!is_hex_color("0F766E"));
        assert!(!is_hex_color("#0F766"));
        assert!(is_http_url("https://acme.org"));
        assert!(!is_http_url("javascript:alert(1)"));
    }
}
