//! **Plaid** banking provider (roadmap issue #36) — bank account linking and
//! transaction feeds for reconciliation.
//!
//! Sandbox-first: [`Provider::simulate`] is the default. The simulated feed is
//! deterministic — the caller passes the deposits it *expects* (settled
//! payments for the account's entity) and the simulator returns them as bank
//! lines plus a stable pinch of noise (a bank fee, an unrelated deposit), so
//! auto-matching has both hits and leftovers to demonstrate. Live mode
//! (`LIVE_PROVIDERS=plaid` + `plaid.client_id` / `plaid.secret` credentials,
//! per-account access tokens under `plaid.access_token.<bank_account_id>`)
//! exchanges Link tokens and pulls `/transactions/sync`.

use super::{err, Provider, ProviderCtx, ProviderError};
use sea_orm::ConnectionTrait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Vault keys for the Plaid API credential pair.
pub const CLIENT_ID_KEY: &str = "plaid.client_id";
pub const SECRET_KEY: &str = "plaid.secret";

/// Vault key holding the access token for one linked account.
pub fn access_token_key(bank_account_id: Uuid) -> String {
    format!("plaid.access_token.{bank_account_id}")
}

fn base_url() -> String {
    // sandbox | development | production — sandbox by default.
    let env = std::env::var("PLAID_ENV").unwrap_or_else(|_| "sandbox".into());
    format!("https://{env}.plaid.com")
}

/// A deposit the sync caller expects to see land (a settled payment).
#[derive(Clone, Debug, Serialize)]
pub struct ExpectedDeposit {
    pub date: String,
    pub amount_cents: i64,
    pub memo: String,
}

#[derive(Clone, Debug, Serialize)]
pub enum BankRequest {
    /// Link a bank account for feeds. Live mode exchanges a Plaid Link
    /// `public_token`; the simulator mints a stable account id.
    Link {
        bank_account_id: Uuid,
        institution: String,
        /// Plaid Link public token (live mode only).
        public_token: Option<String>,
    },
    /// Pull the account's transactions since a date.
    Sync {
        bank_account_id: Uuid,
        account_external_id: String,
        since: String,
        /// What the ledger expects to have landed — drives the simulator.
        expected: Vec<ExpectedDeposit>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BankTxnLine {
    /// Provider transaction id (dedupe key).
    pub external_id: String,
    pub posted_date: String,
    pub description: String,
    /// Signed: positive = deposit into the account.
    pub amount_cents: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BankResponse {
    Linked { account_external_id: String },
    Transactions { lines: Vec<BankTxnLine> },
}

pub struct PlaidProvider;

#[async_trait::async_trait]
impl Provider for PlaidProvider {
    type Request = BankRequest;
    type Response = BankResponse;

    fn key(&self) -> &'static str {
        "plaid"
    }

    async fn call<C: ConnectionTrait + Sync>(
        &self,
        ctx: &ProviderCtx<'_, C>,
        req: &Self::Request,
    ) -> Result<Self::Response, ProviderError> {
        let client_id = ctx
            .secret(CLIENT_ID_KEY)
            .await?
            .ok_or_else(|| err("no credential configured under 'plaid.client_id'"))?;
        let secret = ctx
            .secret(SECRET_KEY)
            .await?
            .ok_or_else(|| err("no credential configured under 'plaid.secret'"))?;
        let http = super::client::build_http_client()?;
        let base = base_url();

        match req {
            BankRequest::Link {
                bank_account_id,
                public_token,
                ..
            } => {
                let public_token = public_token
                    .as_deref()
                    .ok_or_else(|| err("live linking requires a Plaid Link public_token"))?;
                let body = serde_json::json!({
                    "client_id": client_id,
                    "secret": secret,
                    "public_token": public_token,
                });
                let resp: serde_json::Value = http
                    .post(format!("{base}/item/public_token/exchange"))
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| err(format!("plaid request failed: {e}")))?
                    .json()
                    .await
                    .map_err(|e| err(format!("plaid returned non-JSON: {e}")))?;
                let access_token = resp
                    .get("access_token")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| err("plaid exchange missing access_token"))?;
                // Persist the access token in the vault for future syncs.
                crate::secrets::store(
                    ctx.db,
                    Some(ctx.tenant_id),
                    &access_token_key(*bank_account_id),
                    access_token,
                    None,
                )
                .await
                .map_err(|e| err(format!("failed to store plaid access token: {e}")))?;
                let account_id = resp
                    .get("item_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("plaid_item")
                    .to_string();
                Ok(BankResponse::Linked {
                    account_external_id: account_id,
                })
            }
            BankRequest::Sync {
                bank_account_id,
                since,
                ..
            } => {
                let access_token = ctx
                    .secret(&access_token_key(*bank_account_id))
                    .await?
                    .ok_or_else(|| err("no plaid access token for this account"))?;
                let body = serde_json::json!({
                    "client_id": client_id,
                    "secret": secret,
                    "access_token": access_token,
                    "start_date": since,
                    "end_date": chrono::Utc::now().date_naive().to_string(),
                });
                let resp: serde_json::Value = http
                    .post(format!("{base}/transactions/get"))
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| err(format!("plaid request failed: {e}")))?
                    .json()
                    .await
                    .map_err(|e| err(format!("plaid returned non-JSON: {e}")))?;
                let lines = resp
                    .get("transactions")
                    .and_then(|v| v.as_array())
                    .map(|txns| {
                        txns.iter()
                            .filter_map(|t| {
                                Some(BankTxnLine {
                                    external_id: t.get("transaction_id")?.as_str()?.to_string(),
                                    posted_date: t.get("date")?.as_str()?.to_string(),
                                    description: t
                                        .get("name")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("Transaction")
                                        .to_string(),
                                    // Plaid: positive = money leaving the account.
                                    amount_cents: -((t.get("amount")?.as_f64()? * 100.0).round()
                                        as i64),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                Ok(BankResponse::Transactions { lines })
            }
        }
    }

    async fn simulate<C: ConnectionTrait + Sync>(
        &self,
        _ctx: &ProviderCtx<'_, C>,
        req: &Self::Request,
    ) -> Result<Self::Response, ProviderError> {
        Ok(match req {
            BankRequest::Link {
                bank_account_id, ..
            } => BankResponse::Linked {
                account_external_id: format!("sim_acct_{}", bank_account_id.simple()),
            },
            BankRequest::Sync {
                account_external_id,
                since,
                expected,
                ..
            } => {
                // Every expected deposit lands, each with a stable id derived
                // from the account + date + amount so re-syncs dedupe cleanly.
                let mut lines: Vec<BankTxnLine> = expected
                    .iter()
                    .map(|e| BankTxnLine {
                        external_id: format!(
                            "sim_txn_{account_external_id}_{}_{}",
                            e.date, e.amount_cents
                        ),
                        posted_date: e.date.clone(),
                        description: e.memo.clone(),
                        amount_cents: e.amount_cents,
                    })
                    .collect();
                // Deterministic noise: a monthly service fee and one unrelated
                // deposit that reconciliation must leave unmatched.
                let month = &since[..7.min(since.len())];
                lines.push(BankTxnLine {
                    external_id: format!("sim_txn_{account_external_id}_{month}_fee"),
                    posted_date: since.clone(),
                    description: "Monthly service fee".into(),
                    amount_cents: -1_500,
                });
                lines.push(BankTxnLine {
                    external_id: format!("sim_txn_{account_external_id}_{month}_misc"),
                    posted_date: since.clone(),
                    description: "Mobile deposit".into(),
                    amount_cents: 12_345,
                });
                BankResponse::Transactions { lines }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::DatabaseConnection;

    #[tokio::test]
    async fn simulated_link_is_stable() {
        let db = DatabaseConnection::Disconnected;
        let ctx = ProviderCtx::new(&db, Uuid::from_u128(1));
        let req = BankRequest::Link {
            bank_account_id: Uuid::from_u128(5),
            institution: "First Cascade Bank".into(),
            public_token: None,
        };
        let BankResponse::Linked {
            account_external_id: a,
        } = PlaidProvider.simulate(&ctx, &req).await.unwrap()
        else {
            panic!("expected Linked");
        };
        let BankResponse::Linked {
            account_external_id: b,
        } = PlaidProvider.simulate(&ctx, &req).await.unwrap()
        else {
            panic!("expected Linked");
        };
        assert_eq!(a, b);
        assert!(a.starts_with("sim_acct_"));
    }

    #[tokio::test]
    async fn simulated_sync_returns_expected_plus_noise() {
        let db = DatabaseConnection::Disconnected;
        let ctx = ProviderCtx::new(&db, Uuid::from_u128(1));
        let req = BankRequest::Sync {
            bank_account_id: Uuid::from_u128(5),
            account_external_id: "sim_acct_x".into(),
            since: "2026-06-01".into(),
            expected: vec![ExpectedDeposit {
                date: "2026-06-03".into(),
                amount_cents: 185_000,
                memo: "ACH deposit — rent".into(),
            }],
        };
        let BankResponse::Transactions { lines } =
            PlaidProvider.simulate(&ctx, &req).await.unwrap()
        else {
            panic!("expected Transactions");
        };
        // 1 expected + 2 noise, all with stable ids.
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].amount_cents, 185_000);
        assert!(lines.iter().all(|l| l.external_id.starts_with("sim_txn_")));
        // Re-running yields identical ids (dedupe on re-sync).
        let BankResponse::Transactions { lines: again } =
            PlaidProvider.simulate(&ctx, &req).await.unwrap()
        else {
            panic!("expected Transactions");
        };
        assert_eq!(
            lines.iter().map(|l| &l.external_id).collect::<Vec<_>>(),
            again.iter().map(|l| &l.external_id).collect::<Vec<_>>()
        );
    }
}
