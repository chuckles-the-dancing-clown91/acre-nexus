//! Role-based access control: the permission catalog, persona (profile-type)
//! catalog, and the seeded system roles that wire them together.
//!
//! ## Data-driven by design
//! Permissions are fine-grained `resource:action` strings persisted in
//! `role_permission` and resolved per user at login (see
//! [`crate::auth::permissions_for`]). The [`Permission`] enum below is a
//! compile-time convenience for the keys the Rust handlers check directly; the
//! Acre admin dashboard can additionally create roles and grant **any** catalog
//! permission at runtime without code changes.
//!
//! ## Three layers
//! * [`PERMISSION_CATALOG`] — every permission the platform ships, with UI
//!   metadata. Seeded into the `permission` table so the dashboard can offer a
//!   picker (and so custom permissions can be added later).
//! * [`PROFILE_TYPES`] — the personas a membership can have (Acre employees vs
//!   client landlords/back-office/renters …). Seeded into `profile_type`.
//! * [`SYSTEM_ROLES`] — default roles, each a named bundle of permissions at a
//!   `platform` or `tenant` scope. Seeded into `role` + `role_permission`.

use std::collections::HashSet;

/// Every permission the platform understands at compile time. `as_str` is the
/// persisted/JWT form. New *custom* permissions may also exist in the DB without
/// a variant here — handlers gate on those via [`crate::auth::AuthUser::require_key`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permission {
    // Property / leasing domain.
    PropertyRead,
    PropertyWrite,
    ListingRead,
    ListingWrite,
    ApplicationRead,
    ApplicationWrite,
    ThemeWrite,
    ApiTokenManage,
    BillingRead,
    // Tenant administration.
    TenantManage,
    // Identity & access management (the user/RBAC system).
    UserRead,
    UserManage,
    ProfileRead,
    ProfileWrite,
    /// Decrypt and view sensitive PII (SSN, government IDs). Highly privileged.
    ProfilePiiRead,
    MemberRead,
    MemberManage,
    RoleRead,
    RoleManage,
    /// View the security audit log.
    AuditRead,
    /// Cross-tenant platform administration (Acre HQ staff only) — implies all.
    PlatformAdmin,
}

impl Permission {
    pub fn as_str(self) -> &'static str {
        match self {
            Permission::PropertyRead => "property:read",
            Permission::PropertyWrite => "property:write",
            Permission::ListingRead => "listing:read",
            Permission::ListingWrite => "listing:write",
            Permission::ApplicationRead => "application:read",
            Permission::ApplicationWrite => "application:write",
            Permission::ThemeWrite => "theme:write",
            Permission::ApiTokenManage => "apitoken:manage",
            Permission::BillingRead => "billing:read",
            Permission::TenantManage => "tenant:manage",
            Permission::UserRead => "user:read",
            Permission::UserManage => "user:manage",
            Permission::ProfileRead => "profile:read",
            Permission::ProfileWrite => "profile:write",
            Permission::ProfilePiiRead => "profile:read_pii",
            Permission::MemberRead => "member:read",
            Permission::MemberManage => "member:manage",
            Permission::RoleRead => "role:read",
            Permission::RoleManage => "role:manage",
            Permission::AuditRead => "audit:read",
            Permission::PlatformAdmin => "platform:admin",
        }
    }
}

use Permission::*;

/// All permissions — convenience for the super-admin role.
pub const ALL_PERMS: &[Permission] = &[
    PropertyRead,
    PropertyWrite,
    ListingRead,
    ListingWrite,
    ApplicationRead,
    ApplicationWrite,
    ThemeWrite,
    ApiTokenManage,
    BillingRead,
    TenantManage,
    UserRead,
    UserManage,
    ProfileRead,
    ProfileWrite,
    ProfilePiiRead,
    MemberRead,
    MemberManage,
    RoleRead,
    RoleManage,
    AuditRead,
    PlatformAdmin,
];

/// Scope at which a role / permission / persona applies.
pub const SCOPE_PLATFORM: &str = "platform";
pub const SCOPE_TENANT: &str = "tenant";

// ---------------------------------------------------------------------------
// Permission catalog (seeded into `permission`)
// ---------------------------------------------------------------------------

/// UI/seed metadata for one permission.
pub struct PermissionMeta {
    pub key: &'static str,
    pub category: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    /// `platform`, `tenant`, or `both`.
    pub scope: &'static str,
}

/// The full catalog, grouped by category. Seeded so the dashboard can present a
/// permission picker; custom permissions can be appended in the DB later.
pub const PERMISSION_CATALOG: &[PermissionMeta] = &[
    PermissionMeta {
        key: "property:read",
        category: "Properties",
        label: "View properties",
        description: "View the portfolio and property profiles.",
        scope: "tenant",
    },
    PermissionMeta {
        key: "property:write",
        category: "Properties",
        label: "Edit properties",
        description: "Create and edit properties.",
        scope: "tenant",
    },
    PermissionMeta {
        key: "listing:read",
        category: "Leasing",
        label: "View listings",
        description: "View public listings.",
        scope: "tenant",
    },
    PermissionMeta {
        key: "listing:write",
        category: "Leasing",
        label: "Edit listings",
        description: "Create and edit listings.",
        scope: "tenant",
    },
    PermissionMeta {
        key: "application:read",
        category: "Leasing",
        label: "View applications",
        description: "View rental applications.",
        scope: "tenant",
    },
    PermissionMeta {
        key: "application:write",
        category: "Leasing",
        label: "Manage applications",
        description: "Advance / decide on applications.",
        scope: "tenant",
    },
    PermissionMeta {
        key: "theme:write",
        category: "Settings",
        label: "Edit branding",
        description: "Edit white-label branding and legal templates.",
        scope: "tenant",
    },
    PermissionMeta {
        key: "apitoken:manage",
        category: "Settings",
        label: "Manage API tokens",
        description: "Issue and revoke vendor API tokens.",
        scope: "tenant",
    },
    PermissionMeta {
        key: "billing:read",
        category: "Billing",
        label: "View billing",
        description: "View invoices and plan usage.",
        scope: "both",
    },
    PermissionMeta {
        key: "tenant:manage",
        category: "Settings",
        label: "Manage workspace",
        description: "Edit workspace settings and modules.",
        scope: "tenant",
    },
    PermissionMeta {
        key: "user:read",
        category: "Access",
        label: "View users",
        description: "View user accounts.",
        scope: "both",
    },
    PermissionMeta {
        key: "user:manage",
        category: "Access",
        label: "Manage users",
        description: "Create, edit, suspend user accounts.",
        scope: "both",
    },
    PermissionMeta {
        key: "profile:read",
        category: "Access",
        label: "View profiles",
        description: "View user profiles (non-sensitive fields).",
        scope: "both",
    },
    PermissionMeta {
        key: "profile:write",
        category: "Access",
        label: "Edit profiles",
        description: "Edit user profile details.",
        scope: "both",
    },
    PermissionMeta {
        key: "profile:read_pii",
        category: "Access",
        label: "View sensitive PII",
        description: "Decrypt and view SSN / government IDs. Highly privileged.",
        scope: "both",
    },
    PermissionMeta {
        key: "member:read",
        category: "Access",
        label: "View members",
        description: "View workspace members and personas.",
        scope: "tenant",
    },
    PermissionMeta {
        key: "member:manage",
        category: "Access",
        label: "Manage members",
        description: "Invite, assign personas, and remove members.",
        scope: "tenant",
    },
    PermissionMeta {
        key: "role:read",
        category: "Access",
        label: "View roles",
        description: "View roles and their permissions.",
        scope: "both",
    },
    PermissionMeta {
        key: "role:manage",
        category: "Access",
        label: "Manage roles",
        description: "Create roles and edit their permissions.",
        scope: "both",
    },
    PermissionMeta {
        key: "audit:read",
        category: "Access",
        label: "View audit log",
        description: "View the security audit trail (PII reveals, role/user changes).",
        scope: "both",
    },
    PermissionMeta {
        key: "platform:admin",
        category: "Platform",
        label: "Platform administrator",
        description: "Full cross-tenant administration (Acre HQ). Implies every permission.",
        scope: "platform",
    },
];

// ---------------------------------------------------------------------------
// Persona catalog (seeded into `profile_type`)
// ---------------------------------------------------------------------------

/// A persona / "profile type": what kind of actor a membership represents.
pub struct ProfileTypeMeta {
    pub key: &'static str,
    pub scope: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    /// The default system-role key granted when a member is created with this persona.
    pub default_role: &'static str,
}

/// All personas. Platform personas are Acre employees; tenant personas are the
/// people inside a client workspace (and the renters they serve).
pub const PROFILE_TYPES: &[ProfileTypeMeta] = &[
    ProfileTypeMeta {
        key: "acre_admin",
        scope: "platform",
        label: "Acre Admin",
        description: "Acre HQ administrator with full platform access.",
        default_role: "acre_admin",
    },
    ProfileTypeMeta {
        key: "acre_account_manager",
        scope: "platform",
        label: "Account Manager",
        description: "Manages client accounts and onboarding.",
        default_role: "acre_account_manager",
    },
    ProfileTypeMeta {
        key: "acre_support",
        scope: "platform",
        label: "Support Agent",
        description: "Assists clients; can view-as a tenant.",
        default_role: "acre_support",
    },
    ProfileTypeMeta {
        key: "acre_billing",
        scope: "platform",
        label: "Billing Specialist",
        description: "Manages plans, invoices, and billing.",
        default_role: "acre_billing",
    },
    ProfileTypeMeta {
        key: "acre_read_only",
        scope: "platform",
        label: "Acre Read-only",
        description: "Read-only platform access (audit / analyst).",
        default_role: "acre_read_only",
    },
    ProfileTypeMeta {
        key: "tenant_owner",
        scope: "tenant",
        label: "Workspace Owner",
        description: "Owns the client account; full workspace control.",
        default_role: "tenant_owner",
    },
    ProfileTypeMeta {
        key: "property_manager",
        scope: "tenant",
        label: "Property Manager",
        description: "Runs day-to-day property and leasing operations.",
        default_role: "property_manager",
    },
    ProfileTypeMeta {
        key: "back_office",
        scope: "tenant",
        label: "Back-office Staff",
        description: "Applications, billing, and administrative work.",
        default_role: "back_office",
    },
    ProfileTypeMeta {
        key: "leasing_agent",
        scope: "tenant",
        label: "Leasing Agent",
        description: "Manages listings and applicant pipeline.",
        default_role: "leasing_agent",
    },
    ProfileTypeMeta {
        key: "maintenance",
        scope: "tenant",
        label: "Maintenance",
        description: "Views properties and work orders.",
        default_role: "maintenance",
    },
    ProfileTypeMeta {
        key: "landlord",
        scope: "tenant",
        label: "Landlord / Owner",
        description: "Property owner the workspace manages on behalf of.",
        default_role: "landlord",
    },
    ProfileTypeMeta {
        key: "renter",
        scope: "tenant",
        label: "Renter",
        description: "A resident / applicant using the renter portal.",
        default_role: "renter",
    },
];

/// Resolve the default system-role key for a persona key, if any.
pub fn default_role_for_persona(persona: &str) -> Option<&'static str> {
    PROFILE_TYPES
        .iter()
        .find(|p| p.key == persona)
        .map(|p| p.default_role)
}

// ---------------------------------------------------------------------------
// System roles (seeded into `role` + `role_permission`)
// ---------------------------------------------------------------------------

/// A built-in role definition: a named permission bundle at a given scope.
pub struct SystemRole {
    pub key: &'static str,
    pub scope: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub permissions: &'static [Permission],
}

/// Tenant-admin permission bundle (everything within a workspace).
const TENANT_FULL: &[Permission] = &[
    PropertyRead,
    PropertyWrite,
    ListingRead,
    ListingWrite,
    ApplicationRead,
    ApplicationWrite,
    ThemeWrite,
    ApiTokenManage,
    BillingRead,
    TenantManage,
    UserRead,
    ProfileRead,
    ProfileWrite,
    MemberRead,
    MemberManage,
    RoleRead,
    RoleManage,
];

/// The seeded system roles. Acre personas are platform-scoped; client personas
/// are tenant-scoped. The dashboard can clone or extend these.
pub const SYSTEM_ROLES: &[SystemRole] = &[
    // ---- Platform (Acre HQ) ----
    SystemRole {
        key: "acre_admin",
        scope: SCOPE_PLATFORM,
        name: "Acre Admin",
        description: "Full cross-tenant platform administration.",
        permissions: ALL_PERMS,
    },
    SystemRole {
        key: "acre_account_manager",
        scope: SCOPE_PLATFORM,
        name: "Account Manager",
        description: "Manage client accounts, users, and access.",
        permissions: &[
            TenantManage,
            UserRead,
            UserManage,
            ProfileRead,
            MemberRead,
            MemberManage,
            RoleRead,
            BillingRead,
            PropertyRead,
            ApplicationRead,
            AuditRead,
        ],
    },
    SystemRole {
        key: "acre_support",
        scope: SCOPE_PLATFORM,
        name: "Support Agent",
        description: "Assist clients; view workspaces and users.",
        permissions: &[
            UserRead,
            ProfileRead,
            MemberRead,
            RoleRead,
            PropertyRead,
            ListingRead,
            ApplicationRead,
            TenantManage,
        ],
    },
    SystemRole {
        key: "acre_billing",
        scope: SCOPE_PLATFORM,
        name: "Billing Specialist",
        description: "Manage plans, invoices, and billing.",
        permissions: &[BillingRead, UserRead, ProfileRead],
    },
    SystemRole {
        key: "acre_read_only",
        scope: SCOPE_PLATFORM,
        name: "Acre Read-only",
        description: "Read-only platform access for audit / analytics.",
        permissions: &[
            UserRead,
            ProfileRead,
            MemberRead,
            RoleRead,
            PropertyRead,
            ListingRead,
            ApplicationRead,
            BillingRead,
            AuditRead,
        ],
    },
    // ---- Tenant (client workspace) ----
    SystemRole {
        key: "tenant_owner",
        scope: SCOPE_TENANT,
        name: "Workspace Owner",
        description: "Full control of the client workspace.",
        permissions: TENANT_FULL,
    },
    SystemRole {
        key: "property_manager",
        scope: SCOPE_TENANT,
        name: "Property Manager",
        description: "Day-to-day property and leasing operations.",
        permissions: &[
            PropertyRead,
            PropertyWrite,
            ListingRead,
            ListingWrite,
            ApplicationRead,
            ApplicationWrite,
            MemberRead,
            RoleRead,
            ThemeWrite,
        ],
    },
    SystemRole {
        key: "back_office",
        scope: SCOPE_TENANT,
        name: "Back-office Staff",
        description: "Applications, billing, and administrative work.",
        permissions: &[
            PropertyRead,
            ApplicationRead,
            ApplicationWrite,
            BillingRead,
            MemberRead,
            UserRead,
            ProfileRead,
        ],
    },
    SystemRole {
        key: "leasing_agent",
        scope: SCOPE_TENANT,
        name: "Leasing Agent",
        description: "Manage listings and the applicant pipeline.",
        permissions: &[
            PropertyRead,
            ListingRead,
            ListingWrite,
            ApplicationRead,
            ApplicationWrite,
        ],
    },
    SystemRole {
        key: "maintenance",
        scope: SCOPE_TENANT,
        name: "Maintenance",
        description: "View properties and work orders.",
        permissions: &[PropertyRead],
    },
    SystemRole {
        key: "landlord",
        scope: SCOPE_TENANT,
        name: "Landlord / Owner",
        description: "Owner view of their managed properties and leasing.",
        permissions: &[
            PropertyRead,
            PropertyWrite,
            ListingRead,
            ListingWrite,
            ApplicationRead,
        ],
    },
    SystemRole {
        key: "renter",
        scope: SCOPE_TENANT,
        name: "Renter",
        description: "Resident / applicant portal access.",
        permissions: &[ListingRead],
    },
];

// ---------------------------------------------------------------------------
// Resolved grants on an authenticated principal
// ---------------------------------------------------------------------------

/// Resolved permission set carried on an authenticated principal.
#[derive(Clone, Debug, Default)]
pub struct Grants(pub HashSet<String>);

impl Grants {
    pub fn from_iter<I: IntoIterator<Item = String>>(it: I) -> Self {
        Grants(it.into_iter().collect())
    }

    /// Whether the principal holds permission `p` (platform admins hold all).
    #[allow(dead_code)] // public convenience; handlers go through `require`/`has_key`.
    pub fn has(&self, p: Permission) -> bool {
        self.has_key(p.as_str())
    }

    /// String-keyed check, for dynamic/custom permissions not in [`Permission`].
    pub fn has_key(&self, key: &str) -> bool {
        self.0.contains(Permission::PlatformAdmin.as_str()) || self.0.contains(key)
    }
}
