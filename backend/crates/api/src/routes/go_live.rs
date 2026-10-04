//! **Go live**: one page that says, provider by provider, whether this
//! workspace is ready to stop simulating, plus the platform checks a deploy
//! needs (production mode, public addresses, backups and the last restore
//! drill).
//!
//! `GET /go-live` (`integrations:manage`). Each provider row reports whether
//! the live switch is on (`LIVE_PROVIDERS`), which credentials are in the
//! vault (names only; never values), the last real call and failures in the
//! last week (from the `provider.call` audit), and for providers that call
//! back, when the last signed webhook arrived. Nothing here calls out.

use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::tenancy::TenantScope;
use chrono::{Duration, Utc};
use entity::prelude::{AuditLog, BackupRun};
use rocket::get;
use rocket::serde::json::Json;
use schemars::JsonSchema;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Serialize;
use uuid::Uuid;

/// What a provider needs before it can go live.
pub struct ProviderSpec {
    pub key: &'static str,
    pub label: &'static str,
    pub what: &'static str,
    /// Vault keys it reads; `|` separates alternatives (any one will do).
    pub secrets: &'static [&'static str],
    /// An environment variable that can stand in for the secrets.
    pub env: Option<&'static str>,
    /// Notification channel whose default provider must exist (email, sms).
    pub channel: Option<&'static str>,
    /// The provider name it signs webhooks with, when it calls back.
    pub webhook: Option<&'static str>,
    /// Where in the console it's set up.
    pub href: &'static str,
}

pub const PROVIDERS: &[ProviderSpec] = &[
    ProviderSpec {
        key: "email",
        label: "Email",
        what: "Receipts, notices, invites and reminders.",
        secrets: &[],
        env: None,
        channel: Some("email"),
        webhook: None,
        href: "/console/notifications",
    },
    ProviderSpec {
        key: "sms",
        label: "Texts (Twilio)",
        what: "Two-way texts, reminders and missed-call text-back.",
        secrets: &[],
        env: None,
        channel: Some("sms"),
        webhook: Some("twilio"),
        href: "/console/texts",
    },
    ProviderSpec {
        key: "stripe",
        label: "Payments (Stripe)",
        what: "Rent, fees and deposits online.",
        secrets: &["stripe.secret_key"],
        env: None,
        channel: None,
        webhook: Some("stripe"),
        href: "/console/settings",
    },
    ProviderSpec {
        key: "plaid",
        label: "Bank feeds (Plaid)",
        what: "Bank transactions matched to payments.",
        secrets: &["plaid.client_id", "plaid.secret"],
        env: None,
        channel: None,
        webhook: None,
        href: "/console/settings",
    },
    ProviderSpec {
        key: "checkr",
        label: "Screening (Checkr)",
        what: "Background and credit checks on applicants.",
        secrets: &["checkr.api_key"],
        env: None,
        channel: None,
        webhook: Some("checkr"),
        href: "/console/settings",
    },
    ProviderSpec {
        key: "gusto",
        label: "Payroll (Gusto)",
        what: "Hours pushed to payroll.",
        secrets: &["gusto.access_token"],
        env: None,
        channel: None,
        webhook: None,
        href: "/console/back-office?tab=payroll",
    },
    ProviderSpec {
        key: "maps",
        label: "Google Maps",
        what: "Address search, street photos and reviews.",
        secrets: &["google.maps_api_key"],
        env: None,
        channel: None,
        webhook: None,
        href: "/console/integrations",
    },
    ProviderSpec {
        key: "fbi",
        label: "Crime data (FBI)",
        what: "Safety panel on each property.",
        secrets: &["fbi.api_key"],
        env: Some("FBI_CDE_API_KEY"),
        channel: None,
        webhook: None,
        href: "/console/settings",
    },
    ProviderSpec {
        key: "rentcast",
        label: "Property records (RentCast)",
        what: "Parcel, taxes and value.",
        secrets: &["rentcast.api_key"],
        env: Some("RENTCAST_API_KEY"),
        channel: None,
        webhook: None,
        href: "/console/settings",
    },
];

/// Where a provider stands.
#[derive(Serialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Readiness {
    /// Live, credentials in place, and the last real call worked.
    Ready,
    /// Live and set up, but no real call has gone through yet.
    Untested,
    /// Live, but the last real call failed.
    Failing,
    /// Live, but something it needs is missing.
    Missing,
    /// Still simulated.
    Simulated,
}

/// The pure rule, so it can be tested.
pub fn readiness(live: bool, missing: usize, last_ok: Option<bool>) -> Readiness {
    match (live, missing, last_ok) {
        (false, _, _) => Readiness::Simulated,
        (true, m, _) if m > 0 => Readiness::Missing,
        (true, _, Some(true)) => Readiness::Ready,
        (true, _, Some(false)) => Readiness::Failing,
        (true, _, None) => Readiness::Untested,
    }
}

#[derive(Serialize, JsonSchema)]
pub struct Requirement {
    pub label: String,
    pub present: bool,
}

#[derive(Serialize, JsonSchema)]
pub struct ProviderCheck {
    pub key: String,
    pub label: String,
    pub what: String,
    pub href: String,
    pub live: bool,
    pub requirements: Vec<Requirement>,
    /// The last real (live) call.
    pub last_call_at: Option<String>,
    pub last_call_ok: Option<bool>,
    pub last_error: Option<String>,
    pub failures_7d: usize,
    /// Only for providers that call back.
    pub webhook_expected: bool,
    pub last_webhook_at: Option<String>,
    pub readiness: Readiness,
}

#[derive(Serialize, JsonSchema)]
pub struct PlatformCheck {
    pub key: String,
    pub label: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Serialize, JsonSchema)]
pub struct RunDto {
    pub started_at: String,
    pub finished_at: Option<String>,
    pub ok: bool,
    pub bytes: Option<i64>,
    pub location: Option<String>,
    pub detail: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct GoLiveResp {
    pub providers: Vec<ProviderCheck>,
    pub platform: Vec<PlatformCheck>,
    pub last_backup: Option<RunDto>,
    pub last_good_backup_at: Option<String>,
    pub last_drill: Option<RunDto>,
    /// `LIVE_PROVIDERS` as the server sees it.
    pub live_providers: String,
}

async fn has_secret(db: &impl ConnectionTrait, tenant_id: Uuid, key: &str) -> bool {
    crate::secrets::reveal(db, Some(tenant_id), key)
        .await
        .ok()
        .flatten()
        .is_some_and(|v| !v.trim().is_empty())
}

fn run_dto(r: entity::backup_run::Model) -> RunDto {
    RunDto {
        started_at: r.started_at.to_rfc3339(),
        finished_at: r.finished_at.map(|d| d.to_rfc3339()),
        ok: r.ok,
        bytes: r.bytes,
        location: r.location,
        detail: r.detail,
    }
}

async fn last_run(
    db: &impl ConnectionTrait,
    kind: &str,
    only_ok: bool,
) -> Result<Option<entity::backup_run::Model>, sea_orm::DbErr> {
    let mut q = BackupRun::find().filter(entity::backup_run::Column::Kind.eq(kind));
    if only_ok {
        q = q.filter(entity::backup_run::Column::Ok.eq(true));
    }
    q.order_by_desc(entity::backup_run::Column::StartedAt)
        .one(db)
        .await
}

/// `GET /go-live` — provider by provider, is this workspace ready to go live?
#[rocket_okapi::openapi(tag = "Integrations")]
#[get("/go-live")]
pub async fn go_live(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<GoLiveResp>> {
    user.require(Permission::IntegrationsManage)?;
    let t = scope.tenant_id;
    let since = Utc::now() - Duration::days(30);
    let week = Utc::now() - Duration::days(7);
    let audits = AuditLog::find()
        .filter(entity::audit_log::Column::TenantId.eq(t))
        .filter(entity::audit_log::Column::Action.is_in([
            crate::audit::actions::PROVIDER_CALL,
            crate::audit::actions::WEBHOOK_RECEIVED,
            crate::audit::actions::SMS_RECEIVE,
        ]))
        .filter(entity::audit_log::Column::CreatedAt.gte(since))
        .order_by_desc(entity::audit_log::Column::CreatedAt)
        .limit(2000)
        .all(&db)
        .await?;
    let meta = |a: &entity::audit_log::Model, k: &str| -> Option<String> {
        a.metadata
            .as_ref()
            .and_then(|m| m.get(k))
            .and_then(|v| v.as_str())
            .map(str::to_string)
    };

    let mut providers = Vec::new();
    for spec in PROVIDERS {
        let live = crate::providers::is_live(spec.key);
        let mut reqs = Vec::new();
        let env_ok = spec
            .env
            .and_then(|e| std::env::var(e).ok())
            .is_some_and(|v| !v.trim().is_empty());
        for s in spec.secrets {
            let present = env_ok || has_secret(&db, t, s).await;
            reqs.push(Requirement {
                label: format!("{s} in the vault"),
                present,
            });
        }
        if let Some(ch) = spec.channel {
            let p = crate::notify::default_provider(&db, t, ch).await;
            let present = p.as_ref().is_some_and(|p| p.enabled);
            reqs.push(Requirement {
                label: format!("A default {ch} provider, turned on"),
                present,
            });
            if let Some(key) = p.and_then(|p| p.secret_ref) {
                reqs.push(Requirement {
                    present: has_secret(&db, t, &key).await,
                    label: format!("{key} in the vault"),
                });
            }
        }
        if let Some(w) = spec.webhook {
            if w != "twilio" {
                let k = crate::providers::webhook::secret_key_name(w);
                reqs.push(Requirement {
                    present: has_secret(&db, t, &k).await,
                    label: format!("{k} (webhook signing secret)"),
                });
            }
        }
        // Only real calls count; simulated ones are audited with live=false
        // (and older entries carry no flag).
        let calls: Vec<&entity::audit_log::Model> = audits
            .iter()
            .filter(|a| a.action == crate::audit::actions::PROVIDER_CALL)
            .filter(|a| meta(a, "provider").as_deref() == Some(spec.key))
            .filter(|a| {
                a.metadata
                    .as_ref()
                    .and_then(|m| m.get("live"))
                    .and_then(|v| v.as_bool())
                    == Some(true)
            })
            .collect();
        let last = calls.first();
        let last_ok = last.map(|a| meta(a, "status").as_deref() == Some("succeeded"));
        let failures_7d = calls
            .iter()
            .filter(|a| a.created_at.with_timezone(&Utc) >= week)
            .filter(|a| meta(a, "status").as_deref() == Some("failed"))
            .count();
        let last_webhook_at = spec.webhook.and_then(|w| {
            audits
                .iter()
                .find(|a| {
                    if w == "twilio" {
                        a.action == crate::audit::actions::SMS_RECEIVE
                    } else {
                        a.action == crate::audit::actions::WEBHOOK_RECEIVED
                            && meta(a, "provider").as_deref() == Some(w)
                    }
                })
                .map(|a| a.created_at.to_rfc3339())
        });
        let missing = reqs.iter().filter(|r| !r.present).count();
        providers.push(ProviderCheck {
            key: spec.key.into(),
            label: spec.label.into(),
            what: spec.what.into(),
            href: spec.href.into(),
            live,
            last_call_at: last.map(|a| a.created_at.to_rfc3339()),
            last_error: last.and_then(|a| meta(a, "error")),
            last_call_ok: last_ok,
            failures_7d,
            webhook_expected: spec.webhook.is_some(),
            last_webhook_at,
            readiness: readiness(live, missing, last_ok),
            requirements: reqs,
        });
    }

    let api_url = crate::oauth::public_api_url();
    let app_url = std::env::var("PUBLIC_APP_URL").unwrap_or_default();
    let last_backup = last_run(&db, "backup", false).await?;
    let last_good = last_run(&db, "backup", true).await?;
    let last_drill = last_run(&db, "restore_drill", false).await?;
    let fresh = last_good
        .as_ref()
        .is_some_and(|b| b.started_at.with_timezone(&Utc) > Utc::now() - Duration::hours(36));
    let drilled = last_drill.as_ref().is_some_and(|d| {
        d.ok && d.started_at.with_timezone(&Utc) > Utc::now() - Duration::days(90)
    });
    let platform = vec![
        PlatformCheck {
            key: "production".into(),
            label: "Production mode".into(),
            ok: crate::config::is_production(),
            detail: "APP_ENV=production turns on fail-closed keys and turns off auto-migrate.".into(),
        },
        PlatformCheck {
            key: "api_url".into(),
            label: "Public API address".into(),
            ok: api_url.starts_with("https://"),
            detail: format!("PUBLIC_API_URL is {api_url}. Webhook signatures are checked against it, so it must be the real https address."),
        },
        PlatformCheck {
            key: "app_url".into(),
            label: "Public app address".into(),
            ok: app_url.starts_with("https://"),
            detail: if app_url.is_empty() {
                "PUBLIC_APP_URL isn't set; links in emails and texts need it.".into()
            } else {
                format!("PUBLIC_APP_URL is {app_url}.")
            },
        },
        PlatformCheck {
            key: "backup".into(),
            label: "Backup in the last day and a half".into(),
            ok: fresh,
            detail: "backend/deploy/backup.sh, nightly, encrypted with age.".into(),
        },
        PlatformCheck {
            key: "drill".into(),
            label: "Restore drill in the last 90 days".into(),
            ok: drilled,
            detail: "backend/deploy/restore-drill.sh restores the newest backup into a scratch database and times it.".into(),
        },
    ];
    Ok(Json(GoLiveResp {
        providers,
        platform,
        last_good_backup_at: last_good.map(|b| b.started_at.to_rfc3339()),
        last_backup: last_backup.map(run_dto),
        last_drill: last_drill.map(run_dto),
        live_providers: std::env::var("LIVE_PROVIDERS").unwrap_or_default(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readiness_rules() {
        assert_eq!(readiness(false, 3, None), Readiness::Simulated);
        assert_eq!(readiness(true, 1, Some(true)), Readiness::Missing);
        assert_eq!(readiness(true, 0, Some(true)), Readiness::Ready);
        assert_eq!(readiness(true, 0, Some(false)), Readiness::Failing);
        assert_eq!(readiness(true, 0, None), Readiness::Untested);
    }

    #[test]
    fn every_provider_says_where_to_set_it_up() {
        for p in PROVIDERS {
            assert!(p.href.starts_with("/console/"), "{}", p.key);
            assert!(!p.label.is_empty() && !p.what.is_empty());
        }
    }
}
