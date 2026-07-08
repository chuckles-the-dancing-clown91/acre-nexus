use super::dto::{
    contractor_names, ChangeOrderDto, CreateProjectReq, RehabDrawDto, RehabLineDto,
    RehabProjectDetailDto, RehabProjectDto, UpdateProjectReq,
};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{
    Counterparty, Property, RehabChangeOrder, RehabDraw, RehabLine, RehabProject,
};
use rocket::serde::json::Json;
use rocket::{get, patch, post, State};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

/// Fetch a project's lines, draws (newest first), and change orders.
async fn related(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    project_id: Uuid,
) -> ApiResult<(
    Vec<entity::rehab_line::Model>,
    Vec<entity::rehab_draw::Model>,
    Vec<entity::rehab_change_order::Model>,
)> {
    let lines = RehabLine::find()
        .filter(entity::rehab_line::Column::TenantId.eq(tenant_id))
        .filter(entity::rehab_line::Column::ProjectId.eq(project_id))
        .order_by_asc(entity::rehab_line::Column::SortOrder)
        .all(db)
        .await?;
    let draws = RehabDraw::find()
        .filter(entity::rehab_draw::Column::TenantId.eq(tenant_id))
        .filter(entity::rehab_draw::Column::ProjectId.eq(project_id))
        .order_by_desc(entity::rehab_draw::Column::Number)
        .all(db)
        .await?;
    let change_orders = RehabChangeOrder::find()
        .filter(entity::rehab_change_order::Column::TenantId.eq(tenant_id))
        .filter(entity::rehab_change_order::Column::ProjectId.eq(project_id))
        .order_by_desc(entity::rehab_change_order::Column::CreatedAt)
        .all(db)
        .await?;
    Ok((lines, draws, change_orders))
}

/// Build the full project detail (roll-up + lines + draws + change orders), with
/// contractor names resolved on the draws.
pub async fn build_project_detail(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    project: &entity::rehab_project::Model,
) -> ApiResult<RehabProjectDetailDto> {
    let (lines, draws, change_orders) = related(db, tenant_id, project.id).await?;

    let ids: Vec<Uuid> = draws.iter().filter_map(|d| d.contractor_id).collect();
    let names = if ids.is_empty() {
        Default::default()
    } else {
        contractor_names(
            &Counterparty::find()
                .filter(entity::counterparty::Column::TenantId.eq(tenant_id))
                .filter(entity::counterparty::Column::Id.is_in(ids))
                .all(db)
                .await?,
        )
    };

    Ok(RehabProjectDetailDto {
        project: RehabProjectDto::build(project, &lines, &draws, &change_orders),
        lines: lines.into_iter().map(RehabLineDto::from).collect(),
        draws: draws
            .iter()
            .map(|d| RehabDrawDto::build(d, d.contractor_id.and_then(|id| names.get(&id).cloned())))
            .collect(),
        change_orders: change_orders
            .into_iter()
            .map(ChangeOrderDto::from)
            .collect(),
    })
}

/// `GET /properties/<id>/rehab-projects` — rehab projects on a property.
#[rocket_okapi::openapi(tag = "Rehab")]
#[get("/properties/<id>/rehab-projects")]
pub async fn list(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Vec<RehabProjectDto>>> {
    user.require(Permission::RehabRead)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "rehab").await?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;

    let projects = RehabProject::find()
        .filter(entity::rehab_project::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::rehab_project::Column::PropertyId.eq(pid))
        .order_by_desc(entity::rehab_project::Column::CreatedAt)
        .all(&db)
        .await?;

    let mut out = Vec::with_capacity(projects.len());
    for p in &projects {
        let (lines, draws, change_orders) = related(&db, scope.tenant_id, p.id).await?;
        out.push(RehabProjectDto::build(p, &lines, &draws, &change_orders));
    }
    Ok(Json(out))
}

/// `POST /properties/<id>/rehab-projects` — start a rehab budget on a property.
#[rocket_okapi::openapi(tag = "Rehab")]
#[post("/properties/<id>/rehab-projects", data = "<body>")]
pub async fn create(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<CreateProjectReq>,
) -> ApiResult<Json<RehabProjectDetailDto>> {
    user.require(Permission::RehabManage)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "rehab").await?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    // The property must exist in this tenant.
    Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;

    let b = body.into_inner();
    let name = b.name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::BadRequest("name is required".into()));
    }
    let now = Utc::now();
    let project = entity::rehab_project::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        property_id: Set(pid),
        name: Set(name.clone()),
        status: Set("planning".into()),
        budget_cents: Set(b.budget_cents.unwrap_or(0).max(0)),
        contingency_bps: Set(b.contingency_bps.unwrap_or(0).clamp(0, 10_000)),
        start_date: Set(b.start_date),
        target_end_date: Set(b.target_end_date),
        notes: Set(b.notes),
        created_by: Set(Some(user.user_id)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::REHAB_PROJECT_CREATE,
        Some("rehab_project"),
        Some(project.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "property_id": pid, "name": name })),
    )
    .await;

    Ok(Json(
        build_project_detail(&db, scope.tenant_id, &project).await?,
    ))
}

/// `GET /rehab-projects/<id>` — full project detail.
#[rocket_okapi::openapi(tag = "Rehab")]
#[get("/rehab-projects/<id>")]
pub async fn get(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<RehabProjectDetailDto>> {
    user.require(Permission::RehabRead)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "rehab").await?;
    let project = super::load_project(&db, scope.tenant_id, id).await?;
    Ok(Json(
        build_project_detail(&db, scope.tenant_id, &project).await?,
    ))
}

/// `PATCH /rehab-projects/<id>` — edit budget / status / dates.
#[rocket_okapi::openapi(tag = "Rehab")]
#[patch("/rehab-projects/<id>", data = "<body>")]
pub async fn update(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateProjectReq>,
) -> ApiResult<Json<RehabProjectDetailDto>> {
    user.require(Permission::RehabManage)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "rehab").await?;
    let project = super::load_project(&db, scope.tenant_id, id).await?;
    let b = body.into_inner();
    let mut m = project.into_active_model();
    if let Some(v) = b.name {
        m.name = Set(v);
    }
    if let Some(v) = b.status {
        if !["planning", "active", "complete", "on_hold"].contains(&v.as_str()) {
            return Err(ApiError::BadRequest(format!("invalid status: {v}")));
        }
        m.status = Set(v);
    }
    if let Some(v) = b.budget_cents {
        m.budget_cents = Set(v.max(0));
    }
    if let Some(v) = b.contingency_bps {
        m.contingency_bps = Set(v.clamp(0, 10_000));
    }
    if let Some(v) = b.start_date {
        m.start_date = Set(Some(v));
    }
    if let Some(v) = b.target_end_date {
        m.target_end_date = Set(Some(v));
    }
    if let Some(v) = b.notes {
        m.notes = Set(Some(v));
    }
    m.updated_at = Set(Utc::now().into());
    let saved = m.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::REHAB_PROJECT_UPDATE,
        Some("rehab_project"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        None,
    )
    .await;

    Ok(Json(
        build_project_detail(&db, scope.tenant_id, &saved).await?,
    ))
}
