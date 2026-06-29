//! Per-resource **scope resolution** — the database-backed half of the scoped
//! RBAC the spec mandates (§3.2, §11.6). [`crate::rbac::scope_covers`] is the
//! pure coverage rule; this module builds a resource's [`ResourceScope`] chain
//! and answers "may this principal do `P` *here*?" by combining the principal's
//! workspace-wide grants (already in the JWT) with any narrower scoped grants.

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::scope::{scope_covers, ResourceScope};
use crate::rbac::Permission;
use entity::prelude::{Property, RolePermission, UserRole};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

/// Build the scope chain for a property (its grouping + title entity).
#[allow(dead_code)] // convenience for handlers that load a property id without the row
pub async fn property_scope(
    db: &DatabaseConnection,
    property_id: Uuid,
) -> ApiResult<Option<ResourceScope>> {
    Ok(Property::find_by_id(property_id)
        .one(db)
        .await?
        .map(|p| ResourceScope::property(p.id, p.portfolio_id, p.llc_id)))
}

/// Assert `user` may exercise `perm` on the resource described by `resource`.
///
/// Passes when the principal holds `perm` workspace-wide (the flat JWT grant set,
/// which already includes the `platform:admin` super-permission), **or** holds a
/// narrower scoped role assignment that grants `perm` and whose scope covers the
/// resource. Otherwise `403`.
pub async fn require_scoped(
    db: &DatabaseConnection,
    user: &AuthUser,
    perm: Permission,
    resource: &ResourceScope,
) -> ApiResult<()> {
    // Workspace-wide / platform grants already resolved into the token.
    if user.grants.has_key(perm.as_str()) {
        return Ok(());
    }

    // Otherwise look for a narrower scoped grant covering this resource.
    let Some(active_tenant) = user.tenant_id else {
        return Err(forbidden(perm));
    };
    let assignments = UserRole::find()
        .filter(entity::user_role::Column::UserId.eq(user.user_id))
        .all(db)
        .await?;

    for a in assignments {
        // Only resource-scoped grants in the active workspace are relevant here.
        if a.tenant_id != Some(active_tenant) {
            continue;
        }
        if !crate::rbac::scope::is_resource_scope(&a.scope) {
            continue;
        }
        if !scope_covers(&a.scope, a.scope_ref_id, resource) {
            continue;
        }
        // Does this assignment's role actually grant the permission?
        let granted = RolePermission::find()
            .filter(entity::role_permission::Column::RoleId.eq(a.role_id))
            .filter(entity::role_permission::Column::Permission.eq(perm.as_str()))
            .one(db)
            .await?
            .is_some();
        if granted {
            return Ok(());
        }
    }
    Err(forbidden(perm))
}

fn forbidden(perm: Permission) -> ApiError {
    ApiError::Forbidden(format!("missing permission: {}", perm.as_str()))
}
