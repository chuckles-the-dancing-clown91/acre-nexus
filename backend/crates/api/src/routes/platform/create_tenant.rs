use super::dto::{CreateTenantReq, TenantSummary};
use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use entity::prelude::Tenant;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `POST /platform/tenants` — provision a new client company.
#[rocket_okapi::openapi(tag = "Platform Admin")]
#[post("/platform/tenants", data = "<body>")]
pub async fn create_tenant(
    state: &State<AppState>,
    user: AuthUser,
    body: Json<CreateTenantReq>,
) -> ApiResult<Json<TenantSummary>> {
    user.require(Permission::PlatformAdmin)?;
    let body = body.into_inner();

    let slug = body.slug.trim().to_lowercase();
    let name = body.name.trim().to_string();
    if slug.is_empty() || name.is_empty() {
        return Err(ApiError::BadRequest("slug and name are required".into()));
    }
    let plan = match body.plan.trim() {
        "" => "starter".to_string(),
        p => p.to_string(),
    };

    if Tenant::find()
        .filter(entity::tenant::Column::Slug.eq(slug.clone()))
        .one(&state.user_db)
        .await?
        .is_some()
    {
        return Err(ApiError::Conflict(
            "a tenant with that slug already exists".into(),
        ));
    }

    let id = Uuid::new_v4();
    entity::tenant::ActiveModel {
        id: Set(id),
        slug: Set(slug.clone()),
        name: Set(name.clone()),
        plan: Set(plan.clone()),
        status: Set("active".into()),
        custom_domain: Set(None),
        created_at: Set(chrono::Utc::now().into()),
    }
    .insert(&state.user_db)
    .await?;

    crate::audit::record(
        &state.user_db,
        Some(user.user_id),
        "tenant.create",
        Some("tenant"),
        Some(id.to_string()),
        None,
        None,
    )
    .await;

    Ok(Json(TenantSummary {
        id,
        slug,
        name,
        plan,
        status: "active".into(),
        custom_domain: None,
        property_count: 0,
        managed_revenue_label: usd(0),
    }))
}
