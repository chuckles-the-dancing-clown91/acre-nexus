use super::dto::{CreateChangeOrderReq, DecideReq, RehabProjectDetailDto};
use super::projects::build_project_detail;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{RehabChangeOrder, RehabProject};
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set};
use uuid::Uuid;

/// `POST /rehab-projects/<id>/change-orders` — propose a budget change.
#[rocket_okapi::openapi(tag = "Rehab")]
#[post("/rehab-projects/<id>/change-orders", data = "<body>")]
pub async fn create(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<CreateChangeOrderReq>,
) -> ApiResult<Json<RehabProjectDetailDto>> {
    user.require(Permission::RehabManage)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "rehab").await?;
    let project = super::load_project(&db, scope.tenant_id, id).await?;
    let b = body.into_inner();
    let description = b.description.trim().to_string();
    if description.is_empty() {
        return Err(ApiError::BadRequest("description is required".into()));
    }
    let co = entity::rehab_change_order::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        project_id: Set(project.id),
        description: Set(description),
        amount_cents: Set(b.amount_cents),
        status: Set("pending".into()),
        created_by: Set(Some(user.user_id)),
        approved_by: Set(None),
        created_at: Set(Utc::now().into()),
        decided_at: Set(None),
    }
    .insert(&db)
    .await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::REHAB_CHANGE_ORDER,
        Some("rehab_change_order"),
        Some(co.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "amount_cents": co.amount_cents, "status": "pending" })),
    )
    .await;

    Ok(Json(
        build_project_detail(&db, scope.tenant_id, &project).await?,
    ))
}

/// `POST /rehab-change-orders/<id>/decide` — approve or reject a change order.
/// Approving rolls the delta into the project's adjusted budget.
#[rocket_okapi::openapi(tag = "Rehab")]
#[post("/rehab-change-orders/<id>/decide", data = "<body>")]
pub async fn decide(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<DecideReq>,
) -> ApiResult<Json<RehabProjectDetailDto>> {
    user.require(Permission::RehabManage)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "rehab").await?;
    let cid =
        Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid change-order id".into()))?;
    let co = RehabChangeOrder::find_by_id(cid)
        .filter(entity::rehab_change_order::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("change order not found".into()))?;
    if co.status != "pending" {
        return Err(ApiError::Conflict("change order already decided".into()));
    }
    let project_id = co.project_id;
    let approve = body.into_inner().approve;
    let mut m = co.into_active_model();
    m.status = Set(if approve { "approved" } else { "rejected" }.into());
    m.approved_by = Set(Some(user.user_id));
    m.decided_at = Set(Some(Utc::now().into()));
    let saved = m.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::REHAB_CHANGE_ORDER,
        Some("rehab_change_order"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "status": saved.status })),
    )
    .await;

    let project = RehabProject::find_by_id(project_id)
        .filter(entity::rehab_project::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("rehab project not found".into()))?;
    Ok(Json(
        build_project_detail(&db, scope.tenant_id, &project).await?,
    ))
}
