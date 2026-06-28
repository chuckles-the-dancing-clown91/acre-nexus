use super::dto::{OwnershipDto, UpdateOwnershipReq};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::Ownership;
use rocket::serde::json::Json;
use rocket::{patch, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `PATCH /ownership/<id>` — update fields on an ownership record.
#[rocket_okapi::openapi(tag = "Title")]
#[patch("/ownership/<id>", data = "<body>")]
pub async fn update_ownership(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateOwnershipReq>,
) -> ApiResult<Json<OwnershipDto>> {
    user.require(Permission::TitleManage)?;
    let oid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let existing = Ownership::find_by_id(oid)
        .filter(entity::ownership::Column::TenantId.eq(scope.tenant_id))
        .one(&state.property_db)
        .await?
        .ok_or_else(|| ApiError::NotFound("ownership not found".into()))?;
    let b = body.into_inner();
    let mut am: entity::ownership::ActiveModel = existing.into();
    if let Some(v) = b.owner_kind {
        am.owner_kind = Set(v);
    }
    if let Some(v) = b.owner_id {
        am.owner_id = Set(Some(v));
    }
    if let Some(v) = b.owner_name {
        am.owner_name = Set(v);
    }
    if let Some(v) = b.vesting {
        am.vesting = Set(Some(v));
    }
    if let Some(v) = b.percent_bps {
        am.percent_bps = Set(v);
    }
    if let Some(v) = b.deed_type {
        am.deed_type = Set(Some(v));
    }
    if let Some(v) = b.deed_recorded_date {
        am.deed_recorded_date = Set(Some(v));
    }
    if let Some(v) = b.deed_reference {
        am.deed_reference = Set(Some(v));
    }
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&state.property_db).await?;
    crate::audit::record(
        &state.user_db,
        Some(user.user_id),
        crate::audit::actions::OWNERSHIP_UPDATE,
        Some("ownership"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(
            serde_json::json!({ "owner_kind": saved.owner_kind, "percent_bps": saved.percent_bps }),
        ),
    )
    .await;
    Ok(Json(OwnershipDto::from(saved)))
}
