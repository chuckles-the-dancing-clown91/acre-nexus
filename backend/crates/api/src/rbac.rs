//! Role-based access control.
//!
//! Permissions are fine-grained `resource:action` strings. Roles bundle them.
//! The built-in system roles below are seeded for every deployment; tenants may
//! additionally define custom roles in the `role` table.

use std::collections::HashSet;

/// Every permission the platform understands. The string form (`as_str`) is what
/// gets persisted in `role_permission` and embedded in JWT claims / API-token scopes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permission {
    PropertyRead,
    PropertyWrite,
    ListingRead,
    ListingWrite,
    ApplicationRead,
    ApplicationWrite,
    TenantManage,
    BillingRead,
    ThemeWrite,
    ApiTokenManage,
    /// Cross-tenant platform administration (Acre HQ staff only).
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
            Permission::TenantManage => "tenant:manage",
            Permission::BillingRead => "billing:read",
            Permission::ThemeWrite => "theme:write",
            Permission::ApiTokenManage => "apitoken:manage",
            Permission::PlatformAdmin => "platform:admin",
        }
    }
}

/// A built-in role definition: `(key, name, description, permissions)`.
pub struct SystemRole {
    pub key: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub permissions: &'static [Permission],
}

use Permission::*;

/// All permissions — convenience for the platform-admin role.
pub const ALL_PERMS: &[Permission] = &[
    PropertyRead,
    PropertyWrite,
    ListingRead,
    ListingWrite,
    ApplicationRead,
    ApplicationWrite,
    TenantManage,
    BillingRead,
    ThemeWrite,
    ApiTokenManage,
    PlatformAdmin,
];

/// The seeded system roles, mirroring the prototype's six perspectives.
pub const SYSTEM_ROLES: &[SystemRole] = &[
    SystemRole {
        key: "platform_admin",
        name: "Platform Admin",
        description: "Acre HQ staff — full cross-tenant access.",
        permissions: ALL_PERMS,
    },
    SystemRole {
        key: "pm_admin",
        name: "Property Manager Admin",
        description: "Client company admin — full access within their tenant.",
        permissions: &[
            PropertyRead,
            PropertyWrite,
            ListingRead,
            ListingWrite,
            ApplicationRead,
            ApplicationWrite,
            TenantManage,
            BillingRead,
            ThemeWrite,
            ApiTokenManage,
        ],
    },
    SystemRole {
        key: "landlord",
        name: "Landlord",
        description: "Manage own properties, listings and applications.",
        permissions: &[
            PropertyRead,
            PropertyWrite,
            ListingRead,
            ListingWrite,
            ApplicationRead,
            ApplicationWrite,
        ],
    },
    SystemRole {
        key: "maintenance",
        name: "Maintenance",
        description: "View properties and work orders.",
        permissions: &[PropertyRead],
    },
    SystemRole {
        key: "tenant",
        name: "Tenant",
        description: "Renter portal access.",
        permissions: &[ListingRead],
    },
];

/// Resolved permission set carried on an authenticated principal.
#[derive(Clone, Debug, Default)]
pub struct Grants(pub HashSet<String>);

impl Grants {
    pub fn from_iter<I: IntoIterator<Item = String>>(it: I) -> Self {
        Grants(it.into_iter().collect())
    }

    pub fn has(&self, p: Permission) -> bool {
        // Platform admins implicitly hold every permission.
        self.0.contains(Permission::PlatformAdmin.as_str()) || self.0.contains(p.as_str())
    }
}
