use super::dto::TenantDetail;
use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use entity::prelude::{Property, Tenant};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

/// `GET /platform/tenants/<id>` — a single client company with its rollups.
#[rocket_okapi::openapi(tag = "Platform Admin")]
#[get("/platform/tenants/<id>")]
pub async fn get_tenant(
    state: &State<AppState>,
    user: AuthUser,
    id: &str,
) -> ApiResult<Json<TenantDetail>> {
    user.require(Permission::PlatformAdmin)?;
    let tid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid tenant id".into()))?;

    let t = Tenant::find_by_id(tid)
        .one(&state.user_db)
        .await?
        .ok_or_else(|| ApiError::NotFound("tenant not found".into()))?;

    // Properties live in acre_property under RLS — clamp to this tenant.
    let txn = AppState::tenant_tx(&state.property_db, tid).await?;
    let props = Property::find().all(&txn).await?;
    txn.rollback().await.ok();
    let revenue: i64 = props.iter().map(|p| p.monthly_rent_cents).sum();

    // Members are tenant-scoped memberships in acre_user.
    let members = entity::membership::Entity::find()
        .filter(entity::membership::Column::TenantId.eq(tid))
        .all(&state.user_db)
        .await?;

    Ok(Json(TenantDetail {
        id: t.id,
        slug: t.slug,
        name: t.name,
        plan: t.plan,
        status: t.status,
        custom_domain: t.custom_domain,
        property_count: props.len() as i64,
        member_count: members.len() as i64,
        revenue_cents: revenue,
        managed_revenue_label: usd(revenue),
        created_at: t.created_at.to_rfc3339(),
    }))
}
