use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, schemars::JsonSchema)]
pub struct LoginReq {
    pub email: String,
    pub password: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct TokenResp {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: &'static str,
    pub expires_in: i64,
    pub user: UserResp,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct UserResp {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    /// Primary tenant of the account (back-compat).
    pub tenant_id: Option<Uuid>,
    /// The workspace the current token is scoped to (`None` = Acre HQ / platform).
    pub active_tenant_id: Option<Uuid>,
    pub is_platform_staff: bool,
    pub permissions: Vec<String>,
    /// Every persona the user holds, across platform and tenants.
    pub memberships: Vec<MembershipSummary>,
    /// Workspaces the user can switch into (drives the workspace switcher).
    pub workspaces: Vec<WorkspaceSummary>,
}

/// One of a user's personas, with the owning workspace resolved for display.
#[derive(Serialize, schemars::JsonSchema)]
pub struct MembershipSummary {
    pub scope: String,
    pub tenant_id: Option<Uuid>,
    pub tenant_slug: Option<String>,
    pub tenant_name: Option<String>,
    pub profile_type: String,
    pub title: Option<String>,
    pub status: String,
    pub is_primary: bool,
}

/// A workspace the user can operate in.
#[derive(Serialize, schemars::JsonSchema, Clone)]
pub struct WorkspaceSummary {
    /// `platform` (Acre HQ) or `tenant` (a client workspace).
    pub kind: String,
    pub tenant_id: Option<Uuid>,
    pub slug: Option<String>,
    pub name: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct RefreshReq {
    pub refresh_token: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SwitchReq {
    /// Target workspace; `null` selects the platform (Acre HQ) context.
    pub tenant_id: Option<Uuid>,
}

/// Response from a workspace switch — a fresh access token scoped to the chosen
/// workspace, with permissions re-resolved for it.
#[derive(Serialize, schemars::JsonSchema)]
pub struct SwitchResp {
    pub access_token: String,
    pub token_type: &'static str,
    pub expires_in: i64,
    pub user: UserResp,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct LogoutReq {
    pub refresh_token: String,
}
