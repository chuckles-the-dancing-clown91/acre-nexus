//! Hierarchical **scope coverage** for role assignments.
//!
//! A role assignment is bound at a *scope* (see [`crate::rbac::scope`] keys). A
//! permission check passes if the user holds a role granting permission `P` at
//! **any** scope that *covers* the target resource. Coverage is hierarchical:
//!
//! ```text
//! platform ⊇ tenant ⊇ portfolio ⊇ property
//!                    ⊇ entity (LLC) ⊇ the properties that LLC holds title to
//! ```
//!
//! This is the single, centralized resolver the spec mandates (§11.6): handlers
//! must not scatter ad-hoc scope checks. [`scope_covers`] is a pure function over
//! a resource's [`ResourceScope`] chain (so it is cheap to unit-test); the
//! database-backed wrapper that builds the chain for a given resource lives in
//! `crate::tenancy::resolve` and is cached per request.

use uuid::Uuid;

/// Whole-platform admin surface (Acre HQ).
pub const SCOPE_PLATFORM: &str = "platform";
/// Everything inside one tenant (firm).
pub const SCOPE_TENANT: &str = "tenant";
/// One legal entity (LLC) — its books and the properties it holds title to.
pub const SCOPE_ENTITY: &str = "entity";
/// One portfolio — the properties grouped under it.
pub const SCOPE_PORTFOLIO: &str = "portfolio";
/// One specific property.
pub const SCOPE_PROPERTY: &str = "property";

/// The five legal scope keys, in widest-to-narrowest order.
pub const ALL_SCOPES: &[&str] = &[
    SCOPE_PLATFORM,
    SCOPE_TENANT,
    SCOPE_ENTITY,
    SCOPE_PORTFOLIO,
    SCOPE_PROPERTY,
];

/// Whether `scope` is a recognized scope key.
pub fn is_valid_scope(scope: &str) -> bool {
    ALL_SCOPES.contains(&scope)
}

/// Whether `scope` names a specific resource (and therefore requires a
/// `scope_ref_id`): entity / portfolio / property.
pub fn is_resource_scope(scope: &str) -> bool {
    matches!(scope, SCOPE_ENTITY | SCOPE_PORTFOLIO | SCOPE_PROPERTY)
}

/// The resolved scope chain of a resource: the (scope, id) pairs that a grant may
/// match to cover it. `tenant` membership is implied by the request's tenant and
/// always covers; narrower grants must match a specific id here.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResourceScope {
    /// The property itself, when the resource is (or belongs to) a property.
    pub property_id: Option<Uuid>,
    /// The portfolio the property is grouped under, if any.
    pub portfolio_id: Option<Uuid>,
    /// The legal entity (LLC) that holds title, if any.
    pub entity_id: Option<Uuid>,
}

impl ResourceScope {
    /// A resource scoped to a single property and its grouping/title chain.
    pub fn property(property_id: Uuid, portfolio_id: Option<Uuid>, entity_id: Option<Uuid>) -> Self {
        ResourceScope {
            property_id: Some(property_id),
            portfolio_id,
            entity_id,
        }
    }

    /// A resource scoped to a whole legal entity (its books / cap table).
    #[allow(dead_code)] // used by cap-table/banking scope checks as they adopt require_scoped
    pub fn entity(entity_id: Uuid) -> Self {
        ResourceScope {
            entity_id: Some(entity_id),
            ..Default::default()
        }
    }

    /// A resource scoped to a whole portfolio.
    #[allow(dead_code)] // used by portfolio scope checks as they adopt require_scoped
    pub fn portfolio(portfolio_id: Uuid) -> Self {
        ResourceScope {
            portfolio_id: Some(portfolio_id),
            ..Default::default()
        }
    }
}

/// Does a grant at `grant_scope` (optionally pinned to `grant_ref`) cover a
/// resource described by `resource`?
///
/// * `platform` and `tenant` grants cover everything (tenant isolation is the
///   separate RLS wall; within a tenant a `tenant`-scoped grant is firm-wide).
/// * `entity` / `portfolio` / `property` grants cover only when their pinned id
///   appears in the resource's chain.
pub fn scope_covers(grant_scope: &str, grant_ref: Option<Uuid>, resource: &ResourceScope) -> bool {
    match grant_scope {
        SCOPE_PLATFORM | SCOPE_TENANT => true,
        SCOPE_ENTITY => grant_ref.is_some() && grant_ref == resource.entity_id,
        SCOPE_PORTFOLIO => grant_ref.is_some() && grant_ref == resource.portfolio_id,
        SCOPE_PROPERTY => grant_ref.is_some() && grant_ref == resource.property_id,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    #[test]
    fn scope_validation() {
        assert!(is_valid_scope("tenant"));
        assert!(is_valid_scope("property"));
        assert!(!is_valid_scope("galaxy"));
        assert!(is_resource_scope("entity"));
        assert!(!is_resource_scope("tenant"));
        assert!(!is_resource_scope("platform"));
    }

    #[test]
    fn tenant_and_platform_grants_cover_everything() {
        let prop = ResourceScope::property(id(1), Some(id(2)), Some(id(3)));
        assert!(scope_covers(SCOPE_TENANT, None, &prop));
        assert!(scope_covers(SCOPE_PLATFORM, None, &prop));
    }

    #[test]
    fn entity_grant_covers_its_properties_only() {
        let in_entity = ResourceScope::property(id(1), None, Some(id(3)));
        let other_entity = ResourceScope::property(id(1), None, Some(id(9)));
        assert!(scope_covers(SCOPE_ENTITY, Some(id(3)), &in_entity));
        assert!(!scope_covers(SCOPE_ENTITY, Some(id(3)), &other_entity));
        // An entity grant with no id pinned covers nothing.
        assert!(!scope_covers(SCOPE_ENTITY, None, &in_entity));
    }

    #[test]
    fn portfolio_and_property_grants_match_their_id() {
        let prop = ResourceScope::property(id(1), Some(id(2)), Some(id(3)));
        assert!(scope_covers(SCOPE_PORTFOLIO, Some(id(2)), &prop));
        assert!(!scope_covers(SCOPE_PORTFOLIO, Some(id(99)), &prop));
        assert!(scope_covers(SCOPE_PROPERTY, Some(id(1)), &prop));
        assert!(!scope_covers(SCOPE_PROPERTY, Some(id(99)), &prop));
    }

    #[test]
    fn entity_resource_is_not_covered_by_property_grant() {
        let entity_resource = ResourceScope::entity(id(3));
        assert!(scope_covers(SCOPE_ENTITY, Some(id(3)), &entity_resource));
        assert!(!scope_covers(SCOPE_PROPERTY, Some(id(3)), &entity_resource));
    }
}
