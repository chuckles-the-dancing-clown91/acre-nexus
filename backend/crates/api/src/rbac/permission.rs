/// Every permission the platform understands at compile time. `as_str` is the
/// persisted/JWT form. New *custom* permissions may also exist in the DB without
/// a variant here — handlers gate on those via [`crate::auth::AuthUser::require_key`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permission {
    // Property / leasing domain.
    PropertyRead,
    PropertyWrite,
    /// Entities/counterparty registry (banks, lenders, contractors …).
    EntityRead,
    EntityManage,
    /// Property financing (mortgages / loans).
    FinanceRead,
    FinanceManage,
    /// Rentals: units, leases/tenancies, rent ledger.
    LeaseRead,
    LeaseManage,
    /// Maintenance work orders / tickets.
    MaintenanceRead,
    MaintenanceManage,
    /// Title: ownership (deed) + liens / encumbrances.
    TitleRead,
    TitleManage,
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
    /// White-label routing: custom domains & audiences.
    DomainRead,
    DomainManage,
    /// Begin an audited, time-boxed impersonation session into a tenant (staff).
    ImpersonateTenant,
    /// Cross-tenant platform administration (Acre HQ staff only) — implies all.
    PlatformAdmin,
}

impl Permission {
    pub fn as_str(self) -> &'static str {
        match self {
            Permission::PropertyRead => "property:read",
            Permission::PropertyWrite => "property:write",
            Permission::EntityRead => "entity:read",
            Permission::EntityManage => "entity:manage",
            Permission::FinanceRead => "finance:read",
            Permission::FinanceManage => "finance:manage",
            Permission::LeaseRead => "lease:read",
            Permission::LeaseManage => "lease:manage",
            Permission::MaintenanceRead => "maintenance:read",
            Permission::MaintenanceManage => "maintenance:manage",
            Permission::TitleRead => "title:read",
            Permission::TitleManage => "title:manage",
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
            Permission::DomainRead => "domain:read",
            Permission::DomainManage => "domain:manage",
            Permission::ImpersonateTenant => "platform:impersonate",
            Permission::PlatformAdmin => "platform:admin",
        }
    }
}

use Permission::*;

/// All permissions — convenience for the super-admin role.
pub const ALL_PERMS: &[Permission] = &[
    PropertyRead,
    PropertyWrite,
    EntityRead,
    EntityManage,
    FinanceRead,
    FinanceManage,
    LeaseRead,
    LeaseManage,
    MaintenanceRead,
    MaintenanceManage,
    TitleRead,
    TitleManage,
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
    DomainRead,
    DomainManage,
    ImpersonateTenant,
    PlatformAdmin,
];

/// Scope at which a role / permission / persona applies.
pub const SCOPE_PLATFORM: &str = "platform";
pub const SCOPE_TENANT: &str = "tenant";
