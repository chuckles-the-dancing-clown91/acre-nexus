use super::dto::{PropertyResp, UpdatePropertyReq};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Property;
use rocket::serde::json::Json;
use rocket::{patch, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `PATCH /properties/<id>` — update mutable property fields.
#[rocket_okapi::openapi(tag = "Properties")]
#[patch("/properties/<id>", data = "<body>")]
pub async fn update(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdatePropertyReq>,
) -> ApiResult<Json<PropertyResp>> {
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let p = Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    // Scoped authorization: a firm-wide `property:write` grant passes; so does a
    // narrower grant (entity/portfolio/property) that covers this property — so a
    // contract bookkeeper scoped to one LLC can edit only that LLC's properties.
    let resource = crate::rbac::scope::ResourceScope::property(p.id, p.portfolio_id, p.llc_id);
    crate::tenancy::resolve::require_scoped(&db, &user, Permission::PropertyWrite, &resource)
        .await?;
    let mut am: entity::property::ActiveModel = p.into();
    let b = body.into_inner();
    if let Some(v) = b.name {
        am.name = Set(v);
    }
    if let Some(v) = b.status {
        am.status = Set(v);
    }
    if let Some(v) = b.occupied_units {
        am.occupied_units = Set(v);
    }
    if let Some(v) = b.monthly_rent_cents {
        am.monthly_rent_cents = Set(v);
    }
    if let Some(v) = b.manager {
        am.manager = Set(v);
    }
    let saved = am.update(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::PROPERTY_UPDATE,
        Some("property"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        None,
    )
    .await;
    Ok(Json(PropertyResp::from(saved)))
}
