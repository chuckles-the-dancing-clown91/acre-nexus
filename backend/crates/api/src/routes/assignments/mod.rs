//! **Staff assignments** — attach a person (property manager, landlord,
//! maintenance, leasing agent, back-office) to a property or legal entity (LLC),
//! and grant them scoped access in the same step.
//!
//! An assignment is both a directory relationship *and* an access grant: creating
//! one resolves the `relationship` (a tenant role key) to the matching system
//! role and adds a `user_role` grant scoped to that property/LLC
//! (`property:{id}` / `entity:{id}`), so [`crate::rbac::scope`] coverage lets the
//! person act on it (an LLC grant automatically covers every property that LLC
//! holds title to). Removing the assignment revokes that grant.
//!
//! The per-resource handlers ([`property`], [`entity`]) are thin: they validate
//! the subject belongs to the tenant, then call the shared helpers here.

pub mod llc;
pub mod property;

use crate::error::{ApiError, ApiResult};
use crate::rbac::scope::{SCOPE_ENTITY, SCOPE_PROPERTY};
use chrono::Utc;
use entity::prelude::{Assignment, Membership, Property, Role, User, UserRole};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Subject kinds an assignment can target.
pub const SUBJECT_PROPERTY: &str = "property";
pub const SUBJECT_ENTITY: &str = "entity";

/// The relationships a person can be assigned as, paired with a display label.
/// Each maps to a seeded tenant [`Role`] of the same key, which is what gets
/// granted at the subject's scope. `tenant_owner` / `renter` are intentionally
/// not assignable this way.
pub const ASSIGNABLE: &[(&str, &str)] = &[
    ("property_manager", "Property Manager"),
    ("landlord", "Landlord / Owner"),
    ("maintenance", "Maintenance"),
    ("leasing_agent", "Leasing Agent"),
    ("back_office", "Back-office Staff"),
];

fn relationship_label(key: &str) -> String {
    ASSIGNABLE
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, label)| (*label).to_string())
        .unwrap_or_else(|| key.to_string())
}

/// The `user_role` coverage scope a subject grants at.
fn grant_scope_for(subject_type: &str) -> &'static str {
    match subject_type {
        SUBJECT_ENTITY => SCOPE_ENTITY,
        _ => SCOPE_PROPERTY,
    }
}

/// Request body to create an assignment on a property or entity.
#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateAssignmentReq {
    /// The user being assigned (must be a member of the workspace).
    pub user_id: Uuid,
    /// A relationship key from [`ASSIGNABLE`].
    pub relationship: String,
    #[serde(default)]
    pub is_primary: bool,
    pub title: Option<String>,
    pub notes: Option<String>,
}

/// A resolved assignment for API responses (enriched with the person's name).
#[derive(Serialize, schemars::JsonSchema)]
pub struct AssignmentDto {
    pub id: Uuid,
    pub subject_type: String,
    pub subject_id: Uuid,
    pub user_id: Uuid,
    pub user_name: String,
    pub user_email: String,
    pub relationship: String,
    pub relationship_label: String,
    pub role_id: Option<Uuid>,
    pub is_primary: bool,
    pub title: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
}

impl AssignmentDto {
    fn from(model: entity::assignment::Model, user_name: String, user_email: String) -> Self {
        AssignmentDto {
            relationship_label: relationship_label(&model.relationship),
            id: model.id,
            subject_type: model.subject_type,
            subject_id: model.subject_id,
            user_id: model.user_id,
            user_name,
            user_email,
            relationship: model.relationship,
            role_id: model.role_id,
            is_primary: model.is_primary,
            title: model.title,
            notes: model.notes,
            created_at: model.created_at.to_rfc3339(),
        }
    }
}

/// List assignments for one subject, newest first, enriched with the person's
/// name/email.
pub(crate) async fn list_for_subject(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    subject_type: &str,
    subject_id: Uuid,
) -> ApiResult<Vec<AssignmentDto>> {
    let rows = Assignment::find()
        .filter(entity::assignment::Column::TenantId.eq(tenant_id))
        .filter(entity::assignment::Column::SubjectType.eq(subject_type))
        .filter(entity::assignment::Column::SubjectId.eq(subject_id))
        .order_by_desc(entity::assignment::Column::CreatedAt)
        .all(db)
        .await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let (name, email) = match User::find_by_id(r.user_id).one(db).await? {
            Some(u) => (u.name, u.email),
            None => (String::new(), String::new()),
        };
        out.push(AssignmentDto::from(r, name, email));
    }
    Ok(out)
}

/// Create (or update, if it already exists) an assignment and its scoped access
/// grant. Shared by the property/entity handlers and by onboarding.
pub(crate) async fn create_assignment_inner(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    actor: Uuid,
    subject_type: &str,
    subject_id: Uuid,
    req: &CreateAssignmentReq,
) -> ApiResult<AssignmentDto> {
    // Relationship must be one we grant a role for.
    if !ASSIGNABLE.iter().any(|(k, _)| *k == req.relationship) {
        return Err(ApiError::BadRequest(format!(
            "invalid relationship: {}",
            req.relationship
        )));
    }

    // The person must be a member of this workspace.
    let assignee = User::find_by_id(req.user_id)
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("user not found".into()))?;
    let is_member = Membership::find()
        .filter(entity::membership::Column::UserId.eq(req.user_id))
        .filter(entity::membership::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .is_some();
    if !is_member {
        return Err(ApiError::BadRequest(
            "user is not a member of this workspace".into(),
        ));
    }

    // Resolve the tenant role to grant for this relationship.
    let role = Role::find()
        .filter(entity::role::Column::Key.eq(req.relationship.clone()))
        .filter(entity::role::Column::Scope.eq(crate::rbac::scope::SCOPE_TENANT))
        .one(db)
        .await?;
    let role_id = role.as_ref().map(|r| r.id);

    let scope = grant_scope_for(subject_type);
    let now = Utc::now();

    // A new primary demotes any existing primary for the same relationship.
    if req.is_primary {
        let existing_primary = Assignment::find()
            .filter(entity::assignment::Column::TenantId.eq(tenant_id))
            .filter(entity::assignment::Column::SubjectType.eq(subject_type))
            .filter(entity::assignment::Column::SubjectId.eq(subject_id))
            .filter(entity::assignment::Column::Relationship.eq(req.relationship.clone()))
            .filter(entity::assignment::Column::IsPrimary.eq(true))
            .all(db)
            .await?;
        for p in existing_primary {
            let mut am: entity::assignment::ActiveModel = p.into();
            am.is_primary = Set(false);
            am.updated_at = Set(now.into());
            am.update(db).await?;
        }
    }

    // Upsert: one row per (subject, user, relationship).
    let existing = Assignment::find()
        .filter(entity::assignment::Column::TenantId.eq(tenant_id))
        .filter(entity::assignment::Column::SubjectType.eq(subject_type))
        .filter(entity::assignment::Column::SubjectId.eq(subject_id))
        .filter(entity::assignment::Column::UserId.eq(req.user_id))
        .filter(entity::assignment::Column::Relationship.eq(req.relationship.clone()))
        .one(db)
        .await?;

    let saved = match existing {
        Some(row) => {
            let mut am: entity::assignment::ActiveModel = row.into();
            am.is_primary = Set(req.is_primary);
            am.title = Set(req.title.clone());
            am.notes = Set(req.notes.clone());
            am.role_id = Set(role_id);
            am.updated_at = Set(now.into());
            am.update(db).await?
        }
        None => {
            entity::assignment::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                subject_type: Set(subject_type.to_string()),
                subject_id: Set(subject_id),
                user_id: Set(req.user_id),
                relationship: Set(req.relationship.clone()),
                role_id: Set(role_id),
                is_primary: Set(req.is_primary),
                title: Set(req.title.clone()),
                notes: Set(req.notes.clone()),
                assigned_by: Set(Some(actor)),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }
            .insert(db)
            .await?
        }
    };

    // Grant scoped access (idempotent): assign the resolved role at this subject's
    // scope if the user doesn't already hold that exact grant.
    if let Some(rid) = role_id {
        let already = UserRole::find()
            .filter(entity::user_role::Column::UserId.eq(req.user_id))
            .filter(entity::user_role::Column::RoleId.eq(rid))
            .filter(entity::user_role::Column::TenantId.eq(tenant_id))
            .filter(entity::user_role::Column::Scope.eq(scope))
            .filter(entity::user_role::Column::ScopeRefId.eq(subject_id))
            .one(db)
            .await?
            .is_some();
        if !already {
            entity::user_role::ActiveModel {
                id: sea_orm::ActiveValue::NotSet,
                user_id: Set(req.user_id),
                role_id: Set(rid),
                tenant_id: Set(Some(tenant_id)),
                scope: Set(scope.to_string()),
                scope_ref_id: Set(Some(subject_id)),
            }
            .insert(db)
            .await?;
        }
    }

    // Convenience: keep the property's display "manager" in sync with its primary
    // (or first) property manager.
    if subject_type == SUBJECT_PROPERTY && req.relationship == "property_manager" {
        if let Some(prop) = Property::find_by_id(subject_id)
            .filter(entity::property::Column::TenantId.eq(tenant_id))
            .one(db)
            .await?
        {
            if req.is_primary || prop.manager.trim().is_empty() {
                let mut am: entity::property::ActiveModel = prop.into();
                am.manager = Set(assignee.name.clone());
                am.update(db).await?;
            }
        }
    }

    crate::audit::record(
        db,
        Some(actor),
        crate::audit::actions::ASSIGNMENT_CREATE,
        Some("assignment"),
        Some(saved.id.to_string()),
        Some(tenant_id),
        Some(serde_json::json!({
            "subject_type": subject_type,
            "subject_id": subject_id,
            "user_id": req.user_id,
            "relationship": req.relationship,
        })),
    )
    .await;

    Ok(AssignmentDto::from(saved, assignee.name, assignee.email))
}

/// Remove an assignment (by id, scoped to the subject) and revoke its grant.
pub(crate) async fn remove_assignment_inner(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    actor: Uuid,
    subject_type: &str,
    subject_id: Uuid,
    assignment_id: Uuid,
) -> ApiResult<()> {
    let row = Assignment::find_by_id(assignment_id)
        .filter(entity::assignment::Column::TenantId.eq(tenant_id))
        .filter(entity::assignment::Column::SubjectType.eq(subject_type))
        .filter(entity::assignment::Column::SubjectId.eq(subject_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("assignment not found".into()))?;

    let (user_id, role_id, relationship) = (row.user_id, row.role_id, row.relationship.clone());
    let scope = grant_scope_for(subject_type);

    // Revoke the scoped grant this assignment conferred.
    if let Some(rid) = role_id {
        UserRole::delete_many()
            .filter(entity::user_role::Column::UserId.eq(user_id))
            .filter(entity::user_role::Column::RoleId.eq(rid))
            .filter(entity::user_role::Column::TenantId.eq(tenant_id))
            .filter(entity::user_role::Column::Scope.eq(scope))
            .filter(entity::user_role::Column::ScopeRefId.eq(subject_id))
            .exec(db)
            .await?;
    }

    Assignment::delete_by_id(assignment_id).exec(db).await?;

    crate::audit::record(
        db,
        Some(actor),
        crate::audit::actions::ASSIGNMENT_REMOVE,
        Some("assignment"),
        Some(assignment_id.to_string()),
        Some(tenant_id),
        Some(serde_json::json!({
            "subject_type": subject_type,
            "subject_id": subject_id,
            "user_id": user_id,
            "relationship": relationship,
        })),
    )
    .await;

    Ok(())
}
