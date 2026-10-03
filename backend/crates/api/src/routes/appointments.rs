//! Appointments: staff offer and manage them, the resident picks in the
//! portal, and anyone with the link picks or declines from a public page.

use crate::appointments::{self as appt, Window};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::tenancy::{Access, TenantScope};
use chrono::{DateTime, Duration, Utc};
use entity::prelude::{Appointment, Counterparty, MaintenanceTicket, Property, User};
use rocket::serde::json::Json;
use rocket::{get, patch, post};
use schemars::JsonSchema;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Serialize, JsonSchema)]
pub struct AppointmentDto {
    pub id: Uuid,
    pub property_id: Uuid,
    pub property_name: Option<String>,
    pub unit_id: Option<Uuid>,
    pub kind: String,
    pub subject_type: String,
    pub subject_id: Option<Uuid>,
    pub title: String,
    pub status: String,
    pub windows: Vec<Window>,
    /// The offered windows in words, in the workspace's time zone.
    pub windows_words: Vec<String>,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    /// The confirmed time in words.
    pub when_words: Option<String>,
    pub with_name: Option<String>,
    pub with_email: Option<String>,
    pub with_phone: Option<String>,
    pub with_role: String,
    pub assignee_user_id: Option<Uuid>,
    pub assignee_name: Option<String>,
    pub vendor_entity_id: Option<Uuid>,
    pub vendor_name: Option<String>,
    pub note: Option<String>,
    pub access_notes: Option<String>,
    pub confirmed_by: Option<String>,
    pub confirmed_at: Option<String>,
    pub proposed_start: Option<String>,
    pub proposed_end: Option<String>,
    pub proposed_words: Option<String>,
    pub outcome_note: Option<String>,
    pub created_at: String,
}

struct Names {
    tz: chrono_tz::Tz,
    properties: HashMap<Uuid, String>,
    people: HashMap<Uuid, String>,
    vendors: HashMap<Uuid, String>,
}

async fn names_for(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    rows: &[entity::appointment::Model],
) -> ApiResult<Names> {
    let tz = appt::tz_for(db, tenant_id).await;
    let pids: Vec<Uuid> = rows.iter().map(|a| a.property_id).collect();
    let uids: Vec<Uuid> = rows.iter().filter_map(|a| a.assignee_user_id).collect();
    let vids: Vec<Uuid> = rows.iter().filter_map(|a| a.vendor_entity_id).collect();
    let properties = if pids.is_empty() {
        HashMap::new()
    } else {
        Property::find()
            .filter(entity::property::Column::Id.is_in(pids))
            .all(db)
            .await?
            .into_iter()
            .map(|p| (p.id, p.name))
            .collect()
    };
    let people = if uids.is_empty() {
        HashMap::new()
    } else {
        User::find()
            .filter(entity::user::Column::Id.is_in(uids))
            .all(db)
            .await?
            .into_iter()
            .map(|u| (u.id, u.name))
            .collect()
    };
    let vendors = if vids.is_empty() {
        HashMap::new()
    } else {
        Counterparty::find()
            .filter(entity::counterparty::Column::Id.is_in(vids))
            .all(db)
            .await?
            .into_iter()
            .map(|c| (c.id, c.name))
            .collect()
    };
    Ok(Names {
        tz,
        properties,
        people,
        vendors,
    })
}

fn dto(a: entity::appointment::Model, n: &Names) -> AppointmentDto {
    let windows = appt::windows_of(&a.windows);
    let when = match (a.starts_at, a.ends_at) {
        (Some(s), Some(e)) => Some(appt::window_words(
            &Window {
                start: s.to_utc(),
                end: e.to_utc(),
            },
            &n.tz,
        )),
        _ => None,
    };
    let proposed = match (a.proposed_start, a.proposed_end) {
        (Some(s), Some(e)) => Some(appt::window_words(
            &Window {
                start: s.to_utc(),
                end: e.to_utc(),
            },
            &n.tz,
        )),
        _ => None,
    };
    AppointmentDto {
        property_name: n.properties.get(&a.property_id).cloned(),
        assignee_name: a.assignee_user_id.and_then(|u| n.people.get(&u).cloned()),
        vendor_name: a.vendor_entity_id.and_then(|v| n.vendors.get(&v).cloned()),
        windows_words: windows
            .iter()
            .map(|w| appt::window_words(w, &n.tz))
            .collect(),
        windows,
        when_words: when,
        proposed_words: proposed,
        id: a.id,
        property_id: a.property_id,
        unit_id: a.unit_id,
        kind: a.kind,
        subject_type: a.subject_type,
        subject_id: a.subject_id,
        title: a.title,
        status: a.status,
        starts_at: a.starts_at.map(|t| t.to_rfc3339()),
        ends_at: a.ends_at.map(|t| t.to_rfc3339()),
        with_name: a.with_name,
        with_email: a.with_email,
        with_phone: a.with_phone,
        with_role: a.with_role,
        assignee_user_id: a.assignee_user_id,
        vendor_entity_id: a.vendor_entity_id,
        note: a.note,
        access_notes: a.access_notes,
        confirmed_by: a.confirmed_by,
        confirmed_at: a.confirmed_at.map(|t| t.to_rfc3339()),
        proposed_start: a.proposed_start.map(|t| t.to_rfc3339()),
        proposed_end: a.proposed_end.map(|t| t.to_rfc3339()),
        outcome_note: a.outcome_note,
        created_at: a.created_at.to_rfc3339(),
    }
}

fn parse_id(id: &str) -> ApiResult<Uuid> {
    Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))
}

fn text(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn one_of(label: &str, v: Option<String>, allowed: &[&str], default: &str) -> ApiResult<String> {
    match text(v).map(|s| s.to_lowercase()) {
        None => Ok(default.into()),
        Some(s) if allowed.contains(&s.as_str()) => Ok(s),
        Some(s) => Err(ApiError::BadRequest(format!(
            "{label} \"{s}\" isn't one of: {}",
            allowed.join(", ")
        ))),
    }
}

// ---------------------------------------------------------------------------
// Staff
// ---------------------------------------------------------------------------

/// `GET /appointments?<from>&<to>&<status>&<assignee>&<property_id>&<subject_type>&<subject_id>`
/// — the calendar. Dates are `YYYY-MM-DD`; a proposed appointment with no
/// time yet shows under its first window. Narrowed by reach.
#[rocket_okapi::openapi(tag = "Appointments")]
#[allow(clippy::too_many_arguments)]
#[get("/appointments?<from>&<to>&<status>&<assignee>&<property_id>&<subject_type>&<subject_id>")]
pub async fn list_appointments(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    from: Option<String>,
    to: Option<String>,
    status: Option<String>,
    assignee: Option<String>,
    property_id: Option<String>,
    subject_type: Option<String>,
    subject_id: Option<String>,
) -> ApiResult<Json<Vec<AppointmentDto>>> {
    user.require(Permission::MaintenanceRead)?;
    let mut q =
        Appointment::find().filter(entity::appointment::Column::TenantId.eq(scope.tenant_id));
    if let Some(ids) = access.property_ids() {
        q = q.filter(entity::appointment::Column::PropertyId.is_in(ids));
    }
    if let Some(s) = text(status) {
        q = q.filter(entity::appointment::Column::Status.eq(s));
    }
    if let Some(a) = text(assignee) {
        q = q.filter(entity::appointment::Column::AssigneeUserId.eq(parse_id(&a)?));
    }
    if let Some(p) = text(property_id) {
        q = q.filter(entity::appointment::Column::PropertyId.eq(parse_id(&p)?));
    }
    if let Some(st) = text(subject_type) {
        q = q.filter(entity::appointment::Column::SubjectType.eq(st));
    }
    if let Some(sid) = text(subject_id) {
        q = q.filter(entity::appointment::Column::SubjectId.eq(parse_id(&sid)?));
    }
    let rows = q
        .order_by_asc(entity::appointment::Column::StartsAt)
        .order_by_desc(entity::appointment::Column::CreatedAt)
        .all(&db)
        .await?;
    let day = |s: Option<String>| -> Option<chrono::NaiveDate> {
        text(s).and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok())
    };
    let (from, to) = (day(from), day(to));
    let n = names_for(&db, scope.tenant_id, &rows).await?;
    let rows: Vec<_> = rows
        .into_iter()
        .filter(|a| {
            if from.is_none() && to.is_none() {
                return true;
            }
            let at: Option<DateTime<Utc>> = a
                .starts_at
                .map(|t| t.to_utc())
                .or_else(|| appt::windows_of(&a.windows).first().map(|w| w.start));
            let Some(at) = at else { return true };
            let d = at.with_timezone(&n.tz).date_naive();
            from.is_none_or(|f| d >= f) && to.is_none_or(|t| d <= t)
        })
        .collect();
    Ok(Json(rows.into_iter().map(|a| dto(a, &n)).collect()))
}

#[derive(Deserialize, JsonSchema)]
pub struct CreateAppointmentReq {
    /// The work order the visit is for. Fills in property, resident and title.
    pub ticket_id: Option<Uuid>,
    /// Or any property, for a showing or inspection.
    pub property_id: Option<Uuid>,
    pub unit_id: Option<Uuid>,
    pub kind: Option<String>,
    pub title: Option<String>,
    /// Up to four windows. `end` may be left out to use the default length.
    pub windows: Vec<WindowReq>,
    pub with_name: Option<String>,
    pub with_email: Option<String>,
    pub with_phone: Option<String>,
    pub with_role: Option<String>,
    pub assignee_user_id: Option<Uuid>,
    pub vendor_entity_id: Option<Uuid>,
    pub note: Option<String>,
    pub access_notes: Option<String>,
    pub lead_id: Option<Uuid>,
}

/// A time as RFC 3339, or a plain `YYYY-MM-DDTHH:MM` read in the
/// workspace's time zone (what a date and time field on a form produce).
#[derive(Deserialize, JsonSchema)]
pub struct WindowReq {
    pub start: String,
    pub end: Option<String>,
}

fn parse_when(raw: &str, tz: &chrono_tz::Tz) -> ApiResult<DateTime<Utc>> {
    use chrono::TimeZone;
    let raw = raw.trim();
    if let Ok(t) = DateTime::parse_from_rfc3339(raw) {
        return Ok(t.to_utc());
    }
    let naive = chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M")
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S"))
        .map_err(|_| ApiError::BadRequest(format!("\"{raw}\" isn't a date and time")))?;
    tz.from_local_datetime(&naive)
        .earliest()
        .map(|t| t.to_utc())
        .ok_or_else(|| ApiError::BadRequest("that time doesn't exist in this time zone".into()))
}

async fn window_from(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    w: WindowReq,
) -> ApiResult<Window> {
    let tz = appt::tz_for(db, tenant_id).await;
    let default_len = Duration::minutes(
        crate::settings::get_i64(db, tenant_id, crate::settings::APPOINTMENT_WINDOW_MINUTES)
            .await
            .clamp(15, 24 * 60),
    );
    let start = parse_when(&w.start, &tz)?;
    let end = match w.end {
        Some(e) => parse_when(&e, &tz)?,
        None => start + default_len,
    };
    Ok(Window { start, end })
}

#[derive(Serialize, JsonSchema)]
pub struct CreatedAppointment {
    #[serde(flatten)]
    pub appointment: AppointmentDto,
    /// The link that was sent, so staff can hand it over another way too.
    pub link: String,
}

/// `POST /appointments` — offer windows. For a work order, the resident on
/// the lease is the person unless another is given.
#[rocket_okapi::openapi(tag = "Appointments")]
#[post("/appointments", data = "<body>")]
pub async fn create_appointment(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    body: Json<CreateAppointmentReq>,
) -> ApiResult<Json<CreatedAppointment>> {
    user.require(Permission::MaintenanceManage)?;
    let b = body.into_inner();
    let mut raw = Vec::new();
    for w in b.windows {
        raw.push(window_from(&db, scope.tenant_id, w).await?);
    }
    let windows = appt::clean_windows(raw, Utc::now()).map_err(ApiError::BadRequest)?;

    let (mut offer, ticket) = match b.ticket_id {
        Some(tid) => {
            let t = MaintenanceTicket::find_by_id(tid)
                .filter(entity::maintenance_ticket::Column::TenantId.eq(scope.tenant_id))
                .one(&db)
                .await?
                .ok_or_else(|| ApiError::NotFound("work order not found".into()))?;
            if !access.sees(t.property_id) {
                return Err(ApiError::NotFound("work order not found".into()));
            }
            let lease = appt::resident_for_ticket(&db, scope.tenant_id, &t).await;
            let o = appt::Offer {
                property_id: t.property_id,
                unit_id: t.unit_id,
                kind: "repair".into(),
                subject_type: "ticket".into(),
                subject_id: Some(t.id),
                title: t.title.clone(),
                windows,
                with_name: lease.as_ref().map(|l| l.tenant_name.clone()),
                with_email: lease.as_ref().and_then(|l| l.tenant_email.clone()),
                with_phone: lease.as_ref().and_then(|l| l.tenant_phone.clone()),
                with_role: "resident".into(),
                assignee_user_id: t.assignee_user_id,
                vendor_entity_id: t.assignee_entity_id,
                note: None,
                access_notes: t.access_notes.clone(),
                created_by: Some(user.user_id),
            };
            (o, Some(t))
        }
        None => {
            let pid = b
                .property_id
                .ok_or_else(|| ApiError::BadRequest("give a work order or a property".into()))?;
            let p = Property::find_by_id(pid)
                .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
                .one(&db)
                .await?
                .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
            if !access.sees(p.id) {
                return Err(ApiError::NotFound("property not found".into()));
            }
            let title = text(b.title.clone())
                .ok_or_else(|| ApiError::BadRequest("say what the visit is for".into()))?;
            let o = appt::Offer {
                property_id: p.id,
                unit_id: b.unit_id,
                kind: "other".into(),
                subject_type: if b.lead_id.is_some() {
                    "lead".into()
                } else {
                    "custom".into()
                },
                subject_id: b.lead_id,
                title,
                windows,
                with_name: None,
                with_email: None,
                with_phone: None,
                with_role: "resident".into(),
                assignee_user_id: Some(user.user_id),
                vendor_entity_id: None,
                note: None,
                access_notes: None,
                created_by: Some(user.user_id),
            };
            (o, None)
        }
    };
    // Explicit fields win over what the work order supplied.
    if let Some(k) = text(b.kind) {
        offer.kind = one_of("kind", Some(k), appt::KINDS, "repair")?;
    } else if ticket.is_none() {
        offer.kind = if offer.subject_type == "lead" {
            "showing".into()
        } else {
            "other".into()
        };
    }
    if let Some(t) = text(b.title) {
        offer.title = t;
    }
    if let Some(n) = text(b.with_name) {
        offer.with_name = Some(n);
    }
    if let Some(e) = text(b.with_email) {
        if !e.contains('@') {
            return Err(ApiError::BadRequest("that email doesn't look right".into()));
        }
        offer.with_email = Some(e);
    }
    if let Some(p) = text(b.with_phone) {
        offer.with_phone = Some(p);
    }
    if b.with_role.is_some() {
        offer.with_role = one_of("with_role", b.with_role, appt::ROLES, "resident")?;
    }
    if b.assignee_user_id.is_some() {
        offer.assignee_user_id = b.assignee_user_id;
    }
    if b.vendor_entity_id.is_some() {
        offer.vendor_entity_id = b.vendor_entity_id;
    }
    if let Some(n) = text(b.note) {
        offer.note = Some(n);
    }
    if let Some(a) = text(b.access_notes) {
        offer.access_notes = Some(a);
    }
    if offer.with_email.is_none() && offer.with_phone.is_none() {
        return Err(ApiError::BadRequest(
            "there's no email or phone for the person; add one".into(),
        ));
    }
    let (saved, raw) = appt::offer(&db, scope.tenant_id, offer).await?;
    let n = names_for(&db, scope.tenant_id, std::slice::from_ref(&saved)).await?;
    Ok(Json(CreatedAppointment {
        appointment: dto(saved, &n),
        link: appt::book_url(&raw),
    }))
}

async fn appointment_in(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    id: &str,
) -> ApiResult<entity::appointment::Model> {
    Appointment::find_by_id(parse_id(id)?)
        .filter(entity::appointment::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("appointment not found".into()))
}

/// `GET /appointments/<id>`
#[rocket_okapi::openapi(tag = "Appointments")]
#[get("/appointments/<id>")]
pub async fn get_appointment(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<AppointmentDto>> {
    user.require(Permission::MaintenanceRead)?;
    let a = appointment_in(&db, scope.tenant_id, id).await?;
    let n = names_for(&db, scope.tenant_id, std::slice::from_ref(&a)).await?;
    Ok(Json(dto(a, &n)))
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct UpdateAppointmentReq {
    /// Confirm a time (one of the windows, or any time staff agreed on the
    /// phone).
    pub confirm: Option<WindowReq>,
    /// `cancelled` | `done` | `no_show`.
    pub status: Option<String>,
    pub assignee_user_id: Option<Uuid>,
    pub note: Option<String>,
    pub outcome_note: Option<String>,
}

/// `PATCH /appointments/<id>` — confirm a time by phone, hand it to someone,
/// or mark how it went.
#[rocket_okapi::openapi(tag = "Appointments")]
#[patch("/appointments/<id>", data = "<body>")]
pub async fn update_appointment(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateAppointmentReq>,
) -> ApiResult<Json<AppointmentDto>> {
    user.require(Permission::MaintenanceManage)?;
    let mut a = appointment_in(&db, scope.tenant_id, id).await?;
    let b = body.into_inner();
    let now = Utc::now();
    if let Some(w) = b.confirm {
        let w = window_from(&db, scope.tenant_id, w).await?;
        if w.end <= w.start {
            return Err(ApiError::BadRequest("the end is before the start".into()));
        }
        a = appt::confirm(&db, scope.tenant_id, a, w, "staff").await?;
    }
    let mut am: entity::appointment::ActiveModel = a.clone().into();
    let mut changed = false;
    if let Some(s) = text(b.status) {
        if !matches!(s.as_str(), "cancelled" | "done" | "no_show") {
            return Err(ApiError::BadRequest(
                "status must be cancelled, done or no_show".into(),
            ));
        }
        if matches!(s.as_str(), "done" | "no_show") && a.status != "confirmed" {
            return Err(ApiError::Conflict(
                "only a confirmed appointment can be marked done or no-show".into(),
            ));
        }
        if a.subject_type == "ticket" {
            if let Some(tid) = a.subject_id {
                let line = match s.as_str() {
                    "cancelled" => "Appointment cancelled.",
                    "no_show" => "Nobody was home for the appointment.",
                    _ => "Visit done.",
                };
                appt::note_on_ticket(
                    &db,
                    scope.tenant_id,
                    tid,
                    "appointment_outcome",
                    line,
                    "internal",
                )
                .await;
            }
        }
        am.status = sea_orm::Set(s);
        changed = true;
    }
    if let Some(u) = b.assignee_user_id {
        am.assignee_user_id = sea_orm::Set(Some(u));
        changed = true;
    }
    if let Some(n) = b.note {
        am.note = sea_orm::Set(text(Some(n)));
        changed = true;
    }
    if let Some(n) = b.outcome_note {
        am.outcome_note = sea_orm::Set(text(Some(n)));
        changed = true;
    }
    if changed {
        am.updated_at = sea_orm::Set(now.into());
        a = sea_orm::ActiveModelTrait::update(am, &db).await?;
    }
    let n = names_for(&db, scope.tenant_id, std::slice::from_ref(&a)).await?;
    Ok(Json(dto(a, &n)))
}

// ---------------------------------------------------------------------------
// The public link
// ---------------------------------------------------------------------------

#[derive(Serialize, JsonSchema)]
pub struct PublicAppointment {
    pub id: Uuid,
    pub company: String,
    pub title: String,
    pub kind: String,
    pub status: String,
    pub property: String,
    pub with_name: Option<String>,
    pub windows: Vec<Window>,
    pub windows_words: Vec<String>,
    pub when_words: Option<String>,
    pub note: Option<String>,
    pub timezone: String,
}

async fn public_dto(
    db: &crate::db::RequestDb,
    a: entity::appointment::Model,
) -> ApiResult<PublicAppointment> {
    let tz = appt::tz_for(db, a.tenant_id).await;
    let company = entity::prelude::Tenant::find_by_id(a.tenant_id)
        .one(db)
        .await?
        .map(|t| t.name)
        .unwrap_or_default();
    let property = Property::find_by_id(a.property_id)
        .one(db)
        .await?
        .map(|p| format!("{}, {}", p.address, p.city))
        .unwrap_or_default();
    let windows = appt::windows_of(&a.windows);
    Ok(PublicAppointment {
        id: a.id,
        company,
        title: a.title,
        kind: a.kind,
        status: a.status,
        property,
        with_name: a.with_name,
        windows_words: windows.iter().map(|w| appt::window_words(w, &tz)).collect(),
        windows,
        when_words: match (a.starts_at, a.ends_at) {
            (Some(s), Some(e)) => Some(appt::window_words(
                &Window {
                    start: s.to_utc(),
                    end: e.to_utc(),
                },
                &tz,
            )),
            _ => None,
        },
        note: a.note,
        timezone: tz.name().to_string(),
    })
}

/// `GET /public/book/<token>` — what the link opens: the visit and its
/// windows. Read-only, so link scanners change nothing.
#[rocket_okapi::openapi(tag = "Appointments (Public)")]
#[get("/public/book/<token>")]
pub async fn public_view(
    db: crate::db::RequestDb,
    token: &str,
) -> ApiResult<Json<PublicAppointment>> {
    let a = appt::by_token(&db, token).await?;
    Ok(Json(public_dto(&db, a).await?))
}

#[derive(Deserialize, JsonSchema)]
pub struct PickReq {
    /// Index into the offered windows.
    pub window: usize,
}

/// `POST /public/book/<token>` — pick a window.
#[rocket_okapi::openapi(tag = "Appointments (Public)")]
#[post("/public/book/<token>", data = "<body>")]
pub async fn public_pick(
    db: crate::db::RequestDb,
    token: &str,
    body: Json<PickReq>,
) -> ApiResult<Json<PublicAppointment>> {
    let a = appt::by_token(&db, token).await?;
    let windows = appt::windows_of(&a.windows);
    let w = windows
        .get(body.window)
        .cloned()
        .ok_or_else(|| ApiError::BadRequest("pick one of the offered times".into()))?;
    let by = a.with_role.clone();
    let tenant = a.tenant_id;
    let saved = appt::confirm(&db, tenant, a, w, &by).await?;
    Ok(Json(public_dto(&db, saved).await?))
}

#[derive(Deserialize, JsonSchema)]
pub struct DeclineReq {
    /// A time that would work instead, if any.
    pub propose: Option<WindowReq>,
    pub reason: Option<String>,
}

/// `POST /public/book/<token>/decline` — none of the times work; ask for
/// another, or just say so.
#[rocket_okapi::openapi(tag = "Appointments (Public)")]
#[post("/public/book/<token>/decline", data = "<body>")]
pub async fn public_decline(
    db: crate::db::RequestDb,
    token: &str,
    body: Json<DeclineReq>,
) -> ApiResult<Json<PublicAppointment>> {
    let a = appt::by_token(&db, token).await?;
    let b = body.into_inner();
    let proposed = proposed_window(&db, a.tenant_id, b.propose).await?;
    let tenant = a.tenant_id;
    let saved = appt::decline(&db, tenant, a, proposed, text(b.reason)).await?;
    Ok(Json(public_dto(&db, saved).await?))
}

async fn proposed_window(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    w: Option<WindowReq>,
) -> ApiResult<Option<Window>> {
    let Some(w) = w else { return Ok(None) };
    let w = window_from(db, tenant_id, w).await?;
    if w.start < Utc::now() || w.end <= w.start {
        return Err(ApiError::BadRequest(
            "suggest a time that's still ahead".into(),
        ));
    }
    Ok(Some(w))
}

// ---------------------------------------------------------------------------
// The resident, signed in
// ---------------------------------------------------------------------------

/// `GET /my/appointments` — the resident's visits, soonest first.
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[get("/my/appointments")]
pub async fn my_appointments(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<AppointmentDto>>> {
    let me = User::find_by_id(user.user_id)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("user not found".into()))?;
    let rows = Appointment::find()
        .filter(entity::appointment::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::appointment::Column::WithEmail.eq(me.email.to_lowercase()))
        .filter(entity::appointment::Column::Status.is_in(["proposed", "confirmed"]))
        .order_by_asc(entity::appointment::Column::StartsAt)
        .all(&db)
        .await?;
    let n = names_for(&db, scope.tenant_id, &rows).await?;
    Ok(Json(rows.into_iter().map(|a| dto(a, &n)).collect()))
}

async fn mine(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    user_id: Uuid,
    id: &str,
) -> ApiResult<entity::appointment::Model> {
    let me = User::find_by_id(user_id)
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("user not found".into()))?;
    Appointment::find_by_id(parse_id(id)?)
        .filter(entity::appointment::Column::TenantId.eq(tenant_id))
        .filter(entity::appointment::Column::WithEmail.eq(me.email.to_lowercase()))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("appointment not found".into()))
}

/// `POST /my/appointments/<id>/pick`
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[post("/my/appointments/<id>/pick", data = "<body>")]
pub async fn my_pick(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<PickReq>,
) -> ApiResult<Json<AppointmentDto>> {
    let a = mine(&db, scope.tenant_id, user.user_id, id).await?;
    let w = appt::windows_of(&a.windows)
        .get(body.window)
        .cloned()
        .ok_or_else(|| ApiError::BadRequest("pick one of the offered times".into()))?;
    let saved = appt::confirm(&db, scope.tenant_id, a, w, "resident").await?;
    let n = names_for(&db, scope.tenant_id, std::slice::from_ref(&saved)).await?;
    Ok(Json(dto(saved, &n)))
}

/// `POST /my/appointments/<id>/decline`
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[post("/my/appointments/<id>/decline", data = "<body>")]
pub async fn my_decline(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<DeclineReq>,
) -> ApiResult<Json<AppointmentDto>> {
    let a = mine(&db, scope.tenant_id, user.user_id, id).await?;
    let b = body.into_inner();
    let proposed = proposed_window(&db, scope.tenant_id, b.propose).await?;
    let saved = appt::decline(&db, scope.tenant_id, a, proposed, text(b.reason)).await?;
    let n = names_for(&db, scope.tenant_id, std::slice::from_ref(&saved)).await?;
    Ok(Json(dto(saved, &n)))
}
