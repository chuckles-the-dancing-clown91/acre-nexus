//! `POST /platform/provision` — provision a new PM-firm tenant (§5.1).
//!
//! Creates the tenant shell (`status = provisioning`), its first membership (the
//! firm owner, granted `tenant_owner` at `tenant` scope), a default theme, a
//! reserved `{slug}.acrenexus.com` subdomain, and the per-tenant onboarding
//! workflow. Triggered by Acre staff (`tenant:manage`); the returned owner
//! credentials let the firm admin start self-onboarding (§5.2).

use super::dto::{ProvisionReq, ProvisionResp};
use crate::auth::{hash_password, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use chrono::Utc;
use entity::prelude::{Role, Tenant};
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait};
use serde_json::json;
use uuid::Uuid;

/// `POST /platform/provision` — stand up a new firm tenant + owner.
#[rocket_okapi::openapi(tag = "Platform Admin")]
#[post("/platform/provision", data = "<body>")]
pub async fn provision(
    state: &State<AppState>,
    user: AuthUser,
    body: Json<ProvisionReq>,
) -> ApiResult<Json<ProvisionResp>> {
    user.require(Permission::TenantManage)?;
    let b = body.into_inner();

    let slug = b.slug.trim().to_lowercase();
    if slug.is_empty() || !slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(ApiError::BadRequest(
            "slug must be non-empty and url-safe (a-z, 0-9, -)".into(),
        ));
    }
    let owner_email = b.owner_email.trim().to_lowercase();
    if owner_email.is_empty() {
        return Err(ApiError::BadRequest("owner_email is required".into()));
    }

    // Slug uniqueness (the column is unique, but give a friendly error first).
    if Tenant::find()
        .filter(entity::tenant::Column::Slug.eq(slug.clone()))
        .one(&state.db)
        .await?
        .is_some()
    {
        return Err(ApiError::Conflict(format!("slug '{slug}' is taken")));
    }

    // The firm-owner system role to grant at tenant scope.
    let owner_role = Role::find()
        .filter(entity::role::Column::Key.eq("tenant_owner"))
        .filter(entity::role::Column::IsSystem.eq(true))
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::Internal(anyhow::anyhow!("tenant_owner system role missing")))?;

    let now = Utc::now();
    let tenant_id = Uuid::new_v4();
    let owner_user_id = Uuid::new_v4();
    let temp_password = b
        .owner_password
        .clone()
        .unwrap_or_else(|| crate::auth::random_secret(12));
    let pw_hash = hash_password(&temp_password).map_err(ApiError::Internal)?;
    let hostname = format!("{slug}.acrenexus.com");

    let txn = state.db.begin().await?;

    // ---- tenant shell (provisioning) ----
    entity::tenant::ActiveModel {
        id: Set(tenant_id),
        slug: Set(slug.clone()),
        name: Set(b.name.clone()),
        plan: Set(b.plan.clone().unwrap_or_else(|| "starter".into())),
        status: Set("provisioning".into()),
        custom_domain: Set(None),
        parent_org_id: Set(None),
        created_at: Set(now.into()),
    }
    .insert(&txn)
    .await?;

    // ---- default theme ----
    entity::theme::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        company_name: Set(b.name.clone()),
        logo_url: Set(None),
        primary_color: Set("#F5451F".into()),
        accent_color: Set("#F5451F".into()),
        default_mode: Set("light".into()),
        legal_templates: Set(json!({})),
        updated_at: Set(now.into()),
    }
    .insert(&txn)
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
        created_at: Set(now.into()),
    }
    .insert(&txn)
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
    .insert(&txn)
    .await?;

    // ---- firm owner: user + membership + scoped role ----
    entity::user::ActiveModel {
        id: Set(owner_user_id),
        tenant_id: Set(Some(tenant_id)),
        email: Set(owner_email.clone()),
        username: Set(None),
        password_hash: Set(pw_hash),
        name: Set(b.owner_name.clone().unwrap_or_else(|| b.name.clone())),
        is_platform_staff: Set(false),
        status: Set("active".into()),
        last_login_at: Set(None),
        created_at: Set(now.into()),
    }
    .insert(&txn)
    .await?;

    entity::membership::ActiveModel {
        id: Set(Uuid::new_v4()),
        user_id: Set(owner_user_id),
        scope: Set("tenant".into()),
        tenant_id: Set(Some(tenant_id)),
        profile_type: Set("tenant_owner".into()),
        title: Set(Some("Principal".into())),
        status: Set("active".into()),
        is_primary: Set(true),
        created_at: Set(now.into()),
    }
    .insert(&txn)
    .await?;

    entity::user_role::ActiveModel {
        id: sea_orm::ActiveValue::NotSet,
        user_id: Set(owner_user_id),
        role_id: Set(owner_role.id),
        tenant_id: Set(Some(tenant_id)),
        scope: Set("tenant".into()),
        scope_ref_id: Set(None),
    }
    .insert(&txn)
    .await?;

    txn.commit().await?;

    crate::audit::record(
        &state.db,
        Some(user.user_id),
        crate::audit::actions::TENANT_PROVISION,
        Some("tenant"),
        Some(tenant_id.to_string()),
        Some(tenant_id),
        Some(json!({ "slug": slug, "owner_email": owner_email })),
    )
    .await;

    Ok(Json(ProvisionResp {
        tenant_id,
        slug,
        subdomain: hostname,
        owner_user_id,
        owner_email,
        // Returned once so the operator can hand off / the owner can sign in.
        temp_password: if b.owner_password.is_some() {
            None
        } else {
            Some(temp_password)
        },
    }))
}
