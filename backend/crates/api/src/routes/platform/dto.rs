use serde::{Deserialize, Serialize};
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

/// A single tenant with its rollups, for the platform tenant-detail view.
#[derive(Serialize, schemars::JsonSchema)]
pub struct TenantDetail {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub plan: String,
    pub status: String,
    pub custom_domain: Option<String>,
    pub property_count: i64,
    pub member_count: i64,
    pub revenue_cents: i64,
    pub managed_revenue_label: String,
    pub created_at: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateTenantReq {
    pub slug: String,
    pub name: String,
    /// Subscription plan; defaults to `starter` when omitted/empty.
    #[serde(default)]
    pub plan: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateTenantReq {
    /// `active` | `suspended` | `trial`.
    pub status: Option<String>,
    pub plan: Option<String>,
    pub name: Option<String>,
    pub custom_domain: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct PlatformMetrics {
    pub tenant_count: i64,
    pub active_tenants: i64,
    pub total_properties: i64,
    pub total_managed_revenue_label: String,
}
