use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ===========================================================================
// Catalogs
// ===========================================================================

#[derive(Serialize, schemars::JsonSchema)]
pub struct PermissionDto {
    pub key: String,
    pub category: String,
    pub label: String,
    pub description: String,
    pub scope: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ProfileTypeDto {
    pub key: String,
    pub scope: String,
    pub label: String,
    pub description: String,
    pub default_role: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct AuditEntry {
    pub id: Uuid,
    pub actor_user_id: Option<Uuid>,
    pub actor_name: Option<String>,
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub tenant_id: Option<Uuid>,
    pub metadata: Option<serde_json::Value>,
    /// Kind of principal: `user`, `api_token`, `public`, or `system`.
    pub principal_kind: Option<String>,
    // ---- Request context (present on per-request entries) ----
    pub method: Option<String>,
    pub path: Option<String>,
    pub status_code: Option<i32>,
    pub ip: Option<String>,
    pub duration_ms: Option<i64>,
    pub request_id: Option<Uuid>,
    pub created_at: String,
}

// ===========================================================================
// Roles
// ===========================================================================

#[derive(Serialize, schemars::JsonSchema)]
pub struct RoleDto {
    pub id: Uuid,
    pub scope: String,
    pub tenant_id: Option<Uuid>,
    pub key: String,
    pub name: String,
    pub description: String,
    pub is_system: bool,
    pub permissions: Vec<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateRoleReq {
    pub scope: String,
    pub tenant_id: Option<Uuid>,
    pub key: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub permissions: Vec<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateRoleReq {
    pub name: Option<String>,
    pub description: Option<String>,
    /// When present, fully replaces the role's permission set.
    pub permissions: Option<Vec<String>>,
}

// ===========================================================================
// Users + profiles + memberships
// ===========================================================================

#[derive(Serialize, schemars::JsonSchema)]
pub struct UserListItem {
    pub id: Uuid,
    pub email: String,
    pub username: Option<String>,
    pub name: String,
    pub status: String,
    pub is_platform_staff: bool,
    pub tenant_id: Option<Uuid>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct NewMembership {
    pub scope: String,
    pub tenant_id: Option<Uuid>,
    pub profile_type: String,
    pub title: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct ProfileInput {
    pub legal_first_name: Option<String>,
    pub legal_middle_name: Option<String>,
    pub legal_last_name: Option<String>,
    pub preferred_name: Option<String>,
    /// ISO date `YYYY-MM-DD`.
    pub date_of_birth: Option<String>,
    pub phone: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub city: Option<String>,
    pub region: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    /// Plaintext SSN — encrypted before storage, never returned.
    pub ssn: Option<String>,
    pub gov_id_type: Option<String>,
    /// Plaintext government-ID number — encrypted before storage.
    pub gov_id_number: Option<String>,
    pub photo_url: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateUserReq {
    pub email: String,
    pub username: Option<String>,
    pub name: String,
    /// Optional initial password; if omitted, the account is `invited` with a
    /// random password (an invite flow would set it later).
    pub password: Option<String>,
    pub membership: Option<NewMembership>,
    pub profile: Option<ProfileInput>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateUserReq {
    pub name: Option<String>,
    pub username: Option<String>,
    /// `active` | `invited` | `suspended` | `disabled`.
    pub status: Option<String>,
}

// ---- Profiles ----

#[derive(Serialize, schemars::JsonSchema)]
pub struct ProfileDto {
    pub legal_first_name: Option<String>,
    pub legal_middle_name: Option<String>,
    pub legal_last_name: Option<String>,
    pub preferred_name: Option<String>,
    pub date_of_birth: Option<String>,
    pub phone: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub city: Option<String>,
    pub region: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    /// Masked — only the last four are ever returned here.
    pub ssn_last4: Option<String>,
    pub gov_id_type: Option<String>,
    pub gov_id_last4: Option<String>,
    pub photo_url: Option<String>,
    pub has_ssn: bool,
    pub has_gov_id: bool,
}

impl From<entity::user_profile::Model> for ProfileDto {
    fn from(p: entity::user_profile::Model) -> Self {
        ProfileDto {
            legal_first_name: p.legal_first_name,
            legal_middle_name: p.legal_middle_name,
            legal_last_name: p.legal_last_name,
            preferred_name: p.preferred_name,
            date_of_birth: p.date_of_birth.map(|d| d.to_string()),
            phone: p.phone,
            address_line1: p.address_line1,
            address_line2: p.address_line2,
            city: p.city,
            region: p.region,
            postal_code: p.postal_code,
            country: p.country,
            ssn_last4: p.ssn_last4,
            gov_id_type: p.gov_id_type,
            gov_id_last4: p.gov_id_last4,
            photo_url: p.photo_url,
            has_ssn: p.ssn_ciphertext.is_some(),
            has_gov_id: p.gov_id_ciphertext.is_some(),
        }
    }
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct PiiReveal {
    pub ssn: Option<String>,
    pub gov_id_number: Option<String>,
}

// ---- Memberships & role assignment ----

#[derive(Serialize, schemars::JsonSchema)]
pub struct MembershipDto {
    pub id: Uuid,
    pub scope: String,
    pub tenant_id: Option<Uuid>,
    pub profile_type: String,
    pub title: Option<String>,
    pub status: String,
    pub is_primary: bool,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct AssignRoleReq {
    pub role_id: Uuid,
    pub tenant_id: Option<Uuid>,
    /// Coverage scope: `platform` | `tenant` | `entity` | `portfolio` | `property`.
    /// Defaults to `platform` when `tenant_id` is null, else `tenant`.
    pub scope: Option<String>,
    /// The entity/portfolio/property id when `scope` is narrower than `tenant`.
    pub scope_ref_id: Option<Uuid>,
}

// ---- User detail ----

#[derive(Serialize, schemars::JsonSchema)]
pub struct UserRoleDto {
    pub id: i64,
    pub role_id: Uuid,
    pub role_key: String,
    pub role_name: String,
    pub tenant_id: Option<Uuid>,
    /// Coverage scope: platform | tenant | entity | portfolio | property.
    pub scope: String,
    /// The entity/portfolio/property id when the scope is narrower than tenant.
    pub scope_ref_id: Option<Uuid>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct UserDetail {
    pub id: Uuid,
    pub email: String,
    pub username: Option<String>,
    pub name: String,
    pub status: String,
    pub is_platform_staff: bool,
    pub tenant_id: Option<Uuid>,
    pub profile: Option<ProfileDto>,
    pub memberships: Vec<MembershipDto>,
    pub roles: Vec<UserRoleDto>,
}

// ===========================================================================
// Tenant member management
// ===========================================================================

#[derive(Serialize, schemars::JsonSchema)]
pub struct MemberDto {
    pub membership_id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub email: String,
    pub profile_type: String,
    pub title: Option<String>,
    pub status: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct InviteMemberReq {
    pub email: String,
    pub name: String,
    /// Tenant persona, e.g. `property_manager`, `back_office`, `landlord`.
    pub profile_type: String,
    pub title: Option<String>,
}
