//! **Stripe** payments provider (roadmap issue #35) — cards + ACH for rent,
//! deposits, and fees, plus ACH payouts to owners.
//!
//! Tokenized by construction: requests only ever carry provider method tokens
//! (`pm_…` from Stripe.js / Elements in a live deployment, `sim_pm_…` from the
//! simulated tokenizer) — PANs and bank account numbers never touch the
//! platform. Sandbox-first like every provider: [`Provider::simulate`] is the
//! default; setting `LIVE_PROVIDERS=stripe` (plus the `stripe.secret_key`
//! credential in the vault) switches [`Provider::call`] to the real API in
//! whatever mode the key selects (`sk_test_…` = Stripe test mode).
//!
//! Settlement is **webhook-driven** in live mode (`payment_intent.succeeded` /
//! `payment_intent.payment_failed` arrive on `POST /webhooks/stripe`); the
//! simulated processor instead confirms after the tenant's configured
//! callback delay, through the same settlement path.

use super::{err, Provider, ProviderCtx, ProviderError};
use sea_orm::ConnectionTrait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Vault key holding the Stripe secret key (`sk_test_…` / `sk_live_…`).
pub const SECRET_KEY: &str = "stripe.secret_key";
const BASE_URL: &str = "https://api.stripe.com/v1";

/// A charge against a saved method, or an ACH payout to an owner.
#[derive(Clone, Debug, Serialize)]
pub enum PaymentsRequest {
    Charge {
        /// Our `lease_payment.id` — becomes the idempotency key + metadata.
        reference: Uuid,
        amount_cents: i64,
        /// Provider method token (`pm_…` / `sim_pm_…`).
        method_external_id: String,
        description: String,
    },
    Payout {
        /// Our `owner_payout.id`.
        reference: Uuid,
        amount_cents: i64,
        description: String,
    },
}

/// The processor's answer: an external id plus a coarse status our pipeline
/// understands (`processing` | `succeeded` | `failed`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaymentsResponse {
    pub external_id: String,
    pub status: String,
    pub failure_reason: Option<String>,
}

pub struct StripeProvider;

#[async_trait::async_trait]
impl Provider for StripeProvider {
    type Request = PaymentsRequest;
    type Response = PaymentsResponse;

    fn key(&self) -> &'static str {
        "stripe"
    }

    async fn call<C: ConnectionTrait + Sync>(
        &self,
        ctx: &ProviderCtx<'_, C>,
        req: &Self::Request,
    ) -> Result<Self::Response, ProviderError> {
        let client = super::client::OutboundClient::new(ctx, BASE_URL, Some(SECRET_KEY)).await?;
        match req {
            PaymentsRequest::Charge {
                reference,
                amount_cents,
                method_external_id,
                description,
            } => {
                // An off-session PaymentIntent confirmed immediately against the
                // saved method; the webhook delivers the terminal state.
                let form = [
                    ("amount", amount_cents.to_string()),
                    ("currency", "usd".into()),
                    ("payment_method", method_external_id.clone()),
                    ("confirm", "true".into()),
                    ("off_session", "true".into()),
                    ("description", description.clone()),
                    ("metadata[reference]", reference.to_string()),
                ];
                let resp = client
                    .request(reqwest::Method::POST, "/payment_intents")
                    .header("Idempotency-Key", reference.to_string())
                    .form(&form)
                    .send()
                    .await
                    .map_err(|e| err(format!("stripe request failed: {e}")))?;
                parse_stripe_object(resp).await
            }
            PaymentsRequest::Payout {
                reference,
                amount_cents,
                description,
            } => {
                let form = [
                    ("amount", amount_cents.to_string()),
                    ("currency", "usd".into()),
                    ("description", description.clone()),
                    ("metadata[reference]", reference.to_string()),
                ];
                let resp = client
                    .request(reqwest::Method::POST, "/payouts")
                    .header("Idempotency-Key", reference.to_string())
                    .form(&form)
                    .send()
                    .await
                    .map_err(|e| err(format!("stripe request failed: {e}")))?;
                parse_stripe_object(resp).await
            }
        }
    }

    async fn simulate<C: ConnectionTrait + Sync>(
        &self,
        _ctx: &ProviderCtx<'_, C>,
        req: &Self::Request,
    ) -> Result<Self::Response, ProviderError> {
        // Deterministic: the external id derives from our reference, and the
        // canonical Stripe decline test number (…0002) declines here too so
        // demos can exercise the failure path.
        Ok(match req {
            PaymentsRequest::Charge {
                reference,
                method_external_id,
                ..
            } => {
                if method_external_id.ends_with("0002") {
                    PaymentsResponse {
                        external_id: format!("sim_pi_{}", reference.simple()),
                        status: "failed".into(),
                        failure_reason: Some("card declined (simulated)".into()),
                    }
                } else {
                    PaymentsResponse {
                        external_id: format!("sim_pi_{}", reference.simple()),
                        status: "processing".into(),
                        failure_reason: None,
                    }
                }
            }
            PaymentsRequest::Payout { reference, .. } => PaymentsResponse {
                external_id: format!("sim_po_{}", reference.simple()),
                status: "processing".into(),
                failure_reason: None,
            },
        })
    }
}

/// Map a Stripe object response (PaymentIntent / Payout) onto our coarse
/// status vocabulary.
async fn parse_stripe_object(resp: reqwest::Response) -> Result<PaymentsResponse, ProviderError> {
    let status_code = resp.status();
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| err(format!("stripe returned non-JSON: {e}")))?;
    if !status_code.is_success() {
        let msg = body
            .pointer("/error/message")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error");
        // A decline is a terminal outcome, not a transport failure: surface it
        // as a failed response when Stripe still minted an intent.
        if let Some(id) = body
            .pointer("/error/payment_intent/id")
            .and_then(|v| v.as_str())
        {
            return Ok(PaymentsResponse {
                external_id: id.to_string(),
                status: "failed".into(),
                failure_reason: Some(msg.to_string()),
            });
        }
        return Err(err(format!("stripe error ({status_code}): {msg}")));
    }
    let id = body
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| err("stripe response missing id"))?;
    let raw = body.get("status").and_then(|v| v.as_str()).unwrap_or("");
    let status = match raw {
        "succeeded" | "paid" => "succeeded",
        "canceled" | "failed" => "failed",
        // requires_action / processing / pending / in_transit …
        _ => "processing",
    };
    Ok(PaymentsResponse {
        external_id: id.to_string(),
        status: status.into(),
        failure_reason: (status == "failed").then(|| format!("stripe status: {raw}")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::DatabaseConnection;

    #[tokio::test]
    async fn simulated_charge_is_deterministic_and_processing() {
        let db = DatabaseConnection::Disconnected;
        let ctx = ProviderCtx::new(&db, Uuid::from_u128(1));
        let reference = Uuid::from_u128(7);
        let req = PaymentsRequest::Charge {
            reference,
            amount_cents: 185_000,
            method_external_id: "sim_pm_4242".into(),
            description: "Rent".into(),
        };
        let a = StripeProvider.simulate(&ctx, &req).await.unwrap();
        let b = StripeProvider.simulate(&ctx, &req).await.unwrap();
        assert_eq!(a.external_id, b.external_id);
        assert!(a.external_id.starts_with("sim_pi_"));
        assert_eq!(a.status, "processing");
        assert!(a.failure_reason.is_none());
    }

    #[tokio::test]
    async fn simulated_decline_card_fails() {
        let db = DatabaseConnection::Disconnected;
        let ctx = ProviderCtx::new(&db, Uuid::from_u128(1));
        let req = PaymentsRequest::Charge {
            reference: Uuid::from_u128(8),
            amount_cents: 185_000,
            method_external_id: "sim_pm_0002".into(),
            description: "Rent".into(),
        };
        let resp = StripeProvider.simulate(&ctx, &req).await.unwrap();
        assert_eq!(resp.status, "failed");
        assert!(resp.failure_reason.is_some());
    }

    #[tokio::test]
    async fn simulated_payout_processes() {
        let db = DatabaseConnection::Disconnected;
        let ctx = ProviderCtx::new(&db, Uuid::from_u128(1));
        let req = PaymentsRequest::Payout {
            reference: Uuid::from_u128(9),
            amount_cents: 500_000,
            description: "Owner draw".into(),
        };
        let resp = StripeProvider.simulate(&ctx, &req).await.unwrap();
        assert!(resp.external_id.starts_with("sim_po_"));
        assert_eq!(resp.status, "processing");
    }
}
