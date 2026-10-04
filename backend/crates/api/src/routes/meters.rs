//! **Meters and utilities** for properties and units: add a meter, log
//! readings (move-in and move-out readings go on the lease), and see who pays
//! for each utility (`GET /properties/<id>/utilities`), which is what the
//! lease's utility agreement says.

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::tenancy::{Access, TenantScope};
use crate::utilities::{self, UtilityTerm};
use chrono::{NaiveDate, Utc};
use entity::prelude::{Meter, MeterReading, Property, Unit};
use rocket::serde::json::Json;
use rocket::{get, patch, post};
use schemars::JsonSchema;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

const PAID_BY: &[&str] = &["tenant", "landlord", "shared"];
const REASONS: &[&str] = &["routine", "move_in", "move_out", "other"];

fn uuid(s: &str, what: &str) -> ApiResult<Uuid> {
    Uuid::parse_str(s).map_err(|_| ApiError::NotFound(format!("{what} not found")))
}

fn text(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Property managers and maintenance crews both look after meters.
fn may_manage(user: &AuthUser) -> ApiResult<()> {
    if user.grants.has_key("property:write") || user.grants.has_key("maintenance:manage") {
        Ok(())
    } else {
        Err(ApiError::Forbidden(
            "missing permission: property:write or maintenance:manage".into(),
        ))
    }
}

#[derive(Serialize, JsonSchema)]
pub struct MeterDto {
    pub id: Uuid,
    pub property_id: Uuid,
    pub unit_id: Option<Uuid>,
    pub kind: String,
    pub label: String,
    pub meter_number: Option<String>,
    pub location: Option<String>,
    pub provider: Option<String>,
    pub unit_of_measure: Option<String>,
    pub paid_by: String,
    pub billing_note: Option<String>,
    pub status: String,
    pub last_reading: Option<f64>,
    pub last_read_on: Option<String>,
}

fn dto(m: entity::meter::Model, last: Option<&entity::meter_reading::Model>) -> MeterDto {
    MeterDto {
        id: m.id,
        property_id: m.property_id,
        unit_id: m.unit_id,
        kind: m.kind,
        label: m.label,
        meter_number: m.meter_number,
        location: m.location,
        provider: m.provider,
        unit_of_measure: m.unit_of_measure,
        paid_by: m.paid_by,
        billing_note: m.billing_note,
        status: m.status,
        last_reading: last.map(|r| r.reading),
        last_read_on: last.map(|r| r.read_on.to_string()),
    }
}

async fn latest_readings(
    db: &crate::db::RequestDb,
    ids: Vec<Uuid>,
) -> ApiResult<HashMap<Uuid, entity::meter_reading::Model>> {
    let mut out: HashMap<Uuid, entity::meter_reading::Model> = HashMap::new();
    if ids.is_empty() {
        return Ok(out);
    }
    for r in MeterReading::find()
        .filter(entity::meter_reading::Column::MeterId.is_in(ids))
        .order_by_asc(entity::meter_reading::Column::ReadOn)
        .order_by_asc(entity::meter_reading::Column::CreatedAt)
        .all(db)
        .await?
    {
        out.insert(r.meter_id, r);
    }
    Ok(out)
}

/// `GET /meters?property_id&unit_id` — a property's meters, or one unit's.
#[rocket_okapi::openapi(tag = "Meters")]
#[get("/meters?<property_id>&<unit_id>")]
pub async fn list(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    property_id: Option<&str>,
    unit_id: Option<&str>,
) -> ApiResult<Json<Vec<MeterDto>>> {
    user.require(Permission::PropertyRead)?;
    let mut q = Meter::find().filter(entity::meter::Column::TenantId.eq(scope.tenant_id));
    if let Some(p) = property_id.filter(|s| !s.is_empty()) {
        let pid = uuid(p, "property")?;
        if !access.sees(pid) {
            return Err(ApiError::NotFound("property not found".into()));
        }
        q = q.filter(entity::meter::Column::PropertyId.eq(pid));
    } else if let Some(ids) = access.property_ids() {
        q = q.filter(entity::meter::Column::PropertyId.is_in(ids));
    }
    if let Some(u) = unit_id.filter(|s| !s.is_empty()) {
        q = q.filter(entity::meter::Column::UnitId.eq(uuid(u, "unit")?));
    }
    let rows = q
        .order_by_asc(entity::meter::Column::Kind)
        .order_by_asc(entity::meter::Column::Label)
        .all(&db)
        .await?;
    let last = latest_readings(&db, rows.iter().map(|m| m.id).collect()).await?;
    Ok(Json(
        rows.into_iter()
            .map(|m| {
                let l = last.get(&m.id);
                dto(m, l)
            })
            .collect(),
    ))
}

#[derive(Deserialize, JsonSchema)]
pub struct CreateMeterReq {
    pub property_id: Uuid,
    /// Leave out for a house meter that serves the whole property.
    pub unit_id: Option<Uuid>,
    pub kind: String,
    pub label: Option<String>,
    pub meter_number: Option<String>,
    pub location: Option<String>,
    pub provider: Option<String>,
    pub unit_of_measure: Option<String>,
    pub paid_by: Option<String>,
    pub billing_note: Option<String>,
}

/// `POST /meters` — add a meter to a property or one of its units.
#[rocket_okapi::openapi(tag = "Meters")]
#[post("/meters", data = "<body>")]
pub async fn create(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    body: Json<CreateMeterReq>,
) -> ApiResult<Json<MeterDto>> {
    may_manage(&user)?;
    let b = body.into_inner();
    if !utilities::KINDS.contains(&b.kind.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "kind must be one of: {}",
            utilities::KINDS.join(", ")
        )));
    }
    let paid_by = b.paid_by.unwrap_or_else(|| "tenant".into());
    if !PAID_BY.contains(&paid_by.as_str()) {
        return Err(ApiError::BadRequest(
            "paid_by must be tenant, landlord or shared".into(),
        ));
    }
    if !access.sees(b.property_id) {
        return Err(ApiError::NotFound("property not found".into()));
    }
    Property::find_by_id(b.property_id)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    let unit = match b.unit_id {
        Some(uid) => Some(
            Unit::find_by_id(uid)
                .filter(entity::unit::Column::TenantId.eq(scope.tenant_id))
                .filter(entity::unit::Column::PropertyId.eq(b.property_id))
                .one(&db)
                .await?
                .ok_or_else(|| ApiError::NotFound("unit not found on this property".into()))?,
        ),
        None => None,
    };
    let label = text(b.label).unwrap_or_else(|| {
        let k = utilities::kind_label(&b.kind);
        match &unit {
            Some(u) => format!("{} {k}", u.unit_number),
            None => k.to_string(),
        }
    });
    let now = Utc::now();
    let saved = entity::meter::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        property_id: Set(b.property_id),
        unit_id: Set(b.unit_id),
        kind: Set(b.kind),
        label: Set(label),
        meter_number: Set(text(b.meter_number)),
        location: Set(text(b.location)),
        provider: Set(text(b.provider)),
        unit_of_measure: Set(text(b.unit_of_measure)),
        paid_by: Set(paid_by),
        billing_note: Set(text(b.billing_note)),
        status: Set("active".into()),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;
    crate::audit::change::created(
        &db,
        crate::audit::change::Ctx::new(&user, &scope),
        crate::audit::actions::METER_CREATE,
        "meter",
        saved.id,
        Some(saved.property_id),
        &saved.label,
    )
    .await;
    Ok(Json(dto(saved, None)))
}

#[derive(Deserialize, JsonSchema)]
pub struct UpdateMeterReq {
    pub label: Option<String>,
    pub meter_number: Option<String>,
    pub location: Option<String>,
    pub provider: Option<String>,
    pub unit_of_measure: Option<String>,
    pub paid_by: Option<String>,
    pub billing_note: Option<String>,
    /// `active` | `retired`.
    pub status: Option<String>,
}

/// `PATCH /meters/<id>` — change a meter, or retire it when it's replaced.
#[rocket_okapi::openapi(tag = "Meters")]
#[patch("/meters/<id>", data = "<body>")]
pub async fn update(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    id: &str,
    body: Json<UpdateMeterReq>,
) -> ApiResult<Json<MeterDto>> {
    may_manage(&user)?;
    let m = find(&db, scope.tenant_id, &access, id).await?;
    let b = body.into_inner();
    let before = m.clone();
    let mut am: entity::meter::ActiveModel = m.into();
    if let Some(v) = text(b.label) {
        am.label = Set(v);
    }
    if let Some(v) = b.meter_number {
        am.meter_number = Set(text(Some(v)));
    }
    if let Some(v) = b.location {
        am.location = Set(text(Some(v)));
    }
    if let Some(v) = b.provider {
        am.provider = Set(text(Some(v)));
    }
    if let Some(v) = b.unit_of_measure {
        am.unit_of_measure = Set(text(Some(v)));
    }
    if let Some(v) = b.billing_note {
        am.billing_note = Set(text(Some(v)));
    }
    if let Some(v) = b.paid_by {
        if !PAID_BY.contains(&v.as_str()) {
            return Err(ApiError::BadRequest(
                "paid_by must be tenant, landlord or shared".into(),
            ));
        }
        am.paid_by = Set(v);
    }
    if let Some(v) = b.status {
        if !["active", "retired"].contains(&v.as_str()) {
            return Err(ApiError::BadRequest(
                "status must be active or retired".into(),
            ));
        }
        am.status = Set(v);
    }
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&db).await?;
    crate::audit::change::change(
        &db,
        crate::audit::change::Ctx::new(&user, &scope),
        crate::audit::actions::METER_UPDATE,
        "meter",
        saved.id,
        Some(saved.property_id),
        &saved.label,
        &before,
        &saved,
    )
    .await;
    let last = latest_readings(&db, vec![saved.id]).await?;
    let l = last.get(&saved.id);
    Ok(Json(dto(saved.clone(), l)))
}

async fn find(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    access: &Access,
    id: &str,
) -> ApiResult<entity::meter::Model> {
    let m = Meter::find_by_id(uuid(id, "meter")?)
        .filter(entity::meter::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("meter not found".into()))?;
    if !access.sees(m.property_id) {
        return Err(ApiError::NotFound("meter not found".into()));
    }
    Ok(m)
}

#[derive(Serialize, JsonSchema)]
pub struct ReadingDto {
    pub id: Uuid,
    pub read_on: String,
    pub reading: f64,
    /// Used since the reading before it.
    pub used: Option<f64>,
    pub reason: String,
    pub lease_id: Option<Uuid>,
    pub note: Option<String>,
    pub created_at: String,
}

/// `GET /meters/<id>/readings` — newest first, with what was used between each.
#[rocket_okapi::openapi(tag = "Meters")]
#[get("/meters/<id>/readings")]
pub async fn readings(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    id: &str,
) -> ApiResult<Json<Vec<ReadingDto>>> {
    user.require(Permission::PropertyRead)?;
    let m = find(&db, scope.tenant_id, &access, id).await?;
    let rows = MeterReading::find()
        .filter(entity::meter_reading::Column::MeterId.eq(m.id))
        .order_by_asc(entity::meter_reading::Column::ReadOn)
        .order_by_asc(entity::meter_reading::Column::CreatedAt)
        .all(&db)
        .await?;
    let mut prev: Option<f64> = None;
    let mut out: Vec<ReadingDto> = rows
        .into_iter()
        .map(|r| {
            let used = prev.map(|p| r.reading - p);
            prev = Some(r.reading);
            ReadingDto {
                id: r.id,
                read_on: r.read_on.to_string(),
                reading: r.reading,
                used,
                reason: r.reason,
                lease_id: r.lease_id,
                note: r.note,
                created_at: r.created_at.to_rfc3339(),
            }
        })
        .collect();
    out.reverse();
    Ok(Json(out))
}

#[derive(Deserialize, JsonSchema)]
pub struct ReadingReq {
    pub reading: f64,
    /// Defaults to today.
    pub read_on: Option<String>,
    pub reason: Option<String>,
    pub lease_id: Option<Uuid>,
    pub note: Option<String>,
}

/// `POST /meters/<id>/readings` — record a reading. A reading lower than the
/// last one is refused, since meters only count up; retire the meter and add a
/// new one when it was swapped.
#[rocket_okapi::openapi(tag = "Meters")]
#[post("/meters/<id>/readings", data = "<body>")]
pub async fn add_reading(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    id: &str,
    body: Json<ReadingReq>,
) -> ApiResult<Json<ReadingDto>> {
    may_manage(&user)?;
    let m = find(&db, scope.tenant_id, &access, id).await?;
    if m.status != "active" {
        return Err(ApiError::BadRequest("this meter is retired".into()));
    }
    let b = body.into_inner();
    if !b.reading.is_finite() || b.reading < 0.0 {
        return Err(ApiError::BadRequest("enter the number on the meter".into()));
    }
    let read_on = match text(b.read_on) {
        Some(s) => NaiveDate::parse_from_str(&s, "%Y-%m-%d")
            .map_err(|_| ApiError::BadRequest("read_on must be YYYY-MM-DD".into()))?,
        None => Utc::now().date_naive(),
    };
    let reason = b.reason.unwrap_or_else(|| "routine".into());
    if !REASONS.contains(&reason.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "reason must be one of: {}",
            REASONS.join(", ")
        )));
    }
    let last = latest_readings(&db, vec![m.id]).await?;
    if let Some(l) = last.get(&m.id) {
        if b.reading < l.reading {
            return Err(ApiError::BadRequest(format!(
                "that's lower than the last reading ({}). Retire this meter and add a new one if it was replaced.",
                l.reading
            )));
        }
    }
    let r = entity::meter_reading::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        meter_id: Set(m.id),
        read_on: Set(read_on),
        reading: Set(b.reading),
        reason: Set(reason),
        lease_id: Set(b.lease_id),
        note: Set(text(b.note)),
        read_by: Set(Some(user.user_id)),
        created_at: Set(Utc::now().into()),
    }
    .insert(&db)
    .await?;
    crate::audit::change::noted(
        &db,
        crate::audit::change::Ctx::new(&user, &scope),
        crate::audit::actions::METER_READING,
        "meter",
        m.id,
        Some(m.property_id),
        &m.label,
        &format!("Read {} on {}", r.reading, r.read_on),
    )
    .await;
    Ok(Json(ReadingDto {
        id: r.id,
        read_on: r.read_on.to_string(),
        reading: r.reading,
        used: last.get(&m.id).map(|l| r.reading - l.reading),
        reason: r.reason,
        lease_id: r.lease_id,
        note: r.note,
        created_at: r.created_at.to_rfc3339(),
    }))
}

/// `GET /properties/<id>/utilities?unit_id` — who pays for each utility at a
/// property or one of its units. The lease's utility agreement reads this.
#[rocket_okapi::openapi(tag = "Meters")]
#[get("/properties/<id>/utilities?<unit_id>")]
pub async fn property_utilities(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    id: &str,
    unit_id: Option<&str>,
) -> ApiResult<Json<Vec<UtilityTerm>>> {
    user.require(Permission::PropertyRead)?;
    let pid = uuid(id, "property")?;
    if !access.sees(pid) {
        return Err(ApiError::NotFound("property not found".into()));
    }
    let uid = match unit_id.filter(|s| !s.is_empty()) {
        Some(u) => Some(uuid(u, "unit")?),
        None => None,
    };
    Ok(Json(
        utilities::terms(&db, scope.tenant_id, pid, uid).await?,
    ))
}
