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
    /// Open (not resolved/closed) maintenance tickets, when the viewer holds
    /// `maintenance:read`; omitted otherwise.
    pub open_tickets: Option<i64>,
    pub urgent_tickets: Option<i64>,
    /// Leases with a positive balance, when the viewer holds `ledger:read`.
    pub delinquent_tenants: Option<i64>,
    pub delinquent_balance_cents: Option<i64>,
    pub delinquent_balance_label: Option<String>,
    /// Applications awaiting a decision, when the viewer holds `application:read`.
    pub pending_applications: Option<i64>,
    /// Active reminders due within 14 days (or already overdue), when the
    /// viewer holds `calendar:read`.
    pub upcoming_reminders: Option<i64>,
    pub overdue_reminders: Option<i64>,
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
