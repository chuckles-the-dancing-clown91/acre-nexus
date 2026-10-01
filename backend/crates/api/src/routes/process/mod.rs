//! **Step processes** over HTTP — today the unit turnover: templates (the
//! recipe), runs against a unit, step transitions, work orders opened from a
//! step, and the finish gate. The rules live in [`crate::process`].

use crate::audit::actions as act;
use crate::audit::change::{self, Ctx};
use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::{ApiError, ApiResult};
use crate::process::{self as engine, StepState};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::{NaiveDate, Utc};
use entity::prelude::{
    Document, MaintenanceTicket, Process, ProcessStep, ProcessTemplate, ProcessTemplateStep,
    Property, Unit,
};
use rocket::serde::json::Json;
use rocket::{get, post, put, State};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Shapes
// ---------------------------------------------------------------------------

#[derive(Serialize, schemars::JsonSchema)]
pub struct StepDto {
    pub id: Uuid,
    pub position: i32,
    pub key: String,
    pub title: String,
    pub description: Option<String>,
    pub owner_role: String,
    pub assignee_user_id: Option<Uuid>,
    pub depends_on: Vec<String>,
    /// Titles of the steps this one waits on.
    pub waiting_on: Vec<String>,
    pub due_on: Option<String>,
    pub overdue: bool,
    pub required: bool,
    pub requires_photo: bool,
    pub status: String,
    pub done_at: Option<String>,
    pub skip_reason: Option<String>,
    pub note: Option<String>,
    pub ticket_id: Option<Uuid>,
    pub ticket_status: Option<String>,
    pub cost_cents: Option<i64>,
    pub cost_label: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ProcessDto {
    pub id: Uuid,
    pub kind: String,
    pub property_id: Uuid,
    pub property_name: String,
    pub unit_id: Option<Uuid>,
    pub unit_number: Option<String>,
    pub title: String,
    pub status: String,
    pub started_on: String,
    pub target_date: Option<String>,
    pub finished_on: Option<String>,
    pub override_reason: Option<String>,
    pub done: usize,
    pub total: usize,
    /// Required steps not yet finished.
    pub unmet_required: Vec<String>,
    /// Days from start to now (active) or to finish (done): days vacant.
    pub days_open: i64,
    pub overdue: bool,
    pub cost_cents: i64,
    pub cost_label: String,
    /// Present on the detail view.
    pub steps: Option<Vec<StepDto>>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct TemplateStepDto {
    pub key: String,
    pub title: String,
    pub description: Option<String>,
    pub owner_role: String,
    pub depends_on: Vec<String>,
    pub due_offset_days: i32,
    pub required: bool,
    pub requires_photo: bool,
    pub ticket_category: Option<String>,
    pub ticket_priority: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct TemplateDto {
    pub id: Uuid,
    pub kind: String,
    pub name: String,
    pub is_default: bool,
    pub active: bool,
    pub steps: Vec<TemplateStepDto>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct TemplateStepReq {
    pub key: String,
    pub title: String,
    pub description: Option<String>,
    pub owner_role: Option<String>,
    pub depends_on: Option<Vec<String>>,
    pub due_offset_days: Option<i32>,
    pub required: Option<bool>,
    pub requires_photo: Option<bool>,
    pub ticket_category: Option<String>,
    pub ticket_priority: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SaveTemplateReq {
    pub kind: Option<String>,
    pub name: String,
    pub is_default: Option<bool>,
    pub steps: Vec<TemplateStepReq>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct StartTurnReq {
    pub template_id: Option<Uuid>,
    /// ISO date the unit was vacated (default today).
    pub started_on: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct StepActionReq {
    /// `start` | `complete` | `skip` | `reopen` | `assign` | `note`.
    pub action: String,
    pub note: Option<String>,
    /// Required to skip.
    pub reason: Option<String>,
    pub assignee_user_id: Option<Uuid>,
    pub cost_cents: Option<i64>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct StepTicketReq {
    pub title: Option<String>,
    pub description: Option<String>,
    pub priority: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct FinishReq {
    /// Needed when required steps are still open.
    pub override_reason: Option<String>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn pid(raw: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(raw).map_err(|_| ApiError::BadRequest("invalid id".into()))
}

fn clean(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn valid_date(d: &str) -> Result<String, ApiError> {
    NaiveDate::parse_from_str(d.trim(), "%Y-%m-%d")
        .map(|_| d.trim().to_string())
        .map_err(|_| ApiError::BadRequest("date must be YYYY-MM-DD".into()))
}

fn states(rows: &[entity::process_step::Model]) -> Vec<StepState> {
    rows.iter()
        .map(|s| StepState {
            key: s.key.clone(),
            title: s.title.clone(),
            status: s.status.clone(),
            required: s.required,
            depends_on: engine::deps_of(&s.depends_on),
        })
        .collect()
}

async fn find_process(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    id: Uuid,
) -> ApiResult<entity::process::Model> {
    Process::find_by_id(id)
        .filter(entity::process::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("process not found".into()))
}

async fn build_dtos(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    procs: Vec<entity::process::Model>,
    with_steps: bool,
) -> ApiResult<Vec<ProcessDto>> {
    let today = engine::today();
    let prop_ids: Vec<Uuid> = procs.iter().map(|p| p.property_id).collect();
    let props: HashMap<Uuid, String> = Property::find()
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .filter(entity::property::Column::Id.is_in(prop_ids))
        .all(db)
        .await?
        .into_iter()
        .map(|p| (p.id, p.name))
        .collect();
    let unit_ids: Vec<Uuid> = procs.iter().filter_map(|p| p.unit_id).collect();
    let units: HashMap<Uuid, String> = Unit::find()
        .filter(entity::unit::Column::TenantId.eq(tenant_id))
        .filter(entity::unit::Column::Id.is_in(unit_ids))
        .all(db)
        .await?
        .into_iter()
        .map(|u| (u.id, u.unit_number))
        .collect();

    let mut out = Vec::with_capacity(procs.len());
    for p in procs {
        let rows = engine::steps_of(db, tenant_id, p.id).await?;
        let st = states(&rows);
        let (done, total) = engine::progress(&st);
        let cost: i64 = rows.iter().filter_map(|s| s.cost_cents).sum();
        let end = p.finished_on.clone().unwrap_or_else(|| today.clone());
        let overdue =
            p.status == "active" && p.target_date.as_deref().is_some_and(|t| t < today.as_str());
        let steps = if with_steps {
            let titles: HashMap<&str, &str> = rows
                .iter()
                .map(|s| (s.key.as_str(), s.title.as_str()))
                .collect();
            let ticket_ids: Vec<Uuid> = rows.iter().filter_map(|s| s.ticket_id).collect();
            let tickets: HashMap<Uuid, String> = MaintenanceTicket::find()
                .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
                .filter(entity::maintenance_ticket::Column::Id.is_in(ticket_ids))
                .all(db)
                .await?
                .into_iter()
                .map(|t| (t.id, t.status))
                .collect();
            Some(
                rows.iter()
                    .map(|s| {
                        let deps = engine::deps_of(&s.depends_on);
                        let waiting_on = if engine::finished(&s.status) {
                            vec![]
                        } else {
                            deps.iter()
                                .filter(|d| {
                                    rows.iter()
                                        .find(|r| &r.key == *d)
                                        .is_some_and(|r| !engine::finished(&r.status))
                                })
                                .filter_map(|d| titles.get(d.as_str()).map(|t| t.to_string()))
                                .collect()
                        };
                        StepDto {
                            id: s.id,
                            position: s.position,
                            key: s.key.clone(),
                            title: s.title.clone(),
                            description: s.description.clone(),
                            owner_role: s.owner_role.clone(),
                            assignee_user_id: s.assignee_user_id,
                            depends_on: deps,
                            waiting_on,
                            due_on: s.due_on.clone(),
                            overdue: !engine::finished(&s.status)
                                && p.status == "active"
                                && s.due_on.as_deref().is_some_and(|d| d < today.as_str()),
                            required: s.required,
                            requires_photo: s.requires_photo,
                            status: s.status.clone(),
                            done_at: s.done_at.map(|d| d.to_rfc3339()),
                            skip_reason: s.skip_reason.clone(),
                            note: s.note.clone(),
                            ticket_id: s.ticket_id,
                            ticket_status: s.ticket_id.and_then(|t| tickets.get(&t).cloned()),
                            cost_cents: s.cost_cents,
                            cost_label: s.cost_cents.map(usd),
                        }
                    })
                    .collect(),
            )
        } else {
            None
        };
        out.push(ProcessDto {
            id: p.id,
            kind: p.kind.clone(),
            property_id: p.property_id,
            property_name: props.get(&p.property_id).cloned().unwrap_or_default(),
            unit_id: p.unit_id,
            unit_number: p.unit_id.and_then(|u| units.get(&u).cloned()),
            title: p.title.clone(),
            status: p.status.clone(),
            started_on: p.started_on.clone(),
            target_date: p.target_date.clone(),
            finished_on: p.finished_on.clone(),
            override_reason: p.override_reason.clone(),
            done,
            total,
            unmet_required: engine::unmet_required(&st),
            days_open: engine::days_between(&p.started_on, &end),
            overdue,
            cost_cents: cost,
            cost_label: usd(cost),
            steps,
        });
    }
    Ok(out)
}

async fn template_dto(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    t: entity::process_template::Model,
) -> ApiResult<TemplateDto> {
    let steps = ProcessTemplateStep::find()
        .filter(entity::process_template_step::Column::TenantId.eq(tenant_id))
        .filter(entity::process_template_step::Column::TemplateId.eq(t.id))
        .order_by_asc(entity::process_template_step::Column::Position)
        .all(db)
        .await?;
    Ok(TemplateDto {
        id: t.id,
        kind: t.kind,
        name: t.name,
        is_default: t.is_default,
        active: t.active,
        steps: steps
            .into_iter()
            .map(|s| TemplateStepDto {
                depends_on: engine::deps_of(&s.depends_on),
                key: s.key,
                title: s.title,
                description: s.description,
                owner_role: s.owner_role,
                due_offset_days: s.due_offset_days,
                required: s.required,
                requires_photo: s.requires_photo,
                ticket_category: s.ticket_category,
                ticket_priority: s.ticket_priority,
            })
            .collect(),
    })
}

// ---------------------------------------------------------------------------
// Templates
// ---------------------------------------------------------------------------

/// `GET /process-templates?kind` — the recipes (the default is created on first use).
#[rocket_okapi::openapi(tag = "Turnover")]
#[get("/process-templates?<kind>")]
pub async fn list_templates(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    kind: Option<String>,
) -> ApiResult<Json<Vec<TemplateDto>>> {
    user.require(Permission::MaintenanceRead)?;
    let kind = kind
        .filter(|k| !k.trim().is_empty())
        .unwrap_or_else(|| engine::KIND_TURNOVER.into());
    if !engine::KINDS.contains(&kind.as_str()) {
        return Err(ApiError::BadRequest("unknown kind".into()));
    }
    engine::ensure_default_template(&db, scope.tenant_id, &kind).await?;
    let rows = ProcessTemplate::find()
        .filter(entity::process_template::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::process_template::Column::Kind.eq(kind))
        .filter(entity::process_template::Column::Active.eq(true))
        .order_by_desc(entity::process_template::Column::IsDefault)
        .order_by_asc(entity::process_template::Column::Name)
        .all(&db)
        .await?;
    let mut out = Vec::new();
    for t in rows {
        out.push(template_dto(&db, scope.tenant_id, t).await?);
    }
    Ok(Json(out))
}

fn check_steps(steps: &[TemplateStepReq]) -> Result<(), ApiError> {
    for s in steps {
        if s.title.trim().is_empty() {
            return Err(ApiError::BadRequest("every step needs a title".into()));
        }
        if let Some(r) = s.owner_role.as_deref() {
            if !engine::OWNER_ROLES.contains(&r) {
                return Err(ApiError::BadRequest(format!("invalid owner_role: {r}")));
            }
        }
    }
    let graph: Vec<(String, Vec<String>)> = steps
        .iter()
        .map(|s| {
            (
                s.key.trim().to_string(),
                s.depends_on.clone().unwrap_or_default(),
            )
        })
        .collect();
    engine::validate_graph(&graph).map_err(ApiError::BadRequest)
}

async fn write_template(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    template: entity::process_template::Model,
    b: SaveTemplateReq,
) -> ApiResult<entity::process_template::Model> {
    use entity::process_template_step as ts;
    ts::Entity::delete_many()
        .filter(ts::Column::TenantId.eq(tenant_id))
        .filter(ts::Column::TemplateId.eq(template.id))
        .exec(db)
        .await?;
    for (i, s) in b.steps.into_iter().enumerate() {
        ts::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            template_id: Set(template.id),
            position: Set(i as i32),
            key: Set(s.key.trim().to_string()),
            title: Set(s.title.trim().to_string()),
            description: Set(clean(s.description)),
            owner_role: Set(s.owner_role.unwrap_or_else(|| "office".into())),
            depends_on: Set(serde_json::json!(s.depends_on.unwrap_or_default())),
            due_offset_days: Set(s.due_offset_days.unwrap_or(0).clamp(-365, 365)),
            required: Set(s.required.unwrap_or(true)),
            requires_photo: Set(s.requires_photo.unwrap_or(false)),
            ticket_category: Set(clean(s.ticket_category)),
            ticket_priority: Set(clean(s.ticket_priority)),
        }
        .insert(db)
        .await?;
    }
    Ok(template)
}

async fn make_default(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    kind: &str,
    id: Uuid,
) -> ApiResult<()> {
    use entity::process_template as t;
    let others = t::Entity::find()
        .filter(t::Column::TenantId.eq(tenant_id))
        .filter(t::Column::Kind.eq(kind))
        .filter(t::Column::IsDefault.eq(true))
        .filter(t::Column::Id.ne(id))
        .all(db)
        .await?;
    for o in others {
        let mut am: t::ActiveModel = o.into();
        am.is_default = Set(false);
        am.update(db).await?;
    }
    Ok(())
}

/// `POST /process-templates` — add a recipe.
#[rocket_okapi::openapi(tag = "Turnover")]
#[post("/process-templates", data = "<body>")]
pub async fn create_template(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<SaveTemplateReq>,
) -> ApiResult<Json<TemplateDto>> {
    user.require(Permission::MaintenanceManage)?;
    let b = body.into_inner();
    let kind = b
        .kind
        .clone()
        .unwrap_or_else(|| engine::KIND_TURNOVER.into());
    if !engine::KINDS.contains(&kind.as_str()) {
        return Err(ApiError::BadRequest("unknown kind".into()));
    }
    if b.name.trim().is_empty() {
        return Err(ApiError::BadRequest("name is required".into()));
    }
    check_steps(&b.steps)?;
    let now = Utc::now();
    let make = b.is_default.unwrap_or(false);
    let saved = entity::process_template::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        kind: Set(kind.clone()),
        name: Set(b.name.trim().to_string()),
        is_default: Set(make),
        active: Set(true),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;
    let name = saved.name.clone();
    let saved = write_template(&db, scope.tenant_id, saved, b).await?;
    if make {
        make_default(&db, scope.tenant_id, &kind, saved.id).await?;
    }
    change::created(
        &db,
        Ctx::new(&user, &scope),
        act::PROCESS_TEMPLATE_SAVE,
        "process_template",
        saved.id,
        None,
        &format!("Turnover template {name}"),
    )
    .await;
    Ok(Json(template_dto(&db, scope.tenant_id, saved).await?))
}

/// `PUT /process-templates/<id>` — replace a recipe's name and steps. Runs
/// already started keep the steps they were created with.
#[rocket_okapi::openapi(tag = "Turnover")]
#[put("/process-templates/<id>", data = "<body>")]
pub async fn update_template(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<SaveTemplateReq>,
) -> ApiResult<Json<TemplateDto>> {
    user.require(Permission::MaintenanceManage)?;
    let tid = pid(id)?;
    let existing = ProcessTemplate::find_by_id(tid)
        .filter(entity::process_template::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("template not found".into()))?;
    let b = body.into_inner();
    if b.name.trim().is_empty() {
        return Err(ApiError::BadRequest("name is required".into()));
    }
    check_steps(&b.steps)?;
    let before = template_dto(&db, scope.tenant_id, existing.clone()).await?;
    let kind = existing.kind.clone();
    let make = b.is_default.unwrap_or(existing.is_default);
    let mut am: entity::process_template::ActiveModel = existing.into();
    am.name = Set(b.name.trim().to_string());
    am.is_default = Set(make);
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&db).await?;
    let saved = write_template(&db, scope.tenant_id, saved, b).await?;
    if make {
        make_default(&db, scope.tenant_id, &kind, saved.id).await?;
    }
    let after = template_dto(&db, scope.tenant_id, saved).await?;
    change::change(
        &db,
        Ctx::new(&user, &scope),
        act::PROCESS_TEMPLATE_SAVE,
        "process_template",
        after.id,
        None,
        &format!("Turnover template {}", after.name),
        &serde_json::to_value(&before).unwrap_or_default(),
        &serde_json::to_value(&after).unwrap_or_default(),
    )
    .await;
    Ok(Json(after))
}

// ---------------------------------------------------------------------------
// Runs
// ---------------------------------------------------------------------------

/// `GET /processes?kind&status&property_id&unit_id` — turns, newest first.
#[allow(clippy::too_many_arguments)]
#[rocket_okapi::openapi(tag = "Turnover")]
#[get("/processes?<kind>&<status>&<property_id>&<unit_id>")]
pub async fn list_processes(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    kind: Option<String>,
    status: Option<String>,
    property_id: Option<String>,
    unit_id: Option<String>,
) -> ApiResult<Json<Vec<ProcessDto>>> {
    user.require(Permission::MaintenanceRead)?;
    let mut q = Process::find().filter(entity::process::Column::TenantId.eq(scope.tenant_id));
    if let Some(k) = kind.filter(|s| !s.trim().is_empty()) {
        q = q.filter(entity::process::Column::Kind.eq(k));
    }
    if let Some(s) = status.filter(|s| !s.trim().is_empty()) {
        q = q.filter(entity::process::Column::Status.eq(s));
    }
    if let Some(p) = property_id.filter(|s| !s.trim().is_empty()) {
        q = q.filter(entity::process::Column::PropertyId.eq(pid(&p)?));
    }
    if let Some(u) = unit_id.filter(|s| !s.trim().is_empty()) {
        q = q.filter(entity::process::Column::UnitId.eq(pid(&u)?));
    }
    let rows = q
        .order_by_desc(entity::process::Column::CreatedAt)
        .all(&db)
        .await?;
    Ok(Json(build_dtos(&db, scope.tenant_id, rows, false).await?))
}

/// `GET /processes/<id>` — one turn with every step.
#[rocket_okapi::openapi(tag = "Turnover")]
#[get("/processes/<id>")]
pub async fn get_process(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<ProcessDto>> {
    user.require(Permission::MaintenanceRead)?;
    let p = find_process(&db, scope.tenant_id, pid(id)?).await?;
    let mut v = build_dtos(&db, scope.tenant_id, vec![p], true).await?;
    Ok(Json(v.remove(0)))
}

/// `POST /units/<id>/turn` — start a turnover on a unit.
#[rocket_okapi::openapi(tag = "Turnover")]
#[post("/units/<id>/turn", data = "<body>")]
pub async fn start_turn(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<StartTurnReq>,
) -> ApiResult<Json<ProcessDto>> {
    user.require(Permission::MaintenanceManage)?;
    let uid = pid(id)?;
    let unit = Unit::find_by_id(uid)
        .filter(entity::unit::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("unit not found".into()))?;
    let b = body.into_inner();
    let started = match b.started_on.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(d) => valid_date(d)?,
        None => engine::today(),
    };
    let proc = engine::start(
        &db,
        scope.tenant_id,
        engine::KIND_TURNOVER,
        unit.property_id,
        Some(unit.id),
        None,
        b.template_id,
        &started,
        format!("Turn unit {}", unit.unit_number),
        Some(user.user_id),
    )
    .await?;
    change::created(
        &db,
        Ctx::new(&user, &scope),
        act::PROCESS_START,
        "process",
        proc.id,
        Some(proc.property_id),
        &proc.title,
    )
    .await;
    let mut v = build_dtos(&db, scope.tenant_id, vec![proc], true).await?;
    Ok(Json(v.remove(0)))
}

/// `POST /process-steps/<id>/action` — start, complete, skip, reopen, assign
/// or annotate a step. Completing a step that needs a photo needs a document
/// attached to it.
#[rocket_okapi::openapi(tag = "Turnover")]
#[post("/process-steps/<id>/action", data = "<body>")]
pub async fn step_action(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<StepActionReq>,
) -> ApiResult<Json<ProcessDto>> {
    user.require(Permission::MaintenanceManage)?;
    let step = find_step(&db, scope.tenant_id, pid(id)?).await?;
    let proc = find_process(&db, scope.tenant_id, step.process_id).await?;
    if proc.status != "active" {
        return Err(ApiError::Conflict("this turn is no longer active".into()));
    }
    let b = body.into_inner();
    let before = step.clone();
    let now = Utc::now();
    let status = step.status.clone();
    let mut am: entity::process_step::ActiveModel = step.clone().into();
    match b.action.as_str() {
        "start" => {
            if status != engine::READY {
                return Err(ApiError::Conflict(if status == engine::BLOCKED {
                    "this step is waiting on earlier steps".into()
                } else {
                    format!("cannot start a {status} step")
                }));
            }
            am.status = Set(engine::DOING.into());
            am.started_at = Set(Some(now.into()));
        }
        "complete" => {
            if engine::finished(&status) {
                return Err(ApiError::Conflict("already finished".into()));
            }
            if status == engine::BLOCKED {
                return Err(ApiError::Conflict(
                    "this step is waiting on earlier steps".into(),
                ));
            }
            if step.requires_photo {
                let n = Document::find()
                    .filter(entity::document::Column::TenantId.eq(scope.tenant_id))
                    .filter(entity::document::Column::OwnerType.eq("process_step"))
                    .filter(entity::document::Column::OwnerId.eq(step.id))
                    .count(&db)
                    .await?;
                if n == 0 {
                    return Err(ApiError::Conflict(
                        "attach a photo or document before completing this step".into(),
                    ));
                }
            }
            am.status = Set(engine::DONE.into());
            am.done_at = Set(Some(now.into()));
            am.done_by = Set(Some(user.user_id));
            if let Some(c) = b.cost_cents.filter(|c| *c >= 0) {
                am.cost_cents = Set(Some(c));
            }
            if let Some(n) = clean(b.note.clone()) {
                am.note = Set(Some(n));
            }
        }
        "skip" => {
            if engine::finished(&status) {
                return Err(ApiError::Conflict("already finished".into()));
            }
            let reason = clean(b.reason.clone())
                .ok_or_else(|| ApiError::BadRequest("a reason is required to skip".into()))?;
            am.status = Set(engine::SKIPPED.into());
            am.skip_reason = Set(Some(reason));
            am.done_at = Set(Some(now.into()));
            am.done_by = Set(Some(user.user_id));
        }
        "reopen" => {
            if !engine::finished(&status) {
                return Err(ApiError::Conflict("this step is not finished".into()));
            }
            am.status = Set(engine::READY.into());
            am.done_at = Set(None);
            am.done_by = Set(None);
            am.skip_reason = Set(None);
        }
        "assign" => {
            am.assignee_user_id = Set(b.assignee_user_id);
        }
        "note" => {
            am.note = Set(clean(b.note.clone()));
        }
        other => {
            return Err(ApiError::BadRequest(format!("unknown action: {other}")));
        }
    }
    am.updated_at = Set(now.into());
    let saved = am.update(&db).await?;
    change::change(
        &db,
        Ctx::new(&user, &scope),
        act::PROCESS_STEP_UPDATE,
        "process_step",
        saved.id,
        Some(proc.property_id),
        &format!("{} — {}", proc.title, saved.title),
        &before,
        &saved,
    )
    .await;
    engine::refresh(&db, scope.tenant_id, proc.id).await?;
    let mut v = build_dtos(&db, scope.tenant_id, vec![proc], true).await?;
    Ok(Json(v.remove(0)))
}

async fn find_step(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    id: Uuid,
) -> ApiResult<entity::process_step::Model> {
    ProcessStep::find_by_id(id)
        .filter(entity::process_step::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("step not found".into()))
}

/// `POST /process-steps/<id>/ticket` — open a work order from a step and link
/// them. Resolving the ticket completes the step and brings its cost in.
#[rocket_okapi::openapi(tag = "Turnover")]
#[post("/process-steps/<id>/ticket", data = "<body>")]
pub async fn step_ticket(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<StepTicketReq>,
) -> ApiResult<Json<ProcessDto>> {
    user.require(Permission::MaintenanceManage)?;
    let step = find_step(&db, scope.tenant_id, pid(id)?).await?;
    let proc = find_process(&db, scope.tenant_id, step.process_id).await?;
    if proc.status != "active" {
        return Err(ApiError::Conflict("this turn is no longer active".into()));
    }
    if step.ticket_id.is_some() {
        return Err(ApiError::Conflict(
            "this step already has a work order".into(),
        ));
    }
    if engine::finished(&step.status) {
        return Err(ApiError::Conflict("this step is already finished".into()));
    }
    let b = body.into_inner();
    // The template step decides category and priority when it says so.
    let tstep = match proc.template_id {
        Some(tid) => {
            ProcessTemplateStep::find()
                .filter(entity::process_template_step::Column::TenantId.eq(scope.tenant_id))
                .filter(entity::process_template_step::Column::TemplateId.eq(tid))
                .filter(entity::process_template_step::Column::Key.eq(step.key.clone()))
                .one(&db)
                .await?
        }
        None => None,
    };
    let category = tstep
        .as_ref()
        .and_then(|t| t.ticket_category.clone())
        .unwrap_or_else(|| "general".into());
    let priority = clean(b.priority)
        .or_else(|| tstep.as_ref().and_then(|t| t.ticket_priority.clone()))
        .unwrap_or_else(|| "normal".into());
    let unit_label = proc.title.clone();
    let ticket = crate::helpdesk::open_ticket(
        &db,
        scope.tenant_id,
        crate::helpdesk::OpenTicket {
            property_id: proc.property_id,
            unit_id: proc.unit_id,
            lease_id: proc.lease_id,
            title: clean(b.title).unwrap_or_else(|| format!("{}: {}", step.title, unit_label)),
            description: clean(b.description).or_else(|| step.description.clone()),
            category,
            priority,
            reporter: Some("Turnover".into()),
            due_date: step.due_on.clone(),
        },
        Some(user.user_id),
    )
    .await?;
    let before = step.clone();
    let mut am: entity::process_step::ActiveModel = step.into();
    am.ticket_id = Set(Some(ticket.id));
    if before.status == engine::READY {
        am.status = Set(engine::DOING.into());
        am.started_at = Set(Some(Utc::now().into()));
    }
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&db).await?;
    change::change(
        &db,
        Ctx::new(&user, &scope),
        act::PROCESS_STEP_UPDATE,
        "process_step",
        saved.id,
        Some(proc.property_id),
        &format!("{} — {}", proc.title, saved.title),
        &before,
        &saved,
    )
    .await;
    let mut v = build_dtos(&db, scope.tenant_id, vec![proc], true).await?;
    Ok(Json(v.remove(0)))
}

/// `POST /processes/<id>/finish` — close the turn. Every required step must
/// be done or skipped, unless `override_reason` says why not (audited).
#[rocket_okapi::openapi(tag = "Turnover")]
#[post("/processes/<id>/finish", data = "<body>")]
pub async fn finish_process(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<FinishReq>,
) -> ApiResult<Json<ProcessDto>> {
    user.require(Permission::MaintenanceManage)?;
    let proc = find_process(&db, scope.tenant_id, pid(id)?).await?;
    if proc.status != "active" {
        return Err(ApiError::Conflict("this turn is no longer active".into()));
    }
    let rows = engine::steps_of(&db, scope.tenant_id, proc.id).await?;
    let unmet = engine::unmet_required(&states(&rows));
    let reason = clean(body.into_inner().override_reason);
    if !unmet.is_empty() && reason.is_none() {
        return Err(ApiError::Conflict(format!(
            "required steps still open: {}",
            unmet.join(", ")
        )));
    }
    let before = proc.clone();
    let mut am: entity::process::ActiveModel = proc.into();
    am.status = Set("done".into());
    am.finished_on = Set(Some(engine::today()));
    am.override_reason = Set(if unmet.is_empty() { None } else { reason });
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&db).await?;
    change::change(
        &db,
        Ctx::new(&user, &scope),
        act::PROCESS_FINISH,
        "process",
        saved.id,
        Some(saved.property_id),
        &saved.title,
        &before,
        &saved,
    )
    .await;
    let mut v = build_dtos(&db, scope.tenant_id, vec![saved], true).await?;
    Ok(Json(v.remove(0)))
}

/// `POST /processes/<id>/cancel` — abandon a turn that was started by mistake.
#[rocket_okapi::openapi(tag = "Turnover")]
#[post("/processes/<id>/cancel")]
pub async fn cancel_process(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<ProcessDto>> {
    user.require(Permission::MaintenanceManage)?;
    let proc = find_process(&db, scope.tenant_id, pid(id)?).await?;
    if proc.status != "active" {
        return Err(ApiError::Conflict("this turn is no longer active".into()));
    }
    let before = proc.clone();
    let mut am: entity::process::ActiveModel = proc.into();
    am.status = Set("cancelled".into());
    am.finished_on = Set(Some(engine::today()));
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&db).await?;
    change::change(
        &db,
        Ctx::new(&user, &scope),
        act::PROCESS_CANCEL,
        "process",
        saved.id,
        Some(saved.property_id),
        &saved.title,
        &before,
        &saved,
    )
    .await;
    let mut v = build_dtos(&db, scope.tenant_id, vec![saved], true).await?;
    Ok(Json(v.remove(0)))
}
