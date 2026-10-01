//! The team roster and employee (HR) profiles.

use super::{parse_id, sees_pay};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::tenancy::TenantScope;
use crate::workforce::{self, entry_minutes, is_current, overtime, Rules, EMPLOYMENT_TYPES};
use chrono::{NaiveDate, Utc};
use entity::prelude::{EmployeeProfile, Membership, TimeEntry, User};
use rocket::serde::json::Json;
use rocket::{get, put};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Personas that are staff (can be on the team); residents and owners aren't.
const NON_STAFF: &[&str] = &["renter", "landlord", "owner", "investor", "vendor"];

#[derive(Serialize, schemars::JsonSchema)]
pub struct EmployeeDto {
    pub user_id: Uuid,
    pub name: String,
    pub email: String,
    /// The persona(s) they hold in the workspace.
    pub personas: Vec<String>,
    /// Their HR record; `None` = a staff member not set up on the team yet.
    pub profile: Option<ProfileDto>,
    pub clocked_in: bool,
    pub week_minutes: i64,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ProfileDto {
    pub title: Option<String>,
    /// `full_time` | `part_time` | `seasonal` | `contractor` (1099)
    pub employment_type: String,
    /// Only with `payroll:read`.
    pub pay_rate_cents: Option<i64>,
    /// What an hour of their work bills the owner. Only with `payroll:read`.
    pub bill_rate_cents: Option<i64>,
    pub hire_date: Option<String>,
    pub end_date: Option<String>,
    pub current: bool,
    pub weekly_hours_target: i32,
    /// `company` | `personal`
    pub default_vehicle: String,
    pub mileage_reimbursed: bool,
    pub overtime_eligible: bool,
    pub emergency_contact_name: Option<String>,
    pub emergency_contact_phone: Option<String>,
    pub calendar_color: String,
    pub notes: Option<String>,
}

fn profile_dto(
    p: &entity::employee_profile::Model,
    show_pay: bool,
    today: NaiveDate,
) -> ProfileDto {
    ProfileDto {
        title: p.title.clone(),
        employment_type: p.employment_type.clone(),
        pay_rate_cents: show_pay.then_some(p.pay_rate_cents),
        bill_rate_cents: show_pay.then_some(p.bill_rate_cents),
        hire_date: p.hire_date.clone(),
        end_date: p.end_date.clone(),
        current: is_current(p, today),
        weekly_hours_target: p.weekly_hours_target,
        default_vehicle: p.default_vehicle.clone(),
        mileage_reimbursed: p.mileage_reimbursed,
        overtime_eligible: p.employment_type != "contractor",
        emergency_contact_name: p.emergency_contact_name.clone(),
        emergency_contact_phone: p.emergency_contact_phone.clone(),
        calendar_color: p.calendar_color.clone(),
        notes: p.notes.clone(),
    }
}

/// `GET /team` — everyone on staff, with their profile, whether they're on
/// the clock, and their hours this week.
#[rocket_okapi::openapi(tag = "Team")]
#[get("/team")]
pub async fn roster(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<EmployeeDto>>> {
    user.require(Permission::TeamRead)?;
    let rules = Rules::load(&db, scope.tenant_id).await;
    let now = Utc::now();
    let today = rules.local_date(now.into());
    let profiles = workforce::profiles_by_user(&db, scope.tenant_id).await?;
    let mut personas: HashMap<Uuid, Vec<String>> = HashMap::new();
    for m in Membership::find()
        .filter(entity::membership::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::membership::Column::Status.eq("active"))
        .all(&db)
        .await?
    {
        if !NON_STAFF.contains(&m.profile_type.as_str()) || profiles.contains_key(&m.user_id) {
            personas.entry(m.user_id).or_default().push(m.profile_type);
        }
    }
    for uid in profiles.keys() {
        personas.entry(*uid).or_default();
    }
    let ids: Vec<Uuid> = personas.keys().copied().collect();
    let users = User::find()
        .filter(entity::user::Column::Id.is_in(ids.clone()))
        .all(&db)
        .await?;
    let monday = overtime::week_start(today);
    let week = TimeEntry::find()
        .filter(entity::time_entry::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::time_entry::Column::StartedAt.gte(rules.day_start(monday)))
        .all(&db)
        .await?;
    let show_pay = sees_pay(&user);
    let mut out: Vec<EmployeeDto> = users
        .into_iter()
        .map(|u| EmployeeDto {
            user_id: u.id,
            profile: profiles.get(&u.id).map(|p| profile_dto(p, show_pay, today)),
            clocked_in: week
                .iter()
                .any(|e| e.user_id == u.id && e.ended_at.is_none()),
            week_minutes: week
                .iter()
                .filter(|e| e.user_id == u.id)
                .map(|e| entry_minutes(e, now))
                .sum(),
            personas: personas.remove(&u.id).unwrap_or_default(),
            name: u.name,
            email: u.email,
        })
        .collect();
    out.sort_by(|a, b| {
        b.profile
            .is_some()
            .cmp(&a.profile.is_some())
            .then(a.name.cmp(&b.name))
    });
    Ok(Json(out))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ProfileReq {
    pub title: Option<String>,
    pub employment_type: Option<String>,
    /// Changing pay needs `payroll:read`.
    pub pay_rate_cents: Option<i64>,
    pub bill_rate_cents: Option<i64>,
    pub hire_date: Option<String>,
    pub end_date: Option<String>,
    pub weekly_hours_target: Option<i32>,
    pub default_vehicle: Option<String>,
    pub mileage_reimbursed: Option<bool>,
    pub emergency_contact_name: Option<String>,
    pub emergency_contact_phone: Option<String>,
    pub calendar_color: Option<String>,
    pub notes: Option<String>,
}

fn clean(s: Option<String>) -> Option<String> {
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

fn check_date(s: &Option<String>, what: &str) -> ApiResult<()> {
    if let Some(d) = s {
        d.parse::<NaiveDate>()
            .map_err(|_| ApiError::BadRequest(format!("{what} must be a date like 2026-09-30")))?;
    }
    Ok(())
}

/// `PUT /team/<user_id>` — set up or update someone's employee profile.
/// Fields left out keep their value.
#[rocket_okapi::openapi(tag = "Team")]
#[put("/team/<user_id>", data = "<body>")]
pub async fn upsert(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    user_id: &str,
    body: Json<ProfileReq>,
) -> ApiResult<Json<ProfileDto>> {
    user.require(Permission::TeamManage)?;
    let uid = parse_id(user_id, "user")?;
    let b = body.into_inner();
    if (b.pay_rate_cents.is_some() || b.bill_rate_cents.is_some()) && !sees_pay(&user) {
        return Err(ApiError::Forbidden(
            "changing pay or bill rates needs payroll:read".into(),
        ));
    }
    let member = Membership::find()
        .filter(entity::membership::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::membership::Column::UserId.eq(uid))
        .one(&db)
        .await?;
    if member.is_none() {
        return Err(ApiError::NotFound(
            "that person isn't in this workspace".into(),
        ));
    }
    if let Some(t) = &b.employment_type {
        if !EMPLOYMENT_TYPES.contains(&t.as_str()) {
            return Err(ApiError::BadRequest(format!(
                "employment_type must be one of {}",
                EMPLOYMENT_TYPES.join(", ")
            )));
        }
    }
    if let Some(v) = &b.default_vehicle {
        if v != "company" && v != "personal" {
            return Err(ApiError::BadRequest(
                "default_vehicle must be company or personal".into(),
            ));
        }
    }
    if b.pay_rate_cents.is_some_and(|r| r < 0) || b.bill_rate_cents.is_some_and(|r| r < 0) {
        return Err(ApiError::BadRequest("rates can't be negative".into()));
    }
    if b.weekly_hours_target
        .is_some_and(|h| !(0..=80).contains(&h))
    {
        return Err(ApiError::BadRequest("weekly hours must be 0–80".into()));
    }
    check_date(&b.hire_date, "hire_date")?;
    check_date(&b.end_date, "end_date")?;
    if let Some(c) = &b.calendar_color {
        let ok =
            c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|x| x.is_ascii_hexdigit());
        if !ok {
            return Err(ApiError::BadRequest(
                "calendar_color must be like #0e7c86".into(),
            ));
        }
    }

    let now = Utc::now();
    let existing = EmployeeProfile::find()
        .filter(entity::employee_profile::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::employee_profile::Column::UserId.eq(uid))
        .one(&db)
        .await?;
    let saved = match existing {
        Some(p) => {
            let mut am: entity::employee_profile::ActiveModel = p.into();
            if b.title.is_some() {
                am.title = Set(clean(b.title));
            }
            if let Some(v) = b.employment_type {
                am.employment_type = Set(v);
            }
            if let Some(v) = b.pay_rate_cents {
                am.pay_rate_cents = Set(v);
            }
            if let Some(v) = b.bill_rate_cents {
                am.bill_rate_cents = Set(v);
            }
            if b.hire_date.is_some() {
                am.hire_date = Set(clean(b.hire_date));
            }
            if b.end_date.is_some() {
                am.end_date = Set(clean(b.end_date));
            }
            if let Some(v) = b.weekly_hours_target {
                am.weekly_hours_target = Set(v);
            }
            if let Some(v) = b.default_vehicle {
                am.default_vehicle = Set(v);
            }
            if let Some(v) = b.mileage_reimbursed {
                am.mileage_reimbursed = Set(v);
            }
            if b.emergency_contact_name.is_some() {
                am.emergency_contact_name = Set(clean(b.emergency_contact_name));
            }
            if b.emergency_contact_phone.is_some() {
                am.emergency_contact_phone = Set(clean(b.emergency_contact_phone));
            }
            if let Some(v) = b.calendar_color {
                am.calendar_color = Set(v);
            }
            if b.notes.is_some() {
                am.notes = Set(clean(b.notes));
            }
            am.updated_at = Set(now.into());
            am.update(&db).await?
        }
        None => {
            let contractor = b.employment_type.as_deref() == Some("contractor");
            entity::employee_profile::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(scope.tenant_id),
                user_id: Set(uid),
                title: Set(clean(b.title)),
                employment_type: Set(b.employment_type.unwrap_or_else(|| "full_time".into())),
                pay_rate_cents: Set(b.pay_rate_cents.unwrap_or(0)),
                bill_rate_cents: Set(b.bill_rate_cents.unwrap_or(0)),
                hire_date: Set(clean(b.hire_date)),
                end_date: Set(clean(b.end_date)),
                weekly_hours_target: Set(b.weekly_hours_target.unwrap_or(40)),
                default_vehicle: Set(b.default_vehicle.unwrap_or_else(|| "company".into())),
                // Contractors usually claim their own mileage.
                mileage_reimbursed: Set(b.mileage_reimbursed.unwrap_or(!contractor)),
                emergency_contact_name: Set(clean(b.emergency_contact_name)),
                emergency_contact_phone: Set(clean(b.emergency_contact_phone)),
                calendar_color: Set(b.calendar_color.unwrap_or_else(|| "#0e7c86".into())),
                notes: Set(clean(b.notes)),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }
            .insert(&db)
            .await?
        }
    };
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::EMPLOYEE_UPSERT,
        Some("user"),
        Some(uid.to_string()),
        Some(scope.tenant_id),
        // Field names only — never the pay values.
        Some(serde_json::json!({ "employment_type": saved.employment_type })),
    )
    .await;
    let rules = Rules::load(&db, scope.tenant_id).await;
    Ok(Json(profile_dto(
        &saved,
        sees_pay(&user),
        rules.local_date(now.into()),
    )))
}
