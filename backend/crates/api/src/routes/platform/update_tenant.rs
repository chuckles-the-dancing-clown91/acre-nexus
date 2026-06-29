use super::dto::{TenantSummary, UpdateTenantReq};
use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use entity::prelude::{Property, Tenant};
use rocket::serde::json::Json;
use rocket::{patch, State};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use uuid::Uuid;

const VALID_STATUS: &[&str] = &["active", "suspended", "trial"];

/// `PATCH /platform/tenants/<id>` — change a tenant's status, plan, name, or domain.
#[rocket_okapi::openapi(tag = "Platform Admin")]
#[patch("/platform/tenants/<id>", data = "<body>")]
pub async fn update_tenant(
    state: &State<AppState>,
    user: AuthUser,
    id: &str,
    body: Json<UpdateTenantReq>,
) -> ApiResult<Json<TenantSummary>> {
    user.require(Permission::PlatformAdmin)?;
    let tid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid tenant id".into()))?;
    let t = Tenant::find_by_id(tid)
        .one(&state.user_db)
        .await?
        .ok_or_else(|| ApiError::NotFound("tenant not found".into()))?;
    let body = body.into_inner();

    let mut am: entity::tenant::ActiveModel = t.into();
    if let Some(status) = body.status {
        if !VALID_STATUS.contains(&status.as_str()) {
            return Err(ApiError::BadRequest("invalid status".into()));
        }
        am.status = Set(status);
    }
    if let Some(plan) = body.plan {
        am.plan = Set(plan);
    }
    if let Some(name) = body.name {
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(ApiError::BadRequest("name cannot be empty".into()));
        }
        am.name = Set(name);
    }
    if let Some(domain) = body.custom_domain {
        let d = domain.trim().to_string();
        am.custom_domain = Set(if d.is_empty() { None } else { Some(d) });
    }
    let t = am.update(&state.user_db).await?;

    crate::audit::record(
        &state.user_db,
        Some(user.user_id),
        "tenant.update",
        Some("tenant"),
        Some(tid.to_string()),
        None,
        None,
    )
    .await;

    // Recompute rollups (RLS-clamped property read).
    let txn = AppState::tenant_tx(&state.property_db, tid).await?;
    let props = Property::find().all(&txn).await?;
    txn.rollback().await.ok();
    let revenue: i64 = props.iter().map(|p| p.monthly_rent_cents).sum();

    Ok(Json(TenantSummary {
        id: t.id,
        slug: t.slug,
        name: t.name,
        plan: t.plan,
        status: t.status,
        custom_domain: t.custom_domain,
        property_count: props.len() as i64,
        managed_revenue_label: usd(revenue),
    }))
}
