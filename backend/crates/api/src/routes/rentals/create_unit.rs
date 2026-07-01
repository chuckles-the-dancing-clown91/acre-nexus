use super::dto::{CreateUnitReq, UnitDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::Property;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `POST /properties/<id>/units` — add a rentable unit to a property.
#[rocket_okapi::openapi(tag = "Rentals")]
#[post("/properties/<id>/units", data = "<body>")]
pub async fn create_unit(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<CreateUnitReq>,
) -> ApiResult<Json<UnitDto>> {
    user.require(Permission::LeaseManage)?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    let b = body.into_inner();
    let now = Utc::now();
    let status = match b.status {
        Some(s) if !s.trim().is_empty() => s,
        _ => "vacant".to_string(),
    };
    let model = entity::unit::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        property_id: Set(pid),
        unit_number: Set(b.unit_number),
        beds: Set(b.beds),
        baths: Set(b.baths),
        sqft: Set(b.sqft),
        market_rent_cents: Set(b.market_rent_cents),
        status: Set(status),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    };
    let saved = model.insert(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::UNIT_CREATE,
        Some("unit"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "property_id": saved.property_id, "unit_number": saved.unit_number })),
    )
    .await;
    Ok(Json(UnitDto::from(saved)))
}
