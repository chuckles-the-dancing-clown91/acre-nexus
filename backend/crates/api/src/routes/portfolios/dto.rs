use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct PortfolioResp {
    pub id: Uuid,
    pub name: String,
    pub strategy: String,
    /// Number of properties grouped under this portfolio.
    pub property_count: i64,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreatePortfolioReq {
    pub name: String,
    /// Free-form strategy/grouping label (e.g. `flip`, `cashflow`, `pacific-nw`).
    pub strategy: Option<String>,
}
