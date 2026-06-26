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
