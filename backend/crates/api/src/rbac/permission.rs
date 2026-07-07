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
    /// Fee/discount/amenity schedule (the conditional-charge catalog).
    FeeRead,
    FeeManage,
    /// Resident vehicle profiles.
    VehicleRead,
    VehicleManage,
    /// Maintenance work orders / tickets.
    MaintenanceRead,
    MaintenanceManage,
    /// Resident ↔ manager messaging threads.
    MessageRead,
    MessageManage,
    /// Title: ownership (deed) + liens / encumbrances.
    TitleRead,
    TitleManage,
    ListingRead,
    ListingWrite,
    ApplicationRead,
    ApplicationWrite,
    /// View screening reports (FCRA consumer reports) — more sensitive than
    /// the application itself, so it's its own grant.
    ScreeningRead,
    ThemeWrite,
    ApiTokenManage,
    /// Integration credentials + integrations settings (write-only: set /
    /// rotate / delete — plaintext is never read back).
    IntegrationsManage,
    /// Documents: list + download via signed URL.
    DocumentRead,
    /// Documents: upload, version, delete.
    DocumentManage,
    BillingRead,
    /// General ledger: chart of accounts, journal, financial reports.
    LedgerRead,
    /// Post manual journal entries and manage the chart of accounts.
    LedgerManage,
    /// Payments: rent collection status, methods, receipts, reconciliation.
    PaymentRead,
    /// Collect payments, manage methods/autopay, match bank transactions.
    PaymentManage,
    /// Compute and execute owner payouts / draws.
    PayoutManage,
    /// Accounts payable: view vendor bills and their history.
    PayableRead,
    /// Create, edit, and submit vendor bills.
    PayableManage,
    /// Approve submitted vendor bills and execute their payment.
    PayableApprove,
    /// Calendar & reminders: view the aggregated schedule (also the audience
    /// for reminder notifications).
    CalendarRead,
    /// Create, edit, complete, and cancel reminders.
    CalendarManage,
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
            Permission::FeeRead => "fee:read",
            Permission::FeeManage => "fee:manage",
            Permission::VehicleRead => "vehicle:read",
            Permission::VehicleManage => "vehicle:manage",
            Permission::MaintenanceRead => "maintenance:read",
            Permission::MaintenanceManage => "maintenance:manage",
            Permission::MessageRead => "message:read",
            Permission::MessageManage => "message:manage",
            Permission::TitleRead => "title:read",
            Permission::TitleManage => "title:manage",
            Permission::ListingRead => "listing:read",
            Permission::ListingWrite => "listing:write",
            Permission::ApplicationRead => "application:read",
            Permission::ApplicationWrite => "application:write",
            Permission::ScreeningRead => "screening:read",
            Permission::ThemeWrite => "theme:write",
            Permission::ApiTokenManage => "apitoken:manage",
            Permission::IntegrationsManage => "integrations:manage",
            Permission::DocumentRead => "document:read",
            Permission::DocumentManage => "document:manage",
            Permission::BillingRead => "billing:read",
            Permission::LedgerRead => "ledger:read",
            Permission::LedgerManage => "ledger:manage",
            Permission::PaymentRead => "payment:read",
            Permission::PaymentManage => "payment:manage",
            Permission::PayoutManage => "payout:manage",
            Permission::PayableRead => "payable:read",
            Permission::PayableManage => "payable:manage",
            Permission::PayableApprove => "payable:approve",
            Permission::CalendarRead => "calendar:read",
            Permission::CalendarManage => "calendar:manage",
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
    FeeRead,
    FeeManage,
    VehicleRead,
    VehicleManage,
    MaintenanceRead,
    MaintenanceManage,
    MessageRead,
    MessageManage,
    TitleRead,
    TitleManage,
    ListingRead,
    ListingWrite,
    ApplicationRead,
    ApplicationWrite,
    ScreeningRead,
    ThemeWrite,
    ApiTokenManage,
    IntegrationsManage,
    DocumentRead,
    DocumentManage,
    BillingRead,
    LedgerRead,
    LedgerManage,
    PaymentRead,
    PaymentManage,
    PayoutManage,
    PayableRead,
    PayableManage,
    PayableApprove,
    CalendarRead,
    CalendarManage,
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
