//! **Outbound webhook delivery provider** (issue #68) — the outbound
//! counterpart of the inbound ingestion framework in [`super::webhook`].
//!
//! One POST to a subscriber URL, with the raw JSON body signed
//! `X-Acre-Signature: sha256=<hex>` (the same HMAC scheme subscribers can
//! verify with [`super::webhook::verify`]). Sandbox-first like every
//! provider: simulated deliveries succeed deterministically — except to URLs
//! containing `fail`, so the retry → dead-letter path is demoable without a
//! network — until `LIVE_PROVIDERS` lists `webhooks`.

use super::{err, Provider, ProviderCtx, ProviderError};
use sea_orm::ConnectionTrait;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct WebhookOutRequest {
    pub url: String,
    /// The raw JSON body to POST (already serialized — the signature covers
    /// these exact bytes).
    pub body: String,
    /// `sha256=<hex>` over `body`, from [`super::webhook::sign`].
    pub signature: String,
    /// `X-Acre-Event` header value.
    pub event_type: String,
    /// `X-Acre-Delivery` header value (the delivery id).
    pub delivery_id: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct WebhookOutResponse {
    pub status_code: u16,
}

pub struct WebhookOutProvider;

#[async_trait::async_trait]
impl Provider for WebhookOutProvider {
    type Request = WebhookOutRequest;
    type Response = WebhookOutResponse;

    fn key(&self) -> &'static str {
        "webhooks"
    }

    async fn call<C: ConnectionTrait + Sync>(
        &self,
        _ctx: &ProviderCtx<'_, C>,
        req: &Self::Request,
    ) -> Result<Self::Response, ProviderError> {
        let http = super::client::build_http_client()?;
        let resp = http
            .post(&req.url)
            .header("content-type", "application/json")
            .header("X-Acre-Signature", &req.signature)
            .header("X-Acre-Event", &req.event_type)
            .header("X-Acre-Delivery", &req.delivery_id)
            .body(req.body.clone())
            .send()
            .await
            .map_err(|e| err(format!("delivery to {} failed: {e}", req.url)))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(err(format!("subscriber {} answered {}", req.url, status)));
        }
        Ok(WebhookOutResponse {
            status_code: status.as_u16(),
        })
    }

    /// Deterministic sandbox: deliveries succeed, except to URLs containing
    /// `fail` (the demoable failure path, like the `…0002` declining card).
    async fn simulate<C: ConnectionTrait + Sync>(
        &self,
        _ctx: &ProviderCtx<'_, C>,
        req: &Self::Request,
    ) -> Result<Self::Response, ProviderError> {
        if req.url.contains("fail") {
            return Err(err(format!(
                "simulated subscriber {} refused the delivery",
                req.url
            )));
        }
        Ok(WebhookOutResponse { status_code: 200 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::DatabaseConnection;
    use uuid::Uuid;

    #[tokio::test]
    async fn simulated_delivery_succeeds_and_fail_urls_refuse() {
        let db = DatabaseConnection::Disconnected;
        let ctx = ProviderCtx::new(&db, Uuid::from_u128(1));
        let ok = WebhookOutRequest {
            url: "https://vendor.example/hooks/acre".into(),
            body: "{}".into(),
            signature: "sha256=00".into(),
            event_type: "listing.updated".into(),
            delivery_id: Uuid::from_u128(2).to_string(),
        };
        assert_eq!(
            WebhookOutProvider
                .simulate(&ctx, &ok)
                .await
                .unwrap()
                .status_code,
            200
        );
        let bad = WebhookOutRequest {
            url: "https://vendor.example/hooks/failing".into(),
            ..ok
        };
        assert!(WebhookOutProvider.simulate(&ctx, &bad).await.is_err());
    }
}
