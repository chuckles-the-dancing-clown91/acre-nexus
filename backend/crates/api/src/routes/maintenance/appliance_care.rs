//! **Care for an appliance** — the library's suggestion for it (how to look
//! after it and the routine jobs), and applying that suggestion: the care
//! instructions are filled in and the chosen jobs go on the maintenance
//! schedule against this appliance.

use crate::appliance_care::{self, Care};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::tenancy::TenantScope;
use chrono::{Duration, Utc};
use entity::prelude::{Asset, MaintenancePlan};
use rocket::serde::json::Json;
use rocket::{get, post};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct CareJob {
    pub title: String,
    pub description: String,
    pub cadence_days: i32,
    pub priority: String,
    /// Already on this appliance's schedule.
    pub scheduled: bool,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct CareSuggestion {
    /// The library entry used; `None` when nothing matched.
    pub key: Option<String>,
    pub label: Option<String>,
    pub life_years: Option<i32>,
    pub instructions: Option<String>,
    pub jobs: Vec<CareJob>,
    /// Every entry, for choosing one by hand.
    pub library: Vec<appliance_care::CareEntry>,
}

async fn find_asset(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    id: &str,
) -> ApiResult<entity::asset::Model> {
    let aid = Uuid::parse_str(id).map_err(|_| ApiError::NotFound("asset not found".into()))?;
    Asset::find_by_id(aid)
        .filter(entity::asset::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("asset not found".into()))
}

fn plan_title(asset: &entity::asset::Model, job: &str) -> String {
    format!("{}: {}", asset.name, job)
}

fn pick(asset: &entity::asset::Model, key: Option<&str>) -> Option<&'static Care> {
    match key.filter(|k| !k.is_empty()) {
        Some(k) => appliance_care::by_key(k),
        None => appliance_care::matching(&asset.name, &asset.kind),
    }
}

/// `GET /assets/<id>/care?key=` — the care library's suggestion for this
/// appliance. `key` picks an entry by hand when the match is wrong.
#[rocket_okapi::openapi(tag = "Maintenance")]
#[get("/assets/<id>/care?<key>")]
pub async fn care_suggestion(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    key: Option<String>,
) -> ApiResult<Json<CareSuggestion>> {
    user.require(Permission::MaintenanceRead)?;
    let asset = find_asset(&db, scope.tenant_id, id).await?;
    let on_schedule: Vec<String> = MaintenancePlan::find()
        .filter(entity::maintenance_plan::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::maintenance_plan::Column::AssetId.eq(asset.id))
        .all(&db)
        .await?
        .into_iter()
        .map(|p| p.title)
        .collect();
    let care = pick(&asset, key.as_deref());
    Ok(Json(CareSuggestion {
        key: care.map(|c| c.key.into()),
        label: care.map(|c| c.label.into()),
        life_years: care.map(|c| c.life_years),
        instructions: care.map(|c| c.instructions.into()),
        jobs: care
            .map(|c| {
                c.tasks
                    .iter()
                    .map(|t| CareJob {
                        scheduled: on_schedule.contains(&plan_title(&asset, t.title)),
                        title: t.title.into(),
                        description: t.description.into(),
                        cadence_days: t.cadence_days,
                        priority: t.priority.into(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        library: appliance_care::entries(),
    }))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ApplyCare {
    /// Library entry; omit to use the match for this appliance.
    #[serde(default)]
    pub key: Option<String>,
    /// Write the care instructions, replacing any that are there.
    #[serde(default)]
    pub instructions: bool,
    /// Titles of the jobs to put on the schedule.
    #[serde(default)]
    pub jobs: Vec<String>,
    /// Also fill expected life when it is blank.
    #[serde(default)]
    pub life: bool,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct CareApplied {
    pub instructions_set: bool,
    pub plans_created: usize,
}

/// `POST /assets/<id>/care/apply` — take the suggestion: care instructions
/// and the chosen jobs on the schedule. A job already scheduled is skipped.
#[rocket_okapi::openapi(tag = "Maintenance")]
#[post("/assets/<id>/care/apply", data = "<body>")]
pub async fn apply_care(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<ApplyCare>,
) -> ApiResult<Json<CareApplied>> {
    user.require(Permission::MaintenanceManage)?;
    let b = body.into_inner();
    let asset = find_asset(&db, scope.tenant_id, id).await?;
    let care = pick(&asset, b.key.as_deref())
        .ok_or_else(|| ApiError::BadRequest("no care guide matches this appliance".into()))?;
    for j in &b.jobs {
        if !care.tasks.iter().any(|t| t.title == j) {
            return Err(ApiError::BadRequest(format!("unknown job: {j}")));
        }
    }

    let mut set_text = false;
    if b.instructions || (b.life && asset.expected_life_years.is_none()) {
        let mut am: entity::asset::ActiveModel = asset.clone().into();
        if b.instructions {
            am.care_instructions = Set(Some(care.instructions.to_string()));
            set_text = true;
        }
        if b.life && asset.expected_life_years.is_none() {
            am.expected_life_years = Set(Some(care.life_years));
        }
        am.updated_at = Set(Utc::now().into());
        am.update(&db).await?;
    }

    let existing: Vec<String> = MaintenancePlan::find()
        .filter(entity::maintenance_plan::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::maintenance_plan::Column::AssetId.eq(asset.id))
        .all(&db)
        .await?
        .into_iter()
        .map(|p| p.title)
        .collect();
    let category = match asset.kind.as_str() {
        "other" => "general".to_string(),
        k => k.to_string(),
    };
    let today = Utc::now().date_naive();
    let now = Utc::now();
    let mut created = 0;
    for t in care
        .tasks
        .iter()
        .filter(|t| b.jobs.iter().any(|j| j == t.title))
    {
        let title = plan_title(&asset, t.title);
        if existing.contains(&title) {
            continue;
        }
        let saved = entity::maintenance_plan::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(scope.tenant_id),
            property_id: Set(asset.property_id),
            unit_id: Set(asset.unit_id),
            asset_id: Set(Some(asset.id)),
            title: Set(title),
            description: Set(Some(t.description.to_string())),
            category: Set(category.clone()),
            priority: Set(t.priority.to_string()),
            cadence_days: Set(t.cadence_days),
            next_due_date: Set((today + Duration::days(t.cadence_days as i64))
                .format("%Y-%m-%d")
                .to_string()),
            active: Set(true),
            last_ticket_id: Set(None),
            issue_template_id: Set(None),
            mandate_key: Set(None),
            lead_days: Set(None),
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
                "asset_id": asset.id,
                "cadence_days": saved.cadence_days,
                "from": "care_library",
            })),
        )
        .await;
        created += 1;
    }
    Ok(Json(CareApplied {
        instructions_set: set_text,
        plans_created: created,
    }))
}
