//! `POST /platform/provision` — provision a new PM-firm tenant (§5.1).
//!
//! Creates the tenant shell (`status = provisioning`), its first membership (the
//! firm owner, granted `tenant_owner` at `tenant` scope), a default theme, a
//! reserved `{slug}.acrenexus.com` subdomain, and the per-tenant onboarding
//! workflow — the work itself is [`crate::provisioning`], shared with
//! Solnyxus's `POST /.well-known/solnyxus/tenants`. Triggered by Acre staff
//! (`tenant:manage`); the returned owner credentials let the firm admin start
//! self-onboarding (§5.2).

use super::dto::{ProvisionReq, ProvisionResp};
use crate::auth::{hash_password, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::provisioning::{self, NewWorkspace, Owner};
use crate::rbac::Permission;
use crate::state::AppState;
use entity::prelude::Tenant;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;

/// `POST /platform/provision` — stand up a new firm tenant + owner.
#[rocket_okapi::openapi(tag = "Platform Admin")]
#[post("/platform/provision", data = "<body>")]
pub async fn provision(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    body: Json<ProvisionReq>,
) -> ApiResult<Json<ProvisionResp>> {
    user.require(Permission::TenantManage)?;
    let b = body.into_inner();

    let slug = provisioning::normalize_slug(&b.slug).map_err(ApiError::BadRequest)?;
    let owner_email = b.owner_email.trim().to_lowercase();
    if owner_email.is_empty() {
        return Err(ApiError::BadRequest("owner_email is required".into()));
    }

    // Slug uniqueness (the column is unique, but give a friendly error first).
    if Tenant::find()
        .filter(entity::tenant::Column::Slug.eq(slug.clone()))
        .one(&db)
        .await?
        .is_some()
    {
        return Err(ApiError::Conflict(format!("slug '{slug}' is taken")));
    }

    let temp_password = b
        .owner_password
        .clone()
        .unwrap_or_else(|| crate::auth::random_secret(12));
    let pw_hash = hash_password(&temp_password).map_err(ApiError::Internal)?;

    // The whole request runs inside one RLS-scoped transaction (see `crate::db`);
    // provision is a platform-staff op (null tenant GUC), so RLS permits the
    // cross-tenant inserts that stand up the new firm.
    let made = provisioning::create_workspace(
        &db,
        NewWorkspace {
            slug: slug.clone(),
            name: b.name.clone(),
            plan: b.plan.clone(),
            status: "provisioning",
            owner: Owner::New {
                email: owner_email.clone(),
                name: b.owner_name.clone().unwrap_or_else(|| b.name.clone()),
                password_hash: pw_hash,
                status: "active",
            },
        },
    )
    .await?;
    let tenant_id = made.tenant_id;

    crate::audit::record(
        &db,
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
        subdomain: made.hostname,
        owner_user_id: made.owner_user_id,
        owner_email,
        // Returned once so the operator can hand off / the owner can sign in.
        temp_password: if b.owner_password.is_some() {
            None
        } else {
            Some(temp_password)
        },
    }))
}
