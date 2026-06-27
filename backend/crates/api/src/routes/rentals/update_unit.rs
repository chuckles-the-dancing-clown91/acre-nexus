use super::dto::{UnitDto, UpdateUnitReq};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::Unit;
use rocket::serde::json::Json;
use rocket::{patch, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `PATCH /units/<id>` — update fields on a unit.
#[rocket_okapi::openapi(tag = "Rentals")]
#[patch("/units/<id>", data = "<body>")]
pub async fn update_unit(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateUnitReq>,
) -> ApiResult<Json<UnitDto>> {
    user.require(Permission::LeaseManage)?;
    let uid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let existing = Unit::find_by_id(uid)
        .filter(entity::unit::Column::TenantId.eq(scope.tenant_id))
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("unit not found".into()))?;
    let b = body.into_inner();
    let mut am: entity::unit::ActiveModel = existing.into();
    if let Some(v) = b.unit_number {
        am.unit_number = Set(v);
    }
    if let Some(v) = b.beds {
        am.beds = Set(Some(v));
    }
    if let Some(v) = b.baths {
        am.baths = Set(Some(v));
    }
    if let Some(v) = b.sqft {
        am.sqft = Set(Some(v));
    }
    if let Some(v) = b.market_rent_cents {
        am.market_rent_cents = Set(Some(v));
    }
    if let Some(v) = b.status {
        am.status = Set(v);
    }
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&state.db).await?;
    crate::audit::record(
        &state.db,
        Some(user.user_id),
        crate::audit::actions::UNIT_UPDATE,
        Some("unit"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "unit_number": saved.unit_number, "status": saved.status })),
    )
    .await;
    Ok(Json(UnitDto::from(saved)))
}
