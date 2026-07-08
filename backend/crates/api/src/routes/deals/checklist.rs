use super::dto::{DealDto, UpdateChecklistReq};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use rocket::serde::json::Json;
use rocket::{patch, State};
use sea_orm::{ActiveModelTrait, IntoActiveModel, Set};

/// `PATCH /modules/flips/deals/<id>/checklist` — replace the due-diligence
/// checklist wholesale (the console sends the full list on every toggle/edit).
#[rocket_okapi::openapi(tag = "Flips")]
#[patch("/modules/flips/deals/<id>/checklist", data = "<body>")]
pub async fn update_checklist(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateChecklistReq>,
) -> ApiResult<Json<DealDto>> {
    user.require(Permission::DealWrite)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "flips").await?;

    let items = body.into_inner().checklist;
    let value = serde_json::to_value(&items)
        .map_err(|_| ApiError::BadRequest("invalid checklist".into()))?;

    let deal = super::load_deal(&db, scope.tenant_id, id).await?;
    let mut m = deal.into_active_model();
    m.checklist = Set(value);
    m.updated_at = Set(Utc::now().into());
    let saved = m.update(&db).await?;

    Ok(Json(DealDto::build(&saved)))
}
