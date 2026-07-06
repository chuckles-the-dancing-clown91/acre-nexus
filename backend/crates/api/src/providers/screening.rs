//! **Checkr** tenant-screening provider (roadmap Phase 4, epic #8) — credit +
//! criminal + eviction behind the Phase 1 provider framework.
//!
//! Sandbox-first like every provider: [`Provider::simulate`] is the default —
//! a deterministic report derived from the applicant (stable across retries,
//! so CI and demos are reproducible). Setting `LIVE_PROVIDERS=checkr` (plus
//! the `checkr.api_key` credential in the vault) orders real reports; their
//! terminal state arrives on `POST /webhooks/checkr` through the shared
//! signature-verified ingestion endpoint.
//!
//! FCRA discipline: a report is only ever ordered with the applicant's
//! consent stamped (the pipeline enforces it), and requests carry identity
//! attributes only — never SSNs through our storage.

use super::{err, Provider, ProviderCtx, ProviderError};
use sea_orm::ConnectionTrait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Vault key holding the Checkr API key.
pub const SECRET_KEY: &str = "checkr.api_key";
const BASE_URL: &str = "https://api.checkr.com/v1";

#[derive(Clone, Debug, Serialize)]
pub struct ScreeningRequest {
    /// Our `screening_report.id` — idempotency key + webhook correlation.
    pub reference: Uuid,
    pub candidate_name: String,
    pub email: String,
    /// Stated credit score, if the applicant provided one (the simulator
    /// treats it as the bureau answer; a live provider ignores it).
    pub stated_credit_score: Option<i32>,
    pub include_credit: bool,
    pub include_criminal: bool,
    pub include_eviction: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScreeningResponse {
    pub external_id: String,
    /// `pending` (live: webhook completes it later) | `complete`.
    pub status: String,
    pub credit_score: Option<i32>,
    pub criminal_records: Option<i32>,
    pub eviction_records: Option<i32>,
    /// `clear` | `consider`.
    pub recommendation: Option<String>,
}

pub struct CheckrProvider;

#[async_trait::async_trait]
impl Provider for CheckrProvider {
    type Request = ScreeningRequest;
    type Response = ScreeningResponse;

    fn key(&self) -> &'static str {
        "checkr"
    }

    async fn call<C: ConnectionTrait + Sync>(
        &self,
        ctx: &ProviderCtx<'_, C>,
        req: &Self::Request,
    ) -> Result<Self::Response, ProviderError> {
        let client = super::client::OutboundClient::new(ctx, BASE_URL, Some(SECRET_KEY)).await?;
        // Order an invitation-less report against the candidate. Checkr
        // answers `pending`; the completed report arrives by webhook.
        let body = serde_json::json!({
            "package": "tenant_screening",
            "candidate": { "email": req.email, "full_name": req.candidate_name },
            "metadata": { "reference": req.reference.to_string() },
        });
        let resp = client
            .request(reqwest::Method::POST, "/reports")
            .header("Idempotency-Key", req.reference.to_string())
            .json(&body)
            .send()
            .await
            .map_err(|e| err(format!("checkr request failed: {e}")))?;
        let status_code = resp.status();
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| err(format!("checkr returned non-JSON: {e}")))?;
        if !status_code.is_success() {
            let msg = json
                .pointer("/error")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            return Err(err(format!("checkr error ({status_code}): {msg}")));
        }
        let id = json
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| err("checkr response missing id"))?;
        Ok(ScreeningResponse {
            external_id: id.to_string(),
            status: "pending".into(),
            credit_score: None,
            criminal_records: None,
            eviction_records: None,
            recommendation: None,
        })
    }

    async fn simulate<C: ConnectionTrait + Sync>(
        &self,
        _ctx: &ProviderCtx<'_, C>,
        req: &Self::Request,
    ) -> Result<Self::Response, ProviderError> {
        // Deterministic bureau: everything derives from a hash of the
        // applicant's email, so the same applicant always screens the same.
        let h = seed(&req.email);

        let credit_score = if req.include_credit {
            // Stated score wins (it's what the tenant's policy already saw);
            // otherwise 580–839 from the hash.
            Some(req.stated_credit_score.unwrap_or(580 + (h % 260) as i32))
        } else {
            None
        };
        // Records are rare and deterministic; an email containing "flag"
        // always trips one of each (the demo lever, like Stripe's 0002 card).
        let flagged = req.email.contains("flag");
        let criminal_records = req
            .include_criminal
            .then_some(if flagged || h.is_multiple_of(29) {
                1
            } else {
                0
            });
        let eviction_records =
            req.include_eviction
                .then_some(if flagged || h % 41 == 3 { 1 } else { 0 });
        let any_records = criminal_records.unwrap_or(0) > 0 || eviction_records.unwrap_or(0) > 0;

        Ok(ScreeningResponse {
            external_id: format!("sim_rpt_{}", req.reference.simple()),
            status: "complete".into(),
            credit_score,
            criminal_records,
            eviction_records,
            recommendation: Some(if any_records { "consider" } else { "clear" }.into()),
        })
    }
}

/// Stable 32-bit seed from an applicant identifier.
fn seed(input: &str) -> u32 {
    let digest = Sha256::digest(input.to_lowercase().as_bytes());
    u32::from_le_bytes([digest[0], digest[1], digest[2], digest[3]])
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::DatabaseConnection;

    fn req(email: &str, stated: Option<i32>) -> ScreeningRequest {
        ScreeningRequest {
            reference: Uuid::from_u128(7),
            candidate_name: "Test Applicant".into(),
            email: email.into(),
            stated_credit_score: stated,
            include_credit: true,
            include_criminal: true,
            include_eviction: true,
        }
    }

    #[tokio::test]
    async fn simulated_reports_are_deterministic() {
        let db = DatabaseConnection::Disconnected;
        let ctx = ProviderCtx::new(&db, Uuid::from_u128(1));
        let a = CheckrProvider
            .simulate(&ctx, &req("t@example.com", None))
            .await
            .unwrap();
        let b = CheckrProvider
            .simulate(&ctx, &req("t@example.com", None))
            .await
            .unwrap();
        assert_eq!(a.credit_score, b.credit_score);
        assert_eq!(a.criminal_records, b.criminal_records);
        assert_eq!(a.eviction_records, b.eviction_records);
        assert_eq!(a.status, "complete");
        assert!(a.external_id.starts_with("sim_rpt_"));
        let score = a.credit_score.unwrap();
        assert!((580..840).contains(&score), "score {score}");
    }

    #[tokio::test]
    async fn stated_score_wins_and_flag_email_trips_records() {
        let db = DatabaseConnection::Disconnected;
        let ctx = ProviderCtx::new(&db, Uuid::from_u128(1));
        let stated = CheckrProvider
            .simulate(&ctx, &req("t@example.com", Some(712)))
            .await
            .unwrap();
        assert_eq!(stated.credit_score, Some(712));
        assert_eq!(stated.recommendation.as_deref(), Some("clear"));

        let flagged = CheckrProvider
            .simulate(&ctx, &req("flag@example.com", None))
            .await
            .unwrap();
        assert_eq!(flagged.criminal_records, Some(1));
        assert_eq!(flagged.eviction_records, Some(1));
        assert_eq!(flagged.recommendation.as_deref(), Some("consider"));
    }
}
