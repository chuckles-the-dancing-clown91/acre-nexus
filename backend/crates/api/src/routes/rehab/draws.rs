use super::dto::{
    CreateDrawReq, DrawStatusReq, LienWaiverDto, RehabDrawDetailDto, RehabDrawDto,
    RehabProjectDetailDto,
};
use super::projects::build_project_detail;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{Counterparty, RehabDraw, RehabLienWaiver};
use rocket::serde::json::Json;
use rocket::{get, patch, post, State};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use uuid::Uuid;

const DRAW_STATUSES: &[&str] = &["requested", "approved", "funded", "rejected"];

/// Build a draw's detail view (draw + resolved contractor + lien waivers).
pub async fn build_draw_detail(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    draw: &entity::rehab_draw::Model,
) -> ApiResult<RehabDrawDetailDto> {
    let contractor_name = match draw.contractor_id {
        Some(cid) => Counterparty::find_by_id(cid)
            .filter(entity::counterparty::Column::TenantId.eq(tenant_id))
            .one(db)
            .await?
            .map(|c| c.name),
        None => None,
    };
    let waivers = RehabLienWaiver::find()
        .filter(entity::rehab_lien_waiver::Column::TenantId.eq(tenant_id))
        .filter(entity::rehab_lien_waiver::Column::DrawId.eq(draw.id))
        .order_by_desc(entity::rehab_lien_waiver::Column::CreatedAt)
        .all(db)
        .await?;
    Ok(RehabDrawDetailDto {
        draw: RehabDrawDto::build(draw, contractor_name),
        lien_waivers: waivers.into_iter().map(LienWaiverDto::from).collect(),
    })
}

/// `POST /rehab-projects/<id>/draws` — request a draw against the budget.
#[rocket_okapi::openapi(tag = "Rehab")]
#[post("/rehab-projects/<id>/draws", data = "<body>")]
pub async fn create(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<CreateDrawReq>,
) -> ApiResult<Json<RehabProjectDetailDto>> {
    user.require(Permission::RehabManage)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "rehab").await?;
    let project = super::load_project(&db, scope.tenant_id, id).await?;
    let b = body.into_inner();
    let title = b.title.trim().to_string();
    if title.is_empty() {
        return Err(ApiError::BadRequest("title is required".into()));
    }

    let count = RehabDraw::find()
        .filter(entity::rehab_draw::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::rehab_draw::Column::ProjectId.eq(project.id))
        .count(&db)
        .await?;
    let now = Utc::now();
    let draw = entity::rehab_draw::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        project_id: Set(project.id),
        number: Set(count as i32 + 1),
        title: Set(title),
        amount_cents: Set(b.amount_cents.max(0)),
        status: Set("requested".into()),
        contractor_id: Set(b.contractor_id),
        notes: Set(b.notes),
        requested_by: Set(Some(user.user_id)),
        approved_by: Set(None),
        funded_at: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::REHAB_DRAW_CREATE,
        Some("rehab_draw"),
        Some(draw.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "amount_cents": draw.amount_cents, "number": draw.number })),
    )
    .await;

    Ok(Json(
        build_project_detail(&db, scope.tenant_id, &project).await?,
    ))
}

/// `GET /rehab-draws/<id>` — a draw with its lien waivers.
#[rocket_okapi::openapi(tag = "Rehab")]
#[get("/rehab-draws/<id>")]
pub async fn get(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<RehabDrawDetailDto>> {
    user.require(Permission::RehabRead)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "rehab").await?;
    let draw = super::load_draw(&db, scope.tenant_id, id).await?;
    Ok(Json(build_draw_detail(&db, scope.tenant_id, &draw).await?))
}

/// `PATCH /rehab-draws/<id>/status` — move a draw through `requested → approved →
/// funded` (or `rejected`). Funding stamps `funded_at`.
#[rocket_okapi::openapi(tag = "Rehab")]
#[patch("/rehab-draws/<id>/status", data = "<body>")]
pub async fn set_status(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<DrawStatusReq>,
) -> ApiResult<Json<RehabProjectDetailDto>> {
    user.require(Permission::RehabManage)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "rehab").await?;
    let draw = super::load_draw(&db, scope.tenant_id, id).await?;
    let status = body.into_inner().status;
    if !DRAW_STATUSES.contains(&status.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "invalid draw status: {status}"
        )));
    }
    let project_id = draw.project_id;
    let now = Utc::now();
    let mut m = draw.into_active_model();
    if status == "approved" {
        m.approved_by = Set(Some(user.user_id));
    }
    if status == "funded" {
        m.funded_at = Set(Some(now.into()));
    }
    m.status = Set(status.clone());
    m.updated_at = Set(now.into());
    let saved = m.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::REHAB_DRAW_STATUS,
        Some("rehab_draw"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "status": status })),
    )
    .await;

    let project = super::load_project(&db, scope.tenant_id, &project_id.to_string()).await?;
    Ok(Json(
        build_project_detail(&db, scope.tenant_id, &project).await?,
    ))
}
