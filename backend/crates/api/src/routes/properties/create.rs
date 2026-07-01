use super::dto::{CreatePropertyReq, PropertyResp};
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, Set};
use uuid::Uuid;

/// `POST /properties` — add a property to the portfolio.
#[rocket_okapi::openapi(tag = "Properties")]
#[post("/properties", data = "<body>")]
pub async fn create(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<CreatePropertyReq>,
) -> ApiResult<Json<PropertyResp>> {
    user.require(Permission::PropertyWrite)?;
    let b = body.into_inner();
    let model = entity::property::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        llc_id: Set(b.llc_id),
        portfolio_id: Set(b.portfolio_id),
        name: Set(b.name),
        address: Set(b.address),
        city: Set(b.city),
        units: Set(b.units),
        occupied_units: Set(b.occupied_units),
        monthly_rent_cents: Set(b.monthly_rent_cents),
        status: Set(b.status.unwrap_or_else(|| "Stabilized".into())),
        year_built: Set(b.year_built.unwrap_or(0)),
        manager: Set(b.manager.unwrap_or_default()),
        property_type: Set(b.property_type.unwrap_or_default()),
        strategy: Set(b.strategy.unwrap_or_else(|| "rental".into())),
        workflow_stage: Set(String::new()),
        purchase_price_cents: Set(None),
        acquired_on: Set(None),
        created_at: Set(Utc::now().into()),
    };
    let saved = model.insert(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::PROPERTY_CREATE,
        Some("property"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "name": saved.name, "city": saved.city })),
    )
    .await;
    Ok(Json(PropertyResp::from(saved)))
}
