use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct Kpi {
    pub label: String,
    pub value: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct PortfolioSummary {
    pub properties: i64,
    pub units: i64,
    pub occupied_units: i64,
    pub occupancy_pct: i64,
    pub monthly_revenue_cents: i64,
    pub kpis: Vec<Kpi>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct LlcGroup {
    pub id: Uuid,
    pub name: String,
    pub ein: String,
    pub state: String,
    pub property_count: usize,
    pub units: i64,
    pub monthly_rent_cents: i64,
    pub monthly_rent_label: String,
    pub properties: Vec<super::super::properties::PropertyResp>,
}
