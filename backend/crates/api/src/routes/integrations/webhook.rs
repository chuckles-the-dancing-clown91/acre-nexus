//! `POST /webhooks/<provider>` — inbound webhook ingestion (issue #16).
//!
//! The tenant is resolved like any public request (`X-Tenant` header or
//! `?tenant=<slug>` — providers are configured with the full URL). The raw
//! body's HMAC-SHA256 signature (`X-Acre-Signature: sha256=<hex>`) is verified
//! constant-time against the signing secret stored under
//! `webhook.<provider>.secret` in the secrets store (tenant row, falling back
//! to the platform-wide row). A verified event is **enqueued as a
//! `webhook_event` background job** rather than handled synchronously, so
//! inbound events get the same durability/retry/audit trail as everything
//! else.

use crate::error::{ApiError, ApiResult};
use crate::providers::webhook as sigcheck;
use crate::scheduler;
use crate::state::AppState;
use crate::tenancy::PublicTenant;
use rocket::request::{FromRequest, Outcome, Request};
use rocket::serde::json::Json;
use rocket::{post, State};

/// The presented `X-Acre-Signature` header, if any.
pub struct WebhookSignature(pub Option<String>);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for WebhookSignature {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        Outcome::Success(WebhookSignature(
            req.headers()
                .get_one("X-Acre-Signature")
                .map(str::to_string),
        ))
    }
}

/// `POST /webhooks/<provider>` — verify and enqueue one inbound event.
/// Undocumented in OpenAPI on purpose: callers are provider dashboards, not
/// API consumers (contract in `docs/INTEGRATIONS.md`).
#[rocket_okapi::openapi(skip)]
#[post("/webhooks/<provider>", data = "<body>")]
pub async fn receive(
    state: &State<AppState>,
    tenant: PublicTenant,
    signature: WebhookSignature,
    provider: &str,
    body: String,
) -> ApiResult<Json<serde_json::Value>> {
    crate::modules::require_enabled(&state.db, tenant.tenant_id, "integrations").await?;

    let provider = provider.trim().to_lowercase();
    if provider.is_empty()
        || !provider
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(ApiError::BadRequest("invalid provider key".into()));
    }

    // No configured signing secret means this endpoint is simply not open for
    // that provider — indistinguishable from a bad signature on purpose.
    let secret = crate::secrets::reveal(
        &state.db,
        Some(tenant.tenant_id),
        &sigcheck::secret_key_name(&provider),
    )
    .await?;
    let verified = match (&secret, &signature.0) {
        (Some(secret), Some(header)) => sigcheck::verify(secret, body.as_bytes(), header),
        _ => false,
    };
    if !verified {
        tracing::warn!(provider = %provider, tenant = %tenant.tenant_id, "webhook signature rejected");
        return Err(ApiError::Unauthorized);
    }

    // Store the event as JSON when it parses, else as a raw string.
    let event: serde_json::Value =
        serde_json::from_str(&body).unwrap_or_else(|_| serde_json::Value::String(body.clone()));

    let job_id = scheduler::enqueue(
        &state.db,
        tenant.tenant_id,
        "webhook_event",
        serde_json::json!({ "provider": provider, "event": event }),
        0,
    )
    .await?;

    crate::audit::record(
        &state.db,
        None,
        crate::audit::actions::WEBHOOK_RECEIVED,
        Some("background_job"),
        Some(job_id.to_string()),
        Some(tenant.tenant_id),
        Some(serde_json::json!({ "provider": provider, "bytes": body.len() })),
    )
    .await;

    Ok(Json(
        serde_json::json!({ "received": true, "job_id": job_id }),
    ))
}
