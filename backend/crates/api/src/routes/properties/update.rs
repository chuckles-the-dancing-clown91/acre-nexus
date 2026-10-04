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
    let before = p.clone();
    let mut am: entity::property::ActiveModel = p.into();
    let b = body.into_inner();
    if let Some(v) = b.name {
        am.name = Set(v);
    }
    if let Some(raw) = b.property_type.as_deref() {
        let kind = crate::property_kind::parse_for_save(raw)?;
        let units = crate::property_kind::unit_count(&db, pid).await?;
        // A house is one unit: it can't take that type while it has several.
        if crate::property_kind::unit_mode(&kind) == crate::property_kind::UnitMode::Single
            && units > 1
        {
            return Err(ApiError::Conflict(format!(
                "this property has {units} units, so it can't be a {}",
                crate::property_kind::label(&kind).to_lowercase()
            )));
        }
        am.property_type = Set(kind.clone());
        if crate::property_kind::unit_mode(&kind) == crate::property_kind::UnitMode::Single
            && !kind.is_empty()
        {
            crate::property_kind::ensure_home_unit(&db, scope.tenant_id, pid).await?;
        }
    }
    let mut address_changed = false;
    if let Some(v) = b
        .address
        .clone()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
    {
        address_changed = true;
        am.address = Set(v);
    }
    if let Some(v) = b
        .city
        .clone()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
    {
        address_changed = true;
        am.city = Set(v);
    }
    if let Some(v) = b.state.clone() {
        am.state = Set(crate::geo::state_code(&v));
    }
    if let Some(v) = b.postal_code.clone() {
        am.postal_code = Set(v.trim().to_string());
    }
    if address_changed {
        // A new address means a new photo.
        am.photo_status = Set("none".into());
        am.photo_error = Set(None);
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
    if let Some(v) = b.image_url {
        // Empty string clears the hero image.
        am.image_url = Set(if v.trim().is_empty() { None } else { Some(v) });
    }
    let saved = am.update(&db).await?;
    if address_changed {
        crate::geo::queue_fetch(&db, scope.tenant_id, saved.id).await;
    }
    crate::audit::change::change(
        &db,
        crate::audit::change::Ctx::new(&user, &scope),
        crate::audit::actions::PROPERTY_UPDATE,
        "property",
        saved.id,
        Some(saved.id),
        &saved.name,
        &before,
        &saved,
    )
    .await;
    Ok(Json(PropertyResp::from(saved)))
}
