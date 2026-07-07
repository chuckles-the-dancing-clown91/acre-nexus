//! Preventive-maintenance plans (Phase 6): recurring tasks (HVAC service,
//! gutter cleaning) that the helpdesk scan turns into tickets on schedule.

use super::dto::{CreatePlanReq, MaintenancePlanDto, UpdatePlanReq};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::{NaiveDate, Utc};
use entity::prelude::{MaintenancePlan, Property};
use rocket::serde::json::Json;
use rocket::{get, patch, post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use uuid::Uuid;

fn valid_date(d: &str) -> Result<(), ApiError> {
    NaiveDate::parse_from_str(d, "%Y-%m-%d")
        .map(|_| ())
        .map_err(|_| ApiError::BadRequest("dates must be YYYY-MM-DD".into()))
}

fn valid_cadence(days: i32) -> Result<(), ApiError> {
    if (1..=3660).contains(&days) {
        Ok(())
    } else {
        Err(ApiError::BadRequest(
            "cadence_days must be between 1 and 3660".into(),
        ))
    }
}

/// `GET /maintenance-plans` — the workspace's preventive plans, next due
/// first.
#[rocket_okapi::openapi(tag = "Maintenance")]
#[get("/maintenance-plans")]
pub async fn list_plans(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<MaintenancePlanDto>>> {
    user.require(Permission::MaintenanceRead)?;
    let rows = MaintenancePlan::find()
        .filter(entity::maintenance_plan::Column::TenantId.eq(scope.tenant_id))
        .order_by_asc(entity::maintenance_plan::Column::NextDueDate)
        .all(&db)
        .await?;
    Ok(Json(
        rows.into_iter().map(MaintenancePlanDto::from).collect(),
    ))
}

/// `POST /maintenance-plans` — create a recurring plan.
#[rocket_okapi::openapi(tag = "Maintenance")]
#[post("/maintenance-plans", data = "<body>")]
pub async fn create_plan(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<CreatePlanReq>,
) -> ApiResult<Json<MaintenancePlanDto>> {
    user.require(Permission::MaintenanceManage)?;
    let b = body.into_inner();
    let title = b.title.trim().to_string();
    if title.is_empty() {
        return Err(ApiError::BadRequest("title is required".into()));
    }
    valid_cadence(b.cadence_days)?;
    valid_date(&b.next_due_date)?;
    Property::find_by_id(b.property_id)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;

    let now = Utc::now();
    let saved = entity::maintenance_plan::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        property_id: Set(b.property_id),
        unit_id: Set(b.unit_id),
        title: Set(title),
        description: Set(b.description.filter(|d| !d.trim().is_empty())),
        category: Set(b
            .category
            .filter(|c| !c.trim().is_empty())
            .unwrap_or_else(|| "general".into())),
        priority: Set(b
            .priority
            .filter(|p| !p.trim().is_empty())
            .unwrap_or_else(|| "normal".into())),
        cadence_days: Set(b.cadence_days),
        next_due_date: Set(b.next_due_date),
        active: Set(true),
        last_ticket_id: Set(None),
        created_by: Set(Some(user.user_id)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::MAINTENANCE_PLAN_CREATE,
        Some("maintenance_plan"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({
            "property_id": saved.property_id,
            "cadence_days": saved.cadence_days,
        })),
    )
    .await;

    Ok(Json(MaintenancePlanDto::from(saved)))
}

/// `PATCH /maintenance-plans/<id>` — edit or pause/resume a plan.
#[rocket_okapi::openapi(tag = "Maintenance")]
#[patch("/maintenance-plans/<id>", data = "<body>")]
pub async fn update_plan(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdatePlanReq>,
) -> ApiResult<Json<MaintenancePlanDto>> {
    user.require(Permission::MaintenanceManage)?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let plan = MaintenancePlan::find_by_id(pid)
        .filter(entity::maintenance_plan::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("plan not found".into()))?;
    let b = body.into_inner();
    if let Some(days) = b.cadence_days {
        valid_cadence(days)?;
    }
    if let Some(d) = &b.next_due_date {
        valid_date(d)?;
    }

    let mut am: entity::maintenance_plan::ActiveModel = plan.into();
    if let Some(v) = b.title.filter(|t| !t.trim().is_empty()) {
        am.title = Set(v);
    }
    if let Some(v) = b.description {
        am.description = Set(Some(v).filter(|d| !d.trim().is_empty()));
    }
    if let Some(v) = b.category.filter(|c| !c.trim().is_empty()) {
        am.category = Set(v);
    }
    if let Some(v) = b.priority.filter(|p| !p.trim().is_empty()) {
        am.priority = Set(v);
    }
    if let Some(v) = b.cadence_days {
        am.cadence_days = Set(v);
    }
    if let Some(v) = b.next_due_date {
        am.next_due_date = Set(v);
    }
    if let Some(v) = b.active {
        am.active = Set(v);
    }
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::MAINTENANCE_PLAN_UPDATE,
        Some("maintenance_plan"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "active": saved.active })),
    )
    .await;

    Ok(Json(MaintenancePlanDto::from(saved)))
}
