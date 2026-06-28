use super::dto::{CreateOwnershipReq, OwnershipDto};
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

/// `POST /properties/<id>/ownership` — add an ownership record to a property.
#[rocket_okapi::openapi(tag = "Title")]
#[post("/properties/<id>/ownership", data = "<body>")]
pub async fn create_ownership(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<CreateOwnershipReq>,
) -> ApiResult<Json<OwnershipDto>> {
    user.require(Permission::TitleManage)?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&state.property_db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    let b = body.into_inner();
    let now = Utc::now();
    let owner_kind = match b.owner_kind {
        Some(s) if !s.trim().is_empty() => s,
        _ => "llc".to_string(),
    };
    let model = entity::ownership::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        property_id: Set(pid),
        owner_kind: Set(owner_kind),
        owner_id: Set(b.owner_id),
        owner_name: Set(b.owner_name),
        vesting: Set(b.vesting),
        percent_bps: Set(b.percent_bps.unwrap_or(10000)),
        deed_type: Set(b.deed_type),
        deed_recorded_date: Set(b.deed_recorded_date),
        deed_reference: Set(b.deed_reference),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    };
    let saved = model.insert(&state.property_db).await?;
    crate::audit::record(
        &state.user_db,
        Some(user.user_id),
        crate::audit::actions::OWNERSHIP_CREATE,
        Some("ownership"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "property_id": saved.property_id, "owner_kind": saved.owner_kind, "percent_bps": saved.percent_bps })),
    )
    .await;
    Ok(Json(OwnershipDto::from(saved)))
}
