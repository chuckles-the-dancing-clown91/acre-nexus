use super::dto::{CreateLineReq, RehabProjectDetailDto, UpdateLineReq};
use super::projects::build_project_detail;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{RehabLine, RehabProject};
use rocket::serde::json::Json;
use rocket::{delete, patch, post, State};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, ModelTrait, QueryFilter, Set,
};
use uuid::Uuid;

async fn load_line(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    id: &str,
) -> ApiResult<entity::rehab_line::Model> {
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid line id".into()))?;
    RehabLine::find_by_id(lid)
        .filter(entity::rehab_line::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("rehab line not found".into()))
}

async fn detail_for_project(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    project_id: Uuid,
) -> ApiResult<Json<RehabProjectDetailDto>> {
    let project = RehabProject::find_by_id(project_id)
        .filter(entity::rehab_project::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("rehab project not found".into()))?;
    Ok(Json(build_project_detail(db, tenant_id, &project).await?))
}

/// `POST /rehab-projects/<id>/lines` — add a scope / budget line.
#[rocket_okapi::openapi(tag = "Rehab")]
#[post("/rehab-projects/<id>/lines", data = "<body>")]
pub async fn create(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<CreateLineReq>,
) -> ApiResult<Json<RehabProjectDetailDto>> {
    user.require(Permission::RehabManage)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "rehab").await?;
    let project = super::load_project(&db, scope.tenant_id, id).await?;
    let b = body.into_inner();
    let category = b.category.trim().to_string();
    if category.is_empty() {
        return Err(ApiError::BadRequest("category is required".into()));
    }
    entity::rehab_line::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        project_id: Set(project.id),
        category: Set(category),
        description: Set(b.description),
        budget_cents: Set(b.budget_cents.unwrap_or(0).max(0)),
        sort_order: Set(b.sort_order.unwrap_or(0)),
        created_at: Set(Utc::now().into()),
    }
    .insert(&db)
    .await?;
    Ok(Json(
        build_project_detail(&db, scope.tenant_id, &project).await?,
    ))
}

/// `PATCH /rehab-lines/<id>` — edit a scope line.
#[rocket_okapi::openapi(tag = "Rehab")]
#[patch("/rehab-lines/<id>", data = "<body>")]
pub async fn update(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateLineReq>,
) -> ApiResult<Json<RehabProjectDetailDto>> {
    user.require(Permission::RehabManage)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "rehab").await?;
    let line = load_line(&db, scope.tenant_id, id).await?;
    let project_id = line.project_id;
    let b = body.into_inner();
    let mut m = line.into_active_model();
    if let Some(v) = b.category {
        m.category = Set(v);
    }
    if let Some(v) = b.description {
        m.description = Set(Some(v));
    }
    if let Some(v) = b.budget_cents {
        m.budget_cents = Set(v.max(0));
    }
    if let Some(v) = b.sort_order {
        m.sort_order = Set(v);
    }
    m.update(&db).await?;
    detail_for_project(&db, scope.tenant_id, project_id).await
}

/// `DELETE /rehab-lines/<id>` — remove a scope line.
#[rocket_okapi::openapi(tag = "Rehab")]
#[delete("/rehab-lines/<id>")]
pub async fn delete(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<RehabProjectDetailDto>> {
    user.require(Permission::RehabManage)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "rehab").await?;
    let line = load_line(&db, scope.tenant_id, id).await?;
    let project_id = line.project_id;
    line.delete(&db).await?;
    detail_for_project(&db, scope.tenant_id, project_id).await
}
