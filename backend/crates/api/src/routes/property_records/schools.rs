//! The schools a property is zoned for, each with its own profile. Rows come
//! from the schools data source or the team; once the team edits one it is
//! theirs and a data refresh leaves it alone.

use super::{date, parse_id, property_in, text};
use crate::auth::AuthUser;
use crate::enrichment::runner::MANUAL_SCHOOL;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::PropertySchool;
use rocket::serde::json::Json;
use rocket::{delete, get, post, put, State};
use schemars::JsonSchema;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const LEVELS: &[&str] = &[
    "preschool",
    "elementary",
    "middle",
    "high",
    "k8",
    "k12",
    "other",
];

#[derive(Deserialize, JsonSchema)]
pub struct SchoolReq {
    pub name: String,
    pub level: String,
    pub district: Option<String>,
    pub grades: Option<String>,
    /// 1–10.
    pub rating: Option<i32>,
    pub distance_mi: Option<f64>,
    /// The property is in this school's attendance zone.
    #[serde(default)]
    pub assigned: bool,
    pub zone_name: Option<String>,
    pub zone_verified_on: Option<String>,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub website: Option<String>,
    pub enrollment: Option<i32>,
    pub notes: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct SchoolRecordDto {
    pub id: Uuid,
    pub property_id: Uuid,
    pub name: String,
    pub level: String,
    pub district: Option<String>,
    pub grades: Option<String>,
    pub rating: Option<i32>,
    pub distance_mi: Option<f64>,
    pub assigned: bool,
    pub zone_name: Option<String>,
    pub zone_verified_on: Option<String>,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub website: Option<String>,
    pub enrollment: Option<i32>,
    pub notes: Option<String>,
    /// `manual` when the team added or edited it; otherwise the data source.
    pub source: String,
    /// The data source's values are estimates until someone checks them.
    pub simulated: bool,
}

impl From<entity::property_school::Model> for SchoolRecordDto {
    fn from(s: entity::property_school::Model) -> Self {
        SchoolRecordDto {
            simulated: s.source.starts_with("simulated"),
            id: s.id,
            property_id: s.property_id,
            name: s.name,
            level: s.level,
            district: s.district,
            grades: s.grades,
            rating: s.rating,
            distance_mi: s.distance_mi,
            assigned: s.assigned,
            zone_name: s.zone_name,
            zone_verified_on: s.zone_verified_on,
            address: s.address,
            phone: s.phone,
            website: s.website,
            enrollment: s.enrollment,
            notes: s.notes,
            source: s.source,
        }
    }
}

/// Elementary, then middle, then high; zoned schools before others nearby.
pub fn level_rank(level: &str) -> u8 {
    match level {
        "preschool" => 0,
        "elementary" => 1,
        "k8" => 2,
        "middle" => 3,
        "high" => 4,
        "k12" => 5,
        _ => 6,
    }
}

/// `GET /properties/<id>/schools`
#[rocket_okapi::openapi(tag = "Property Profile")]
#[get("/properties/<id>/schools")]
pub async fn list_schools(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Vec<SchoolRecordDto>>> {
    user.require(Permission::PropertyRead)?;
    let property = property_in(&db, scope.tenant_id, id).await?;
    let mut rows = PropertySchool::find()
        .filter(entity::property_school::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::property_school::Column::PropertyId.eq(property.id))
        .all(&db)
        .await?;
    rows.sort_by(|a, b| {
        (level_rank(&a.level), !a.assigned, &a.name).cmp(&(
            level_rank(&b.level),
            !b.assigned,
            &b.name,
        ))
    });
    Ok(Json(rows.into_iter().map(SchoolRecordDto::from).collect()))
}

fn fill(am: &mut entity::property_school::ActiveModel, b: SchoolReq) -> ApiResult<()> {
    let name = text(Some(b.name)).ok_or_else(|| ApiError::BadRequest("name the school".into()))?;
    let level = b.level.trim().to_lowercase();
    if !LEVELS.contains(&level.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "level must be one of: {}",
            LEVELS.join(", ")
        )));
    }
    if let Some(r) = b.rating {
        if !(1..=10).contains(&r) {
            return Err(ApiError::BadRequest("rating is 1 to 10".into()));
        }
    }
    if matches!(b.distance_mi, Some(d) if !(0.0..=500.0).contains(&d)) {
        return Err(ApiError::BadRequest("distance doesn't look right".into()));
    }
    let website = text(b.website);
    if let Some(w) = &website {
        if !(w.starts_with("https://") || w.starts_with("http://")) || w.contains(' ') {
            return Err(ApiError::BadRequest(
                "the website must be a web address (https://…)".into(),
            ));
        }
    }
    am.name = Set(name);
    am.level = Set(level);
    am.district = Set(text(b.district));
    am.grades = Set(text(b.grades));
    am.rating = Set(b.rating);
    am.distance_mi = Set(b.distance_mi);
    am.assigned = Set(b.assigned);
    am.zone_name = Set(text(b.zone_name));
    am.zone_verified_on = Set(date("zone check date", b.zone_verified_on)?);
    am.address = Set(text(b.address));
    am.phone = Set(text(b.phone));
    am.website = Set(website);
    am.enrollment = Set(b.enrollment.filter(|e| *e >= 0));
    am.notes = Set(text(b.notes));
    am.source = Set(MANUAL_SCHOOL.into());
    am.updated_at = Set(Some(Utc::now().into()));
    Ok(())
}

/// `POST /properties/<id>/schools` — add a school.
#[rocket_okapi::openapi(tag = "Property Profile")]
#[post("/properties/<id>/schools", data = "<body>")]
pub async fn create_school(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<SchoolReq>,
) -> ApiResult<Json<SchoolRecordDto>> {
    user.require(Permission::PropertyWrite)?;
    let property = property_in(&db, scope.tenant_id, id).await?;
    let mut am = entity::property_school::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        property_id: Set(property.id),
        created_at: Set(Utc::now().into()),
        ..Default::default()
    };
    fill(&mut am, body.into_inner())?;
    Ok(Json(SchoolRecordDto::from(am.insert(&db).await?)))
}

async fn school_of(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    property_id: Uuid,
    school_id: &str,
) -> ApiResult<entity::property_school::Model> {
    PropertySchool::find_by_id(parse_id(school_id)?)
        .filter(entity::property_school::Column::TenantId.eq(tenant_id))
        .filter(entity::property_school::Column::PropertyId.eq(property_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("school not found".into()))
}

/// `PUT /properties/<id>/schools/<school_id>` — edit a school's profile or
/// zone. The row becomes the team's.
#[rocket_okapi::openapi(tag = "Property Profile")]
#[put("/properties/<id>/schools/<school_id>", data = "<body>")]
pub async fn update_school(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    school_id: &str,
    body: Json<SchoolReq>,
) -> ApiResult<Json<SchoolRecordDto>> {
    user.require(Permission::PropertyWrite)?;
    let property = property_in(&db, scope.tenant_id, id).await?;
    let row = school_of(&db, scope.tenant_id, property.id, school_id).await?;
    let mut am: entity::property_school::ActiveModel = row.into();
    fill(&mut am, body.into_inner())?;
    Ok(Json(SchoolRecordDto::from(am.update(&db).await?)))
}

/// `DELETE /properties/<id>/schools/<school_id>`
#[rocket_okapi::openapi(tag = "Property Profile")]
#[delete("/properties/<id>/schools/<school_id>")]
pub async fn delete_school(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    school_id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::PropertyWrite)?;
    let property = property_in(&db, scope.tenant_id, id).await?;
    let row = school_of(&db, scope.tenant_id, property.id, school_id).await?;
    PropertySchool::delete_by_id(row.id).exec(&db).await?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}
