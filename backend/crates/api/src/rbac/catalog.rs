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
