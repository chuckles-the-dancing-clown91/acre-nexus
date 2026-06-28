use super::dto::TenantSummary;
use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use entity::prelude::{Property, Tenant};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

/// `GET /platform/tenants` — every client company on the platform.
#[rocket_okapi::openapi(tag = "Platform Admin")]
#[get("/platform/tenants")]
pub async fn tenants(
    state: &State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Vec<TenantSummary>>> {
    user.require(Permission::PlatformAdmin)?;
    // Cross-database read: the tenant registry lives in acre_user; each tenant's
    // properties live in acre_property. Two queries, joined in application code.
    let all = Tenant::find()
        .order_by_asc(entity::tenant::Column::Name)
        .all(&state.user_db)
        .await?;

    let mut out = Vec::new();
    for t in all {
        let props = Property::find()
            .filter(entity::property::Column::TenantId.eq(t.id))
            .all(&state.property_db)
            .await?;
        let revenue: i64 = props.iter().map(|p| p.monthly_rent_cents).sum();
        out.push(TenantSummary {
            id: t.id,
            slug: t.slug,
            name: t.name,
            plan: t.plan,
            status: t.status,
            custom_domain: t.custom_domain,
            property_count: props.len() as i64,
            managed_revenue_label: usd(revenue),
        });
    }
    Ok(Json(out))
}
