use super::dto::{LeadDto, UpdateLeadReq, STATUSES};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::Lead;
use rocket::serde::json::Json;
use rocket::{patch, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `PATCH /leads/<id>` — work a lead: contact details, pipeline status, notes.
#[rocket_okapi::openapi(tag = "Leads")]
#[patch("/leads/<id>", data = "<body>")]
pub async fn update_lead(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateLeadReq>,
) -> ApiResult<Json<LeadDto>> {
    user.require(Permission::ApplicationWrite)?;
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let lead = Lead::find_by_id(lid)
        .filter(entity::lead::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("lead not found".into()))?;
    let b = body.into_inner();
    let mut am: entity::lead::ActiveModel = lead.into();
    if let Some(name) = b.name.filter(|n| !n.trim().is_empty()) {
        am.name = Set(name.trim().to_string());
    }
    if let Some(phone) = b.phone {
        am.phone = Set(Some(phone).filter(|p| !p.trim().is_empty()));
    }
    if let Some(status) = b.status.filter(|s| !s.is_empty()) {
        if !STATUSES.contains(&status.as_str()) {
            return Err(ApiError::BadRequest(format!(
                "invalid status '{status}' (expected one of {})",
                STATUSES.join(", ")
            )));
        }
        am.status = Set(status);
    }
    if let Some(notes) = b.notes {
        am.notes = Set(Some(notes).filter(|n| !n.trim().is_empty()));
    }
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::LEAD_UPDATE,
        Some("lead"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "status": saved.status })),
    )
    .await;

    Ok(Json(LeadDto::from(saved)))
}
