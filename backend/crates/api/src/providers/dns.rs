//! **DNS verification provider** (issue #62) — checks that a tenant published
//! the SPF / DKIM / DMARC records for their custom sending domain.
//!
//! The real implementation resolves TXT records over **DNS-over-HTTPS**
//! (Cloudflare's `application/dns-json` endpoint) through the shared outbound
//! client — no resolver dependency, and the same sandbox-first posture as
//! every provider: simulated (every record verifies) unless `LIVE_PROVIDERS`
//! lists `dns`.

use super::{err, Provider, ProviderCtx, ProviderError};
use sea_orm::ConnectionTrait;
use serde::{Deserialize, Serialize};

const DOH_URL: &str = "https://cloudflare-dns.com/dns-query";

/// One TXT record expectation.
#[derive(Clone, Debug, Serialize)]
pub struct DnsCheck {
    /// Stable key the caller reports by (`spf` | `dkim` | `dmarc`).
    pub key: String,
    /// The DNS name to resolve (e.g. `_dmarc.portal.firm.com`).
    pub name: String,
    /// Verification passes when any TXT answer contains this substring.
    pub expect_contains: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct DnsRequest {
    pub checks: Vec<DnsCheck>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DnsResult {
    pub key: String,
    pub name: String,
    pub found: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct DnsResponse {
    pub results: Vec<DnsResult>,
}

/// The `application/dns-json` answer shape (the fields we read).
#[derive(Deserialize)]
struct DohAnswer {
    #[serde(rename = "Answer")]
    answer: Option<Vec<DohRecord>>,
}

#[derive(Deserialize)]
struct DohRecord {
    data: String,
}

/// Whether any TXT answer satisfies the expectation. DoH quotes TXT data (and
/// long records split into quoted chunks), so quotes are stripped before the
/// substring test.
pub fn answers_contain(answers: &[String], expect: &str) -> bool {
    answers
        .iter()
        .map(|a| a.replace('"', ""))
        .any(|a| a.contains(expect))
}

pub struct DnsProvider;

#[async_trait::async_trait]
impl Provider for DnsProvider {
    type Request = DnsRequest;
    type Response = DnsResponse;

    fn key(&self) -> &'static str {
        "dns"
    }

    async fn call<C: ConnectionTrait + Sync>(
        &self,
        _ctx: &ProviderCtx<'_, C>,
        req: &Self::Request,
    ) -> Result<Self::Response, ProviderError> {
        let http = super::client::build_http_client()?;
        let mut results = Vec::with_capacity(req.checks.len());
        for check in &req.checks {
            let resp = http
                .get(DOH_URL)
                .query(&[("name", check.name.as_str()), ("type", "TXT")])
                .header("accept", "application/dns-json")
                .send()
                .await
                .map_err(|e| err(format!("DoH query for {} failed: {e}", check.name)))?;
            if !resp.status().is_success() {
                return Err(err(format!(
                    "DoH query for {} returned {}",
                    check.name,
                    resp.status()
                )));
            }
            let parsed: DohAnswer = resp
                .json()
                .await
                .map_err(|e| err(format!("DoH response for {} unreadable: {e}", check.name)))?;
            let answers: Vec<String> = parsed
                .answer
                .unwrap_or_default()
                .into_iter()
                .map(|r| r.data)
                .collect();
            results.push(DnsResult {
                key: check.key.clone(),
                name: check.name.clone(),
                found: answers_contain(&answers, &check.expect_contains),
            });
        }
        Ok(DnsResponse { results })
    }

    /// Simulation: the records are considered published — dev/CI walk the
    /// whole verify flow without owning a domain.
    async fn simulate<C: ConnectionTrait + Sync>(
        &self,
        _ctx: &ProviderCtx<'_, C>,
        req: &Self::Request,
    ) -> Result<Self::Response, ProviderError> {
        Ok(DnsResponse {
            results: req
                .checks
                .iter()
                .map(|c| DnsResult {
                    key: c.key.clone(),
                    name: c.name.clone(),
                    found: true,
                })
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::DatabaseConnection;
    use uuid::Uuid;

    #[test]
    fn quoted_and_chunked_txt_answers_match() {
        assert!(answers_contain(
            &["\"v=spf1 include:spf.acrenexus.com ~all\"".into()],
            "v=spf1 include:spf.acrenexus.com"
        ));
        // Long TXT records arrive as concatenated quoted chunks.
        assert!(answers_contain(
            &["\"v=DKIM1; k=rsa; \" \"p=abc123\"".into()],
            "p=abc123"
        ));
        assert!(!answers_contain(
            &["\"v=spf1 -all\"".into()],
            "include:spf.acrenexus.com"
        ));
        assert!(!answers_contain(&[], "anything"));
    }

    #[tokio::test]
    async fn simulation_verifies_every_check() {
        let db = DatabaseConnection::Disconnected;
        let ctx = ProviderCtx::new(&db, Uuid::from_u128(1));
        let req = DnsRequest {
            checks: vec![
                DnsCheck {
                    key: "spf".into(),
                    name: "portal.firm.com".into(),
                    expect_contains: "v=spf1".into(),
                },
                DnsCheck {
                    key: "dkim".into(),
                    name: "acre._domainkey.portal.firm.com".into(),
                    expect_contains: "v=DKIM1".into(),
                },
            ],
        };
        let resp = DnsProvider.simulate(&ctx, &req).await.unwrap();
        assert_eq!(resp.results.len(), 2);
        assert!(resp.results.iter().all(|r| r.found));
    }
}
