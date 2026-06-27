use super::dto::{CounterpartyDto, UpdateCounterpartyReq};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::Counterparty;
use rocket::serde::json::Json;
use rocket::{patch, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `PATCH /entities/<id>` — update mutable fields of a counterparty.
#[rocket_okapi::openapi(tag = "Entities")]
#[patch("/entities/<id>", data = "<body>")]
pub async fn update(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateCounterpartyReq>,
) -> ApiResult<Json<CounterpartyDto>> {
    user.require(Permission::EntityManage)?;
    let cid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let c = Counterparty::find_by_id(cid)
        .filter(entity::counterparty::Column::TenantId.eq(scope.tenant_id))
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("counterparty not found".into()))?;
    let b = body.into_inner();
    let mut am: entity::counterparty::ActiveModel = c.into();
    if let Some(v) = b.kind {
        am.kind = Set(v);
    }
    if let Some(v) = b.name {
        am.name = Set(v);
    }
    if let Some(v) = b.contact_name {
        am.contact_name = Set(Some(v));
    }
    if let Some(v) = b.email {
        am.email = Set(Some(v));
    }
    if let Some(v) = b.phone {
        am.phone = Set(Some(v));
    }
    if let Some(v) = b.website {
        am.website = Set(Some(v));
    }
    if let Some(v) = b.address {
        am.address = Set(Some(v));
    }
    if let Some(v) = b.notes {
        am.notes = Set(Some(v));
    }
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&state.db).await?;
    crate::audit::record(
        &state.db,
        Some(user.user_id),
        crate::audit::actions::ENTITY_UPDATE,
        Some("counterparty"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "name": saved.name })),
    )
    .await;
    Ok(Json(CounterpartyDto::from(saved)))
}
