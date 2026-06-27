use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct TenantSummary {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub plan: String,
    pub status: String,
    pub custom_domain: Option<String>,
    pub property_count: i64,
    pub managed_revenue_label: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct PlatformMetrics {
    pub tenant_count: i64,
    pub active_tenants: i64,
    pub total_properties: i64,
    pub total_managed_revenue_label: String,
}
