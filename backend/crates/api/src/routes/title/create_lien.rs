use super::dto::{CreateLienReq, LienDto};
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

/// `POST /properties/<id>/liens` — record a lien against a property's title.
#[rocket_okapi::openapi(tag = "Title")]
#[post("/properties/<id>/liens", data = "<body>")]
pub async fn create_lien(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<CreateLienReq>,
) -> ApiResult<Json<LienDto>> {
    user.require(Permission::TitleManage)?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    let b = body.into_inner();
    let now = Utc::now();
    let kind = match b.kind {
        Some(s) if !s.trim().is_empty() => s,
        _ => "other".to_string(),
    };
    let status = match b.status {
        Some(s) if !s.trim().is_empty() => s,
        _ => "active".to_string(),
    };
    let model = entity::lien::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        property_id: Set(pid),
        lienholder_id: Set(b.lienholder_id),
        lienholder_name: Set(b.lienholder_name),
        kind: Set(kind),
        amount_cents: Set(b.amount_cents),
        position: Set(b.position),
        recorded_date: Set(b.recorded_date),
        status: Set(status),
        reference: Set(b.reference),
        notes: Set(b.notes),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    };
    let saved = model.insert(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::LIEN_CREATE,
        Some("lien"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "property_id": saved.property_id, "kind": saved.kind, "status": saved.status })),
    )
    .await;
    Ok(Json(LienDto::from(saved)))
}
