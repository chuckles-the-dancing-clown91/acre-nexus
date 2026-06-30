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

#[derive(Serialize, schemars::JsonSchema)]
pub struct PlatformMetrics {
    pub tenant_count: i64,
    pub active_tenants: i64,
    pub total_properties: i64,
    pub total_managed_revenue_label: String,
}

// ---- Platform plane: staff + audited impersonation ----

#[derive(Serialize, schemars::JsonSchema)]
pub struct PlatformStaffSummary {
    pub id: Uuid,
    pub user_id: Uuid,
    pub email: String,
    pub name: String,
    pub status: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ImpersonateReq {
    /// Tenant to enter — uuid or slug.
    pub tenant: String,
    /// Why staff is entering the tenant (required, audit-logged).
    pub reason: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ImpersonationResp {
    pub session_id: Uuid,
    pub tenant_id: Uuid,
    pub reason: String,
    pub expires_at: String,
    /// A short-lived access token scoped to the tenant for this session.
    pub access_token: String,
    pub token_type: &'static str,
    pub expires_in: i64,
}

// ---- Firm provisioning (§5.1) ----

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ProvisionReq {
    /// URL-safe firm slug (reserves `{slug}.acrenexus.com`).
    pub slug: String,
    pub name: String,
    /// Subscription plan (defaults to `starter`).
    pub plan: Option<String>,
    pub owner_email: String,
    pub owner_name: Option<String>,
    /// Optional initial password; a temporary one is generated + returned if omitted.
    pub owner_password: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ProvisionResp {
    pub tenant_id: Uuid,
    pub slug: String,
    pub subdomain: String,
    pub owner_user_id: Uuid,
    pub owner_email: String,
    /// Present only when a password was auto-generated (shown once).
    pub temp_password: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ImpersonationSummary {
    pub id: Uuid,
    pub platform_staff_id: Uuid,
    pub tenant_id: Uuid,
    pub tenant_name: Option<String>,
    pub reason: String,
    pub expires_at: String,
    pub revoked_at: Option<String>,
    pub active: bool,
    pub created_at: String,
}
