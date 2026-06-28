use super::dto::{LienDto, UpdateLienReq};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::Lien;
use rocket::serde::json::Json;
use rocket::{patch, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `PATCH /liens/<id>` — update fields on a lien.
#[rocket_okapi::openapi(tag = "Title")]
#[patch("/liens/<id>", data = "<body>")]
pub async fn update_lien(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateLienReq>,
) -> ApiResult<Json<LienDto>> {
    user.require(Permission::TitleManage)?;
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let existing = Lien::find_by_id(lid)
        .filter(entity::lien::Column::TenantId.eq(scope.tenant_id))
        .one(&state.property_db)
        .await?
        .ok_or_else(|| ApiError::NotFound("lien not found".into()))?;
    let b = body.into_inner();
    let mut am: entity::lien::ActiveModel = existing.into();
    if let Some(v) = b.lienholder_id {
        am.lienholder_id = Set(Some(v));
    }
    if let Some(v) = b.lienholder_name {
        am.lienholder_name = Set(v);
    }
    if let Some(v) = b.kind {
        am.kind = Set(v);
    }
    if let Some(v) = b.amount_cents {
        am.amount_cents = Set(Some(v));
    }
    if let Some(v) = b.position {
        am.position = Set(Some(v));
    }
    if let Some(v) = b.recorded_date {
        am.recorded_date = Set(Some(v));
    }
    if let Some(v) = b.status {
        am.status = Set(v);
    }
    if let Some(v) = b.reference {
        am.reference = Set(Some(v));
    }
    if let Some(v) = b.notes {
        am.notes = Set(Some(v));
    }
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&state.property_db).await?;
    crate::audit::record(
        &state.user_db,
        Some(user.user_id),
        crate::audit::actions::LIEN_UPDATE,
        Some("lien"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "kind": saved.kind, "status": saved.status })),
    )
    .await;
    Ok(Json(LienDto::from(saved)))
}
