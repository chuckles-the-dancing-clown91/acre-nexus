//! The **outbound provider + webhook framework** (roadmap issue #16).
//!
//! The enrichment engine ([`crate::enrichment`]) proved the shape every
//! integration should take: deterministic simulated implementations, one real
//! implementation to keep it honest, and everything riding the durable
//! `background_job` queue with retries + backoff. This module generalizes that
//! proven shape into the platform's first real [`Provider`] **trait**:
//!
//! * a typed request/response pair per integration ([`Provider::Request`] /
//!   [`Provider::Response`]),
//! * both [`call`](Provider::call) (real) and [`simulate`](Provider::simulate)
//!   (CI/demo) methods, so every new provider is sandbox-first by construction
//!   — [`execute`](Provider::execute) routes between them,
//! * a generic job [`runner`](run) that translates provider errors into
//!   [`JobOutcome::retry`] vs [`JobOutcome::failed`] exactly like the
//!   enrichment module does, reusing its [`backoff`] formula, and
//! * inbound webhook ingestion with HMAC signature verification
//!   ([`webhook`]), which enqueues a `background_job` instead of handling
//!   events synchronously.

pub mod bank;
pub mod client;
pub mod payments;
pub mod webhook;

use crate::modules::JobOutcome;
use crate::secrets;
use sea_orm::ConnectionTrait;
use serde::Serialize;
use std::fmt;
use uuid::Uuid;

/// A provider failure (transport error, rejected request, bad payload …).
/// Mirrors [`crate::enrichment::data::EnrichmentError`]'s single-variant
/// newtype: the message surfaces on the job's `last_error`, and the retry
/// budget — not an error taxonomy — decides transient vs permanent.
#[derive(Debug, Clone)]
pub struct ProviderError(pub String);

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ProviderError {}

/// Convenience: build a [`ProviderError`] from anything stringy.
pub fn err(msg: impl Into<String>) -> ProviderError {
    ProviderError(msg.into())
}

/// Exponential backoff (seconds) for transient provider failures — the one
/// backoff formula in the codebase, shared with the enrichment module.
pub fn backoff(attempts: i32) -> i64 {
    let exp = attempts.clamp(0, 6) as u32;
    4_i64 * 2_i64.pow(exp)
}

/// Context handed to a provider: the tenant it acts for and a connection for
/// resolving credentials via [`crate::secrets::reveal`].
pub struct ProviderCtx<'a, C: ConnectionTrait> {
    pub db: &'a C,
    pub tenant_id: Uuid,
}

impl<'a, C: ConnectionTrait> ProviderCtx<'a, C> {
    pub fn new(db: &'a C, tenant_id: Uuid) -> Self {
        ProviderCtx { db, tenant_id }
    }

    /// Resolve a credential for this tenant (falling back to the platform-wide
    /// value). Server-side only — never serialize the result into a response.
    /// Consumed by [`super::providers::client::OutboundClient`]; the first live
    /// providers to ride it land with #35/#36/#62.
    #[allow(dead_code)]
    pub async fn secret(&self, key: &str) -> Result<Option<String>, ProviderError> {
        secrets::reveal(self.db, Some(self.tenant_id), key)
            .await
            .map_err(|e| err(format!("secret lookup for '{key}' failed: {e}")))
    }
}

/// The contract every outbound integration implements. Adding a provider is a
/// single small file: define the request/response types, implement `call`
/// (real) and `simulate` (deterministic, for CI/demos), and pick the stable
/// `key` used to configure it. Swapping simulated → live is a configuration
/// change ([`live_providers`]), not a call-site change.
#[async_trait::async_trait]
pub trait Provider: Send + Sync {
    /// What the provider is asked to do.
    type Request: Serialize + Send + Sync;
    /// What a (real or simulated) call yields.
    type Response: Serialize + Send + Sync;

    /// Stable configuration key, e.g. `email` or `sms`.
    fn key(&self) -> &'static str;

    /// Call the real upstream service.
    async fn call<C: ConnectionTrait + Sync>(
        &self,
        ctx: &ProviderCtx<'_, C>,
        req: &Self::Request,
    ) -> Result<Self::Response, ProviderError>;

    /// Deterministic sandbox implementation for CI / demos.
    async fn simulate<C: ConnectionTrait + Sync>(
        &self,
        ctx: &ProviderCtx<'_, C>,
        req: &Self::Request,
    ) -> Result<Self::Response, ProviderError>;

    /// Route to [`call`](Self::call) when this provider is configured live
    /// (`LIVE_PROVIDERS` lists its key), else [`simulate`](Self::simulate).
    async fn execute<C: ConnectionTrait + Sync>(
        &self,
        ctx: &ProviderCtx<'_, C>,
        req: &Self::Request,
    ) -> Result<Self::Response, ProviderError> {
        if live_providers()
            .iter()
            .any(|k| k == self.key() || k == "all")
        {
            self.call(ctx, req).await
        } else {
            self.simulate(ctx, req).await
        }
    }
}

/// Whether `key` is configured live — for callers whose *control flow* differs
/// between live and simulated (e.g. the payment pipeline waits on a webhook in
/// live mode but self-settles after a delay in simulation).
pub fn is_live(key: &str) -> bool {
    live_providers().iter().any(|k| k == key || k == "all")
}

/// Provider keys enabled for live (non-simulated) calls, from the
/// comma-separated `LIVE_PROVIDERS` env var (`all` enables everything).
/// Default: everything simulated — the same sandbox-first posture as the
/// enrichment engine.
fn live_providers() -> Vec<String> {
    std::env::var("LIVE_PROVIDERS")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

/// The generic job runner: execute `provider` for one background job, audit the
/// call, and translate an error into the right [`JobOutcome`] — a transient
/// [`retry`](JobOutcome::retry) with [`backoff`] while budget remains, then a
/// terminal [`failed`](JobOutcome::failed). On success the caller persists the
/// typed response and builds its own completion outcome.
pub async fn run<P, C>(
    provider: &P,
    ctx: &ProviderCtx<'_, C>,
    job: &entity::background_job::Model,
    req: &P::Request,
) -> Result<P::Response, JobOutcome>
where
    P: Provider,
    C: ConnectionTrait + Sync,
{
    match provider.execute(ctx, req).await {
        Ok(resp) => {
            record_call(ctx, provider.key(), job.id, "succeeded", None).await;
            Ok(resp)
        }
        Err(e) => {
            let attempts = job.attempts + 1;
            let terminal = attempts >= job.max_attempts;
            record_call(
                ctx,
                provider.key(),
                job.id,
                if terminal { "failed" } else { "retrying" },
                Some(&e.0),
            )
            .await;
            if terminal {
                Err(JobOutcome::failed(e.to_string()))
            } else {
                Err(JobOutcome::retry(backoff(job.attempts), e.to_string()))
            }
        }
    }
}

/// Audit one provider invocation (`provider.call`) — the fact of the call and
/// its outcome, never request/response contents (which may carry PII).
async fn record_call<C: ConnectionTrait + Sync>(
    ctx: &ProviderCtx<'_, C>,
    provider_key: &str,
    job_id: Uuid,
    status: &str,
    error: Option<&str>,
) {
    crate::audit::record(
        ctx.db,
        None,
        crate::audit::actions::PROVIDER_CALL,
        Some("background_job"),
        Some(job_id.to_string()),
        Some(ctx.tenant_id),
        Some(serde_json::json!({
            "provider": provider_key,
            "status": status,
            "error": error,
        })),
    )
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_matches_enrichment_formula() {
        assert_eq!(backoff(0), 4);
        assert_eq!(backoff(1), 8);
        assert_eq!(backoff(3), 32);
        // Clamped at 2^6.
        assert_eq!(backoff(6), 256);
        assert_eq!(backoff(50), 256);
    }
}
