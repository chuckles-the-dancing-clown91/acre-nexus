//! Shared outbound **HTTP client** for provider implementations.
//!
//! Generalizes what [`crate::enrichment::geocode`] builds ad hoc: a `reqwest`
//! client that works behind the managed agent proxy (trusting its CA bundle
//! when present), plus per-provider base-URL and bearer-auth injection with the
//! credential resolved via [`crate::secrets::reveal`] — never from a response
//! or a log line.

use super::{err, ProviderCtx, ProviderError};
use sea_orm::ConnectionTrait;
use std::time::Duration;

/// Candidate locations for the agent proxy's CA bundle.
const CA_BUNDLE_PATHS: &[&str] = &["/root/.ccr/ca-bundle.crt"];

/// An outbound client bound to one provider's base URL, with optional bearer
/// auth from the secrets store. Part of the #16 framework surface: the first
/// live providers to construct one land with #35/#36/#62.
#[allow(dead_code)]
pub struct OutboundClient {
    http: reqwest::Client,
    base_url: String,
    bearer: Option<String>,
}

#[allow(dead_code)]
impl OutboundClient {
    /// Build a client for `base_url`. When `secret_key` is given, the tenant's
    /// credential is resolved and sent as `Authorization: Bearer …` on every
    /// request; a missing credential is an error, so providers fail loudly
    /// instead of calling upstream unauthenticated.
    pub async fn new<C: ConnectionTrait + Sync>(
        ctx: &ProviderCtx<'_, C>,
        base_url: &str,
        secret_key: Option<&str>,
    ) -> Result<Self, ProviderError> {
        let bearer = match secret_key {
            Some(key) => Some(
                ctx.secret(key)
                    .await?
                    .ok_or_else(|| err(format!("no credential configured under '{key}'")))?,
            ),
            None => None,
        };
        Ok(OutboundClient {
            http: build_http_client()?,
            base_url: base_url.trim_end_matches('/').to_string(),
            bearer,
        })
    }

    /// Start a request to `path` (joined to the base URL) with auth applied.
    pub fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}/{}", self.base_url, path.trim_start_matches('/'));
        let mut req = self.http.request(method, url);
        if let Some(token) = &self.bearer {
            req = req.bearer_auth(token);
        }
        req
    }
}

/// Build an HTTPS client that works behind the agent proxy. Shared by every
/// provider (and reusable by future live enrichment sources).
pub fn build_http_client() -> Result<reqwest::Client, ProviderError> {
    let mut builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("acre-nexus-integrations/0.1");

    for cert in proxy_ca_certificates() {
        builder = builder.add_root_certificate(cert);
    }

    builder
        .build()
        .map_err(|e| err(format!("failed to build HTTP client: {e}")))
}

/// Load any extra CA certificates needed to trust the proxy (best-effort).
fn proxy_ca_certificates() -> Vec<reqwest::Certificate> {
    let mut paths: Vec<String> = CA_BUNDLE_PATHS.iter().map(|s| s.to_string()).collect();
    if let Ok(p) = std::env::var("SSL_CERT_FILE") {
        paths.push(p);
    }

    let mut certs = Vec::new();
    for path in paths {
        if let Ok(pem) = std::fs::read(&path) {
            match reqwest::Certificate::from_pem_bundle(&pem) {
                Ok(bundle) => certs.extend(bundle),
                Err(e) => tracing::debug!("ignoring CA bundle {path}: {e}"),
            }
        }
    }
    certs
}
