//! `POST /domains/<id>/verify-email` — check the SPF / DKIM / DMARC records
//! for a custom sending domain (issue #62), mirroring the routing
//! domain-verify flow: the API surfaces the records to publish
//! (`email_dns_records` on the domain) and this endpoint runs the check.
//!
//! Resolution goes through the sandbox-first DNS provider: simulated (all
//! records pass) unless `LIVE_PROVIDERS` lists `dns`, in which case TXT
//! records are resolved over DNS-over-HTTPS.

use super::dto::{email_dns_records, DomainResp};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::providers::dns::{DnsCheck, DnsProvider, DnsRequest};
use crate::providers::{Provider, ProviderCtx};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::Domain;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `POST /domains/<id>/verify-email` — verify SPF/DKIM/DMARC for branded mail.
#[rocket_okapi::openapi(tag = "Domains")]
#[post("/domains/<id>/verify-email")]
pub async fn verify_email(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<DomainResp>> {
    user.require(Permission::DomainManage)?;
    let did = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid domain id".into()))?;
    let domain = Domain::find()
        .filter(entity::domain::Column::Id.eq(did))
        .filter(entity::domain::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("domain not found".into()))?;
    if domain.kind != "custom" {
        return Err(ApiError::BadRequest(
            "platform subdomains already authenticate — email verification \
             applies to custom domains"
                .into(),
        ));
    }

    let records = email_dns_records(scope.tenant_id, &domain.hostname);
    let req = DnsRequest {
        checks: records
            .iter()
            .map(|r| DnsCheck {
                key: r.key.clone(),
                name: r.name.clone(),
                expect_contains: r.expect_contains.clone(),
            })
            .collect(),
    };
    let ctx = ProviderCtx::new(&db, scope.tenant_id);
    let resp = DnsProvider
        .execute(&ctx, &req)
        .await
        .map_err(|e| ApiError::BadRequest(format!("DNS check failed: {e}")))?;

    let mut status = serde_json::Map::new();
    for result in &resp.results {
        status.insert(result.key.clone(), serde_json::json!(result.found));
    }
    let all_pass = !resp.results.is_empty() && resp.results.iter().all(|r| r.found);

    let now = Utc::now();
    let mut am: entity::domain::ActiveModel = domain.into();
    am.email_dns_status = Set(serde_json::Value::Object(status.clone()));
    am.email_verified_at = Set(all_pass.then(|| now.into()));
    let saved = am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::DOMAIN_EMAIL_VERIFY,
        Some("domain"),
        Some(did.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({
            "hostname": saved.hostname,
            "results": status,
            "verified": all_pass,
        })),
    )
    .await;

    Ok(Json(DomainResp::from(saved)))
}
