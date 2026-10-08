//! **Workspace provisioning** — the one body of work that stands up a new
//! PM-firm workspace, shared by staff provisioning (`POST /platform/provision`)
//! and Solnyxus (`POST /.well-known/solnyxus/tenants`, see
//! [`crate::routes::solnyxus`]).
//!
//! Creates the tenant shell, a default theme, the reserved `{slug}.acrenexus.com`
//! subdomain, the onboarding workflow and the owner — a primary membership
//! granted the `tenant_owner` system role (the workspace's highest role) at
//! tenant scope, on a new login or an existing one — then schedules the
//! workspace's recurring jobs. Callers run it on the platform plane (no tenant
//! GUC), where RLS permits the cross-tenant inserts, and inside one transaction
//! so a failure leaves nothing behind.

use crate::error::{ApiError, ApiResult};
use chrono::Utc;
use entity::prelude::Role;
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use serde_json::json;
use uuid::Uuid;

/// The workspace's highest role, granted to its owner.
pub const OWNER_ROLE: &str = "tenant_owner";

/// Brand colour a new workspace starts with (primary and accent).
pub const DEFAULT_COLOR: &str = "#0E7C86";

/// A slug as stored: trimmed, lowercased, a DNS label (`a-z`, `0-9`, `-`, at
/// most 63 characters, no leading or trailing `-`) since it names a subdomain.
pub fn normalize_slug(raw: &str) -> Result<String, String> {
    let slug = raw.trim().to_lowercase();
    if slug.is_empty()
        || slug.len() > 63
        || slug.starts_with('-')
        || slug.ends_with('-')
        || !slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err("slug must be non-empty and url-safe (a-z, 0-9, -)".into());
    }
    Ok(slug)
}

/// Who owns the new workspace.
pub enum Owner {
    /// A new login. `status` is `active` (staff hand over a password) or
    /// `invited` (the owner chooses one from a set-password link first).
    New {
        email: String,
        name: String,
        password_hash: String,
        status: &'static str,
    },
    /// Someone who already has a login (another workspace's member): they get
    /// a membership here, their account and password stay as they are.
    Existing(entity::user::Model),
}

pub struct NewWorkspace {
    pub slug: String,
    pub name: String,
    /// A `saas` plan key; unknown or empty falls back to the lowest plan.
    pub plan: Option<String>,
    /// `provisioning` (staff hand-off, firm self-onboards) or `active`.
    pub status: &'static str,
    pub owner: Owner,
}

pub struct Provisioned {
    pub tenant_id: Uuid,
    pub owner_user_id: Uuid,
    pub hostname: String,
    pub plan: &'static str,
}

/// Stand up the workspace. Slug uniqueness is the caller's to check first (the
/// column is unique either way).
pub async fn create_workspace(
    db: &impl ConnectionTrait,
    w: NewWorkspace,
) -> ApiResult<Provisioned> {
    // The firm-owner system role to grant at tenant scope.
    let owner_role = Role::find()
        .filter(entity::role::Column::Key.eq(OWNER_ROLE))
        .filter(entity::role::Column::IsSystem.eq(true))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::Internal(anyhow::anyhow!("tenant_owner system role missing")))?;

    let now = Utc::now();
    let tenant_id = Uuid::new_v4();
    let hostname = format!("{}.acrenexus.com", w.slug);
    let plan = crate::saas::plan_for(&w.plan.unwrap_or_default().trim().to_lowercase()).key;

    // ---- tenant shell ----
    entity::tenant::ActiveModel {
        id: Set(tenant_id),
        slug: Set(w.slug.clone()),
        name: Set(w.name.clone()),
        plan: Set(plan.into()),
        status: Set(w.status.into()),
        custom_domain: Set(None),
        parent_org_id: Set(None),
        created_at: Set(now.into()),
    }
    .insert(db)
    .await?;

    // ---- default theme ----
    entity::theme::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        company_name: Set(w.name.clone()),
        logo_url: Set(None),
        primary_color: Set(DEFAULT_COLOR.into()),
        accent_color: Set(DEFAULT_COLOR.into()),
        default_mode: Set("light".into()),
        legal_templates: Set(json!({})),
        notification_templates: Set(json!({})),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;

    // ---- reserved subdomain (admin audience) ----
    entity::domain::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        hostname: Set(hostname.clone()),
        kind: Set("subdomain".into()),
        audience: Set("admin".into()),
        verification_token: Set(None),
        verified_at: Set(Some(now.into())),
        tls_status: Set("active".into()),
        email_dns_status: Set(json!({})),
        email_verified_at: Set(None),
        created_at: Set(now.into()),
    }
    .insert(db)
    .await?;

    // ---- onboarding workflow ----
    entity::onboarding_workflow::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        state: Set("provisioning".into()),
        steps: Set(json!({})),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;

    // ---- firm owner: user + membership + scoped role ----
    let (owner_user_id, primary) = match w.owner {
        Owner::New {
            email,
            name,
            password_hash,
            status,
        } => {
            let id = Uuid::new_v4();
            entity::user::ActiveModel {
                id: Set(id),
                tenant_id: Set(Some(tenant_id)),
                email: Set(email),
                username: Set(None),
                password_hash: Set(password_hash),
                name: Set(name),
                is_platform_staff: Set(false),
                status: Set(status.into()),
                last_login_at: Set(None),
                created_at: Set(now.into()),
            }
            .insert(db)
            .await?;
            (id, true)
        }
        // Their first workspace stays their primary one.
        Owner::Existing(u) => (u.id, false),
    };

    entity::membership::ActiveModel {
        id: Set(Uuid::new_v4()),
        user_id: Set(owner_user_id),
        scope: Set("tenant".into()),
        tenant_id: Set(Some(tenant_id)),
        profile_type: Set(OWNER_ROLE.into()),
        title: Set(Some("Principal".into())),
        status: Set("active".into()),
        is_primary: Set(primary),
        created_at: Set(now.into()),
    }
    .insert(db)
    .await?;

    entity::user_role::ActiveModel {
        id: sea_orm::ActiveValue::NotSet,
        user_id: Set(owner_user_id),
        role_id: Set(owner_role.id),
        tenant_id: Set(Some(tenant_id)),
        scope: Set("tenant".into()),
        scope_ref_id: Set(None),
    }
    .insert(db)
    .await?;

    // The new workspace gets its recurring billing cycle + reminder scan
    // immediately (boot only covers tenants that existed at startup).
    if let Err(e) = crate::billing::ensure_cycle_for_tenant(db, tenant_id).await {
        tracing::error!("provision: billing cycle scheduling failed: {e}");
    }
    if let Err(e) = crate::reminders::ensure_scan_for_tenant(db, tenant_id).await {
        tracing::error!("provision: reminder scan scheduling failed: {e}");
    }
    if let Err(e) = crate::helpdesk::ensure_scan_for_tenant(db, tenant_id).await {
        tracing::error!("provision: helpdesk scan scheduling failed: {e}");
    }
    if let Err(e) = crate::saas::ensure_job_for_tenant(db, tenant_id).await {
        tracing::error!("provision: platform billing scheduling failed: {e}");
    }

    Ok(Provisioned {
        tenant_id,
        owner_user_id,
        hostname,
        plan,
    })
}

/// Whether `user_id` holds the owner role in `tenant_id`.
pub async fn is_owner(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<bool, sea_orm::DbErr> {
    let Some(role) = Role::find()
        .filter(entity::role::Column::Key.eq(OWNER_ROLE))
        .filter(entity::role::Column::IsSystem.eq(true))
        .one(db)
        .await?
    else {
        return Ok(false);
    };
    Ok(entity::prelude::UserRole::find()
        .filter(entity::user_role::Column::UserId.eq(user_id))
        .filter(entity::user_role::Column::RoleId.eq(role.id))
        .filter(entity::user_role::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .is_some())
}

#[cfg(test)]
mod tests {
    use super::normalize_slug;

    #[test]
    fn slugs() {
        assert_eq!(normalize_slug("  Acme-PM ").unwrap(), "acme-pm");
        assert!(normalize_slug("").is_err());
        assert!(normalize_slug("acme pm").is_err());
        assert!(normalize_slug("-acme").is_err());
        assert!(normalize_slug("acme-").is_err());
        assert!(normalize_slug("acme.pm").is_err());
        assert!(normalize_slug(&"a".repeat(64)).is_err());
        assert!(normalize_slug(&"a".repeat(63)).is_ok());
    }
}
