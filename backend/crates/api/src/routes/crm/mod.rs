//! **CRM** (Vantedge phase 2B) — owners are the property manager's clients.
//!
//! * Owner directory: each owner with their LLCs, properties, doors, last
//!   contact and open follow-ups.
//! * Timeline: notes / calls / emails / meetings / issues / texts about an
//!   owner, owner lead, vendor or property — pinned, and with follow-ups that
//!   stay open until done (the due ones show on the dashboard).
//! * Owner-lead pipeline: new → contacted → proposal → won / lost, by source,
//!   with the doors and monthly fee at stake; a printable management proposal;
//!   converting a won lead creates the owner.
//!
//! `entity:read` to look, `entity:manage` to change — the same permissions as
//! the rest of the entities registry.

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::pdfdoc::{money, pct, Block, Column, Document, Table};
use crate::rbac::Permission;
use crate::routes::reports::ReportFile;
use crate::routes::team::parse_id;
use crate::tenancy::TenantScope;
use chrono::{NaiveDate, Utc};
use entity::prelude::{
    Counterparty, CrmNote, EntityOwnership, Llc, Owner, OwnerLead, Property, Theme, Unit, User,
};
use rocket::serde::json::Json;
use rocket::{delete, get, patch, post};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

const SUBJECTS: &[&str] = &["owner", "owner_lead", "counterparty", "property"];
const NOTE_KINDS: &[&str] = &[
    "note", "call", "email", "meeting", "issue", "text", "update",
];
const LEAD_STATUSES: &[&str] = &["new", "contacted", "proposal", "won", "lost"];
const LEAD_SOURCES: &[&str] = &[
    "website", "referral", "phone", "email", "event", "mailer", "other",
];

/// How likely a lead in each stage is to close (for the weighted pipeline).
pub fn stage_weight_bps(status: &str) -> i64 {
    match status {
        "new" => 1_000,
        "contacted" => 2_500,
        "proposal" => 5_000,
        "won" => 10_000,
        _ => 0,
    }
}

fn clean(s: Option<String>) -> Option<String> {
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

async fn today(db: &impl ConnectionTrait, tenant_id: Uuid) -> NaiveDate {
    crate::workforce::Rules::load(db, tenant_id)
        .await
        .local_date(Utc::now().into())
}

// ---------------------------------------------------------------------------
// Timeline
// ---------------------------------------------------------------------------

#[derive(Serialize, schemars::JsonSchema)]
pub struct NoteDto {
    pub id: Uuid,
    pub subject_type: String,
    pub subject_id: Uuid,
    pub subject_name: Option<String>,
    pub property_id: Option<Uuid>,
    pub kind: String,
    pub body: String,
    pub pinned: bool,
    pub follow_up_on: Option<String>,
    pub follow_up_done: bool,
    /// Open and on or before today.
    pub follow_up_due: bool,
    pub author: Option<String>,
    pub created_at: String,
}

async fn subject_names(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    notes: &[entity::crm_note::Model],
) -> ApiResult<HashMap<Uuid, String>> {
    let of = |t: &str| -> Vec<Uuid> {
        notes
            .iter()
            .filter(|n| n.subject_type == t)
            .map(|n| n.subject_id)
            .collect()
    };
    let mut out = HashMap::new();
    for o in Owner::find()
        .filter(entity::owner::Column::TenantId.eq(tenant_id))
        .filter(entity::owner::Column::Id.is_in(of("owner")))
        .all(db)
        .await?
    {
        out.insert(o.id, o.name);
    }
    for l in OwnerLead::find()
        .filter(entity::owner_lead::Column::TenantId.eq(tenant_id))
        .filter(entity::owner_lead::Column::Id.is_in(of("owner_lead")))
        .all(db)
        .await?
    {
        out.insert(l.id, l.name);
    }
    for c in Counterparty::find()
        .filter(entity::counterparty::Column::TenantId.eq(tenant_id))
        .filter(entity::counterparty::Column::Id.is_in(of("counterparty")))
        .all(db)
        .await?
    {
        out.insert(c.id, c.name);
    }
    for p in Property::find()
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .filter(entity::property::Column::Id.is_in(of("property")))
        .all(db)
        .await?
    {
        out.insert(p.id, p.name);
    }
    Ok(out)
}

async fn note_dtos(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    notes: Vec<entity::crm_note::Model>,
) -> ApiResult<Vec<NoteDto>> {
    let day = today(db, tenant_id).await.to_string();
    let subjects = subject_names(db, tenant_id, &notes).await?;
    let authors: HashMap<Uuid, String> = User::find()
        .filter(
            entity::user::Column::Id
                .is_in(notes.iter().filter_map(|n| n.author_id).collect::<Vec<_>>()),
        )
        .all(db)
        .await?
        .into_iter()
        .map(|u| (u.id, u.name))
        .collect();
    Ok(notes
        .into_iter()
        .map(|n| NoteDto {
            id: n.id,
            subject_name: subjects.get(&n.subject_id).cloned(),
            follow_up_due: n.follow_up_done_at.is_none()
                && n.follow_up_on.as_deref().is_some_and(|d| d <= day.as_str()),
            follow_up_done: n.follow_up_done_at.is_some(),
            author: n.author_id.and_then(|a| authors.get(&a).cloned()),
            subject_type: n.subject_type,
            subject_id: n.subject_id,
            property_id: n.property_id,
            kind: n.kind,
            body: n.body,
            pinned: n.pinned,
            follow_up_on: n.follow_up_on,
            created_at: n.created_at.to_rfc3339(),
        })
        .collect())
}

/// Check a subject exists in the workspace.
async fn check_subject(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    kind: &str,
    id: Uuid,
) -> ApiResult<()> {
    let found = match kind {
        "owner" => Owner::find_by_id(id)
            .filter(entity::owner::Column::TenantId.eq(tenant_id))
            .one(db)
            .await?
            .is_some(),
        "owner_lead" => OwnerLead::find_by_id(id)
            .filter(entity::owner_lead::Column::TenantId.eq(tenant_id))
            .one(db)
            .await?
            .is_some(),
        "counterparty" => Counterparty::find_by_id(id)
            .filter(entity::counterparty::Column::TenantId.eq(tenant_id))
            .one(db)
            .await?
            .is_some(),
        "property" => Property::find_by_id(id)
            .filter(entity::property::Column::TenantId.eq(tenant_id))
            .one(db)
            .await?
            .is_some(),
        _ => false,
    };
    if found {
        Ok(())
    } else {
        Err(ApiError::NotFound(format!("{kind} not found")))
    }
}

/// `GET /crm/notes?subject_type&subject_id` — the timeline (pinned first).
#[rocket_okapi::openapi(tag = "CRM")]
#[get("/crm/notes?<subject_type>&<subject_id>")]
pub async fn list_notes(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    subject_type: String,
    subject_id: String,
) -> ApiResult<Json<Vec<NoteDto>>> {
    user.require(Permission::EntityRead)?;
    let sid = parse_id(&subject_id, "subject")?;
    let mut notes = CrmNote::find()
        .filter(entity::crm_note::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::crm_note::Column::SubjectType.eq(subject_type))
        .filter(entity::crm_note::Column::SubjectId.eq(sid))
        .order_by_desc(entity::crm_note::Column::CreatedAt)
        .all(&db)
        .await?;
    notes.sort_by_key(|n| !n.pinned);
    Ok(Json(note_dtos(&db, scope.tenant_id, notes).await?))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct NoteReq {
    pub subject_type: String,
    pub subject_id: Uuid,
    pub property_id: Option<Uuid>,
    pub kind: Option<String>,
    pub body: String,
    pub pinned: Option<bool>,
    /// `YYYY-MM-DD`.
    pub follow_up_on: Option<String>,
}

/// `POST /crm/notes`.
#[rocket_okapi::openapi(tag = "CRM")]
#[post("/crm/notes", data = "<body>")]
pub async fn add_note(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<NoteReq>,
) -> ApiResult<Json<NoteDto>> {
    user.require(Permission::EntityManage)?;
    let b = body.into_inner();
    if !SUBJECTS.contains(&b.subject_type.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "subject_type must be one of {}",
            SUBJECTS.join(", ")
        )));
    }
    check_subject(&db, scope.tenant_id, &b.subject_type, b.subject_id).await?;
    let kind = b.kind.unwrap_or_else(|| "note".into());
    if !NOTE_KINDS.contains(&kind.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "kind must be one of {}",
            NOTE_KINDS.join(", ")
        )));
    }
    let text = b.body.trim().to_string();
    if text.is_empty() || text.chars().count() > 10_000 {
        return Err(ApiError::BadRequest(
            "write something (up to 10,000 characters)".into(),
        ));
    }
    if let Some(d) = &b.follow_up_on {
        d.parse::<NaiveDate>()
            .map_err(|_| ApiError::BadRequest("follow_up_on must be a date".into()))?;
    }
    let now = Utc::now();
    let n = entity::crm_note::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        subject_type: Set(b.subject_type),
        subject_id: Set(b.subject_id),
        property_id: Set(b.property_id),
        kind: Set(kind),
        body: Set(text),
        pinned: Set(b.pinned.unwrap_or(false)),
        follow_up_on: Set(clean(b.follow_up_on)),
        follow_up_done_at: Set(None),
        follow_up_done_by: Set(None),
        author_id: Set(Some(user.user_id)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::CRM_NOTE_ADD,
        Some(n.subject_type.as_str()),
        Some(n.subject_id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "kind": n.kind })),
    )
    .await;
    Ok(Json(
        note_dtos(&db, scope.tenant_id, vec![n]).await?.remove(0),
    ))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct NotePatch {
    pub body: Option<String>,
    pub kind: Option<String>,
    pub pinned: Option<bool>,
    /// A date, or an empty string to clear it.
    pub follow_up_on: Option<String>,
    /// Mark the follow-up done (true) or reopen it (false).
    pub follow_up_done: Option<bool>,
}

async fn find_note(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    id: &str,
) -> ApiResult<entity::crm_note::Model> {
    CrmNote::find_by_id(parse_id(id, "note")?)
        .filter(entity::crm_note::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("note not found".into()))
}

/// `PATCH /crm/notes/<id>` — edit, pin, reschedule or close a follow-up.
#[rocket_okapi::openapi(tag = "CRM")]
#[patch("/crm/notes/<id>", data = "<body>")]
pub async fn update_note(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<NotePatch>,
) -> ApiResult<Json<NoteDto>> {
    user.require(Permission::EntityManage)?;
    let n = find_note(&db, scope.tenant_id, id).await?;
    let mut am: entity::crm_note::ActiveModel = n.into();
    if let Some(b) = body.body.clone() {
        let b = b.trim().to_string();
        if b.is_empty() {
            return Err(ApiError::BadRequest("a note can't be empty".into()));
        }
        am.body = Set(b);
    }
    if let Some(p) = body.pinned {
        am.pinned = Set(p);
    }
    if let Some(k) = body.kind.clone() {
        if !NOTE_KINDS.contains(&k.as_str()) {
            return Err(ApiError::BadRequest(format!(
                "kind must be one of {}",
                NOTE_KINDS.join(", ")
            )));
        }
        am.kind = Set(k);
    }
    if let Some(d) = body.follow_up_on.clone() {
        let d = clean(Some(d));
        if let Some(x) = &d {
            x.parse::<NaiveDate>()
                .map_err(|_| ApiError::BadRequest("follow_up_on must be a date".into()))?;
        }
        am.follow_up_on = Set(d);
    }
    match body.follow_up_done {
        Some(true) => {
            am.follow_up_done_at = Set(Some(Utc::now().into()));
            am.follow_up_done_by = Set(Some(user.user_id));
        }
        Some(false) => {
            am.follow_up_done_at = Set(None);
            am.follow_up_done_by = Set(None);
        }
        None => {}
    }
    am.updated_at = Set(Utc::now().into());
    let n = am.update(&db).await?;
    Ok(Json(
        note_dtos(&db, scope.tenant_id, vec![n]).await?.remove(0),
    ))
}

/// `DELETE /crm/notes/<id>`.
#[rocket_okapi::openapi(tag = "CRM")]
#[delete("/crm/notes/<id>")]
pub async fn delete_note(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::EntityManage)?;
    let n = find_note(&db, scope.tenant_id, id).await?;
    CrmNote::delete_by_id(n.id).exec(&db).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// `GET /crm/follow-ups?all` — open follow-ups, due first (`all=true` for the
/// ones not due yet too).
#[rocket_okapi::openapi(tag = "CRM")]
#[get("/crm/follow-ups?<all>")]
pub async fn follow_ups(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    all: Option<bool>,
) -> ApiResult<Json<Vec<NoteDto>>> {
    user.require(Permission::EntityRead)?;
    let day = today(&db, scope.tenant_id).await.to_string();
    let mut q = CrmNote::find()
        .filter(entity::crm_note::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::crm_note::Column::FollowUpOn.is_not_null())
        .filter(entity::crm_note::Column::FollowUpDoneAt.is_null());
    if !all.unwrap_or(false) {
        q = q.filter(entity::crm_note::Column::FollowUpOn.lte(day));
    }
    let notes = q
        .order_by_asc(entity::crm_note::Column::FollowUpOn)
        .all(&db)
        .await?;
    Ok(Json(note_dtos(&db, scope.tenant_id, notes).await?))
}

// ---------------------------------------------------------------------------
// Owners
// ---------------------------------------------------------------------------

#[derive(Serialize, schemars::JsonSchema)]
pub struct OwnerDto {
    pub id: Uuid,
    pub kind: String,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub notes: Option<String>,
    pub entities: Vec<String>,
    pub properties: usize,
    pub doors: usize,
    pub last_contact_at: Option<String>,
    pub open_follow_ups: usize,
    pub follow_ups_due: usize,
    /// The owner's portal login, once invited.
    pub user_id: Option<Uuid>,
    /// The owner's own approval limit; `None` uses the workspace's.
    pub approval_limit_cents: Option<i64>,
}

/// `GET /crm/owners` — every owner with what they own and where things stand.
#[rocket_okapi::openapi(tag = "CRM")]
#[get("/crm/owners")]
pub async fn owners(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<OwnerDto>>> {
    user.require(Permission::EntityRead)?;
    let t = scope.tenant_id;
    let day = today(&db, t).await.to_string();
    let list = Owner::find()
        .filter(entity::owner::Column::TenantId.eq(t))
        .all(&db)
        .await?;
    let stakes = EntityOwnership::find()
        .filter(entity::entity_ownership::Column::TenantId.eq(t))
        .all(&db)
        .await?;
    let llcs: HashMap<Uuid, String> = Llc::find()
        .filter(entity::llc::Column::TenantId.eq(t))
        .all(&db)
        .await?
        .into_iter()
        .map(|l| (l.id, l.name))
        .collect();
    let props = Property::find()
        .filter(entity::property::Column::TenantId.eq(t))
        .all(&db)
        .await?;
    let mut units: HashMap<Uuid, usize> = HashMap::new();
    for u in Unit::find()
        .filter(entity::unit::Column::TenantId.eq(t))
        .all(&db)
        .await?
    {
        *units.entry(u.property_id).or_default() += 1;
    }
    let notes = CrmNote::find()
        .filter(entity::crm_note::Column::TenantId.eq(t))
        .filter(entity::crm_note::Column::SubjectType.eq("owner"))
        .all(&db)
        .await?;
    let mut out: Vec<OwnerDto> = list
        .into_iter()
        .map(|o| {
            let entity_ids: HashSet<Uuid> = stakes
                .iter()
                .filter(|s| s.owner_id == o.id)
                .map(|s| s.entity_id)
                .collect();
            let owned: Vec<&entity::property::Model> = props
                .iter()
                .filter(|p| p.llc_id.is_some_and(|l| entity_ids.contains(&l)))
                .collect();
            let mine: Vec<&entity::crm_note::Model> =
                notes.iter().filter(|n| n.subject_id == o.id).collect();
            let open: Vec<&&entity::crm_note::Model> = mine
                .iter()
                .filter(|n| n.follow_up_on.is_some() && n.follow_up_done_at.is_none())
                .collect();
            OwnerDto {
                entities: entity_ids
                    .iter()
                    .filter_map(|e| llcs.get(e).cloned())
                    .collect(),
                properties: owned.len(),
                doors: owned
                    .iter()
                    .map(|p| units.get(&p.id).copied().unwrap_or(1).max(1))
                    .sum(),
                last_contact_at: mine
                    .iter()
                    .map(|n| n.created_at)
                    .max()
                    .map(|d| d.to_rfc3339()),
                open_follow_ups: open.len(),
                follow_ups_due: open
                    .iter()
                    .filter(|n| n.follow_up_on.as_deref().is_some_and(|d| d <= day.as_str()))
                    .count(),
                id: o.id,
                kind: o.kind,
                name: o.name,
                email: o.email,
                phone: o.phone,
                notes: o.notes,
                user_id: o.user_id,
                approval_limit_cents: o.approval_limit_cents,
            }
        })
        .collect();
    out.sort_by(|a, b| {
        b.follow_ups_due
            .cmp(&a.follow_ups_due)
            .then(a.name.cmp(&b.name))
    });
    Ok(Json(out))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct OwnerReq {
    pub name: String,
    /// `individual` | `company` | `firm`
    pub kind: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub notes: Option<String>,
}

/// `POST /crm/owners`.
#[rocket_okapi::openapi(tag = "CRM")]
#[post("/crm/owners", data = "<body>")]
pub async fn create_owner(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<OwnerReq>,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::EntityManage)?;
    let o = insert_owner(&db, scope.tenant_id, body.into_inner()).await?;
    Ok(Json(serde_json::json!({ "id": o.id, "name": o.name })))
}

async fn insert_owner(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    b: OwnerReq,
) -> ApiResult<entity::owner::Model> {
    let name = b.name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::BadRequest("the owner needs a name".into()));
    }
    let kind = b.kind.unwrap_or_else(|| "individual".into());
    if !["individual", "company", "firm"].contains(&kind.as_str()) {
        return Err(ApiError::BadRequest(
            "kind must be individual, company or firm".into(),
        ));
    }
    Ok(entity::owner::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        kind: Set(kind),
        name: Set(name),
        email: Set(clean(b.email).map(|e| e.to_lowercase())),
        phone: Set(clean(b.phone)),
        notes: Set(clean(b.notes)),
        created_at: Set(Utc::now().into()),
        user_id: Set(None),
        approval_limit_cents: Set(None),
    }
    .insert(db)
    .await?)
}

/// `PATCH /crm/owners/<id>`.
#[rocket_okapi::openapi(tag = "CRM")]
#[patch("/crm/owners/<id>", data = "<body>")]
pub async fn update_owner(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<OwnerReq>,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::EntityManage)?;
    let o = Owner::find_by_id(parse_id(id, "owner")?)
        .filter(entity::owner::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("owner not found".into()))?;
    let b = body.into_inner();
    let mut am: entity::owner::ActiveModel = o.into();
    let name = b.name.trim().to_string();
    if !name.is_empty() {
        am.name = Set(name);
    }
    if let Some(k) = b.kind {
        if !["individual", "company", "firm"].contains(&k.as_str()) {
            return Err(ApiError::BadRequest(
                "kind must be individual, company or firm".into(),
            ));
        }
        am.kind = Set(k);
    }
    if b.email.is_some() {
        am.email = Set(clean(b.email).map(|e| e.to_lowercase()));
    }
    if b.phone.is_some() {
        am.phone = Set(clean(b.phone));
    }
    if b.notes.is_some() {
        am.notes = Set(clean(b.notes));
    }
    let o = am.update(&db).await?;
    Ok(Json(serde_json::json!({ "id": o.id, "name": o.name })))
}

// ---------------------------------------------------------------------------
// Owner leads
// ---------------------------------------------------------------------------

#[derive(Serialize, schemars::JsonSchema)]
pub struct OwnerLeadDto {
    pub id: Uuid,
    pub name: String,
    pub company: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub properties_count: i32,
    pub doors: i32,
    pub source: String,
    pub status: String,
    pub lost_reason: Option<String>,
    pub monthly_rent_cents: i64,
    /// The management fee offered, basis points of rent (default: the workspace fee).
    pub fee_bps: i64,
    /// monthly rent × fee.
    pub monthly_fee_cents: i64,
    pub notes: Option<String>,
    pub assigned_to: Option<Uuid>,
    pub assigned_name: Option<String>,
    pub owner_id: Option<Uuid>,
    pub next_follow_up: Option<String>,
    pub created_at: String,
}

async fn default_fee_bps(db: &impl ConnectionTrait, tenant_id: Uuid) -> i64 {
    crate::settings::get_i64(db, tenant_id, crate::settings::PAYOUT_MGMT_FEE_BPS).await
}

async fn lead_dtos(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    leads: Vec<entity::owner_lead::Model>,
) -> ApiResult<Vec<OwnerLeadDto>> {
    let fee = default_fee_bps(db, tenant_id).await;
    let users: HashMap<Uuid, String> = User::find()
        .filter(
            entity::user::Column::Id.is_in(
                leads
                    .iter()
                    .filter_map(|l| l.assigned_to)
                    .collect::<Vec<_>>(),
            ),
        )
        .all(db)
        .await?
        .into_iter()
        .map(|u| (u.id, u.name))
        .collect();
    let mut next: HashMap<Uuid, String> = HashMap::new();
    for n in CrmNote::find()
        .filter(entity::crm_note::Column::TenantId.eq(tenant_id))
        .filter(entity::crm_note::Column::SubjectType.eq("owner_lead"))
        .filter(entity::crm_note::Column::FollowUpDoneAt.is_null())
        .filter(entity::crm_note::Column::FollowUpOn.is_not_null())
        .all(db)
        .await?
    {
        let d = n.follow_up_on.unwrap_or_default();
        next.entry(n.subject_id)
            .and_modify(|x| {
                if d < *x {
                    *x = d.clone()
                }
            })
            .or_insert(d);
    }
    Ok(leads
        .into_iter()
        .map(|l| {
            let bps = l.fee_bps.map(i64::from).unwrap_or(fee);
            OwnerLeadDto {
                monthly_fee_cents: crate::workforce::overtime::div_round(
                    l.monthly_rent_cents * bps,
                    10_000,
                ),
                fee_bps: bps,
                assigned_name: l.assigned_to.and_then(|u| users.get(&u).cloned()),
                next_follow_up: next.get(&l.id).cloned(),
                id: l.id,
                name: l.name,
                company: l.company,
                email: l.email,
                phone: l.phone,
                address: l.address,
                properties_count: l.properties_count,
                doors: l.doors,
                source: l.source,
                status: l.status,
                lost_reason: l.lost_reason,
                monthly_rent_cents: l.monthly_rent_cents,
                notes: l.notes,
                assigned_to: l.assigned_to,
                owner_id: l.owner_id,
                created_at: l.created_at.to_rfc3339(),
            }
        })
        .collect())
}

/// `GET /crm/owner-leads?status`.
#[rocket_okapi::openapi(tag = "CRM")]
#[get("/crm/owner-leads?<status>")]
pub async fn list_leads(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    status: Option<String>,
) -> ApiResult<Json<Vec<OwnerLeadDto>>> {
    user.require(Permission::EntityRead)?;
    let mut q = OwnerLead::find().filter(entity::owner_lead::Column::TenantId.eq(scope.tenant_id));
    if let Some(s) = status.filter(|s| !s.is_empty()) {
        q = q.filter(entity::owner_lead::Column::Status.eq(s));
    }
    let leads = q
        .order_by_desc(entity::owner_lead::Column::UpdatedAt)
        .all(&db)
        .await?;
    Ok(Json(lead_dtos(&db, scope.tenant_id, leads).await?))
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct OwnerLeadReq {
    pub name: Option<String>,
    pub company: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub properties_count: Option<i32>,
    pub doors: Option<i32>,
    pub source: Option<String>,
    pub status: Option<String>,
    pub lost_reason: Option<String>,
    pub monthly_rent_cents: Option<i64>,
    pub fee_bps: Option<i32>,
    pub notes: Option<String>,
    pub assigned_to: Option<Uuid>,
}

fn check_lead(b: &OwnerLeadReq) -> ApiResult<()> {
    if let Some(s) = &b.status {
        if !LEAD_STATUSES.contains(&s.as_str()) {
            return Err(ApiError::BadRequest(format!(
                "status must be one of {}",
                LEAD_STATUSES.join(", ")
            )));
        }
    }
    if let Some(s) = &b.source {
        if !LEAD_SOURCES.contains(&s.as_str()) {
            return Err(ApiError::BadRequest(format!(
                "source must be one of {}",
                LEAD_SOURCES.join(", ")
            )));
        }
    }
    if b.doors.is_some_and(|d| !(0..=100_000).contains(&d))
        || b.properties_count
            .is_some_and(|d| !(0..=100_000).contains(&d))
    {
        return Err(ApiError::BadRequest(
            "doors and properties must be 0 or more".into(),
        ));
    }
    if b.monthly_rent_cents.is_some_and(|r| r < 0)
        || b.fee_bps.is_some_and(|f| !(0..=5_000).contains(&f))
    {
        return Err(ApiError::BadRequest(
            "rent can't be negative and the fee is 0–50%".into(),
        ));
    }
    Ok(())
}

/// `POST /crm/owner-leads`.
#[rocket_okapi::openapi(tag = "CRM")]
#[post("/crm/owner-leads", data = "<body>")]
pub async fn create_lead(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<OwnerLeadReq>,
) -> ApiResult<Json<OwnerLeadDto>> {
    user.require(Permission::EntityManage)?;
    let b = body.into_inner();
    check_lead(&b)?;
    let name = clean(b.name.clone())
        .ok_or_else(|| ApiError::BadRequest("the lead needs a name".into()))?;
    let now = Utc::now();
    let l = entity::owner_lead::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        name: Set(name),
        company: Set(clean(b.company)),
        email: Set(clean(b.email).map(|e| e.to_lowercase())),
        phone: Set(clean(b.phone)),
        address: Set(clean(b.address)),
        properties_count: Set(b.properties_count.unwrap_or(1)),
        doors: Set(b.doors.unwrap_or(1)),
        source: Set(b.source.unwrap_or_else(|| "website".into())),
        status: Set(b.status.unwrap_or_else(|| "new".into())),
        lost_reason: Set(clean(b.lost_reason)),
        monthly_rent_cents: Set(b.monthly_rent_cents.unwrap_or(0)),
        fee_bps: Set(b.fee_bps),
        notes: Set(clean(b.notes)),
        assigned_to: Set(b.assigned_to.or(Some(user.user_id))),
        owner_id: Set(None),
        won_at: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::OWNER_LEAD_CREATE,
        Some("owner_lead"),
        Some(l.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "source": l.source })),
    )
    .await;
    Ok(Json(
        lead_dtos(&db, scope.tenant_id, vec![l]).await?.remove(0),
    ))
}

async fn find_lead(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    id: &str,
) -> ApiResult<entity::owner_lead::Model> {
    OwnerLead::find_by_id(parse_id(id, "lead")?)
        .filter(entity::owner_lead::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("lead not found".into()))
}

/// `PATCH /crm/owner-leads/<id>` — update, or move through the pipeline (a
/// stage change is written to the lead's timeline).
#[rocket_okapi::openapi(tag = "CRM")]
#[patch("/crm/owner-leads/<id>", data = "<body>")]
pub async fn update_lead(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<OwnerLeadReq>,
) -> ApiResult<Json<OwnerLeadDto>> {
    user.require(Permission::EntityManage)?;
    let b = body.into_inner();
    check_lead(&b)?;
    let l = find_lead(&db, scope.tenant_id, id).await?;
    let old_status = l.status.clone();
    let mut am: entity::owner_lead::ActiveModel = l.into();
    if let Some(v) = clean(b.name) {
        am.name = Set(v);
    }
    if b.company.is_some() {
        am.company = Set(clean(b.company));
    }
    if b.email.is_some() {
        am.email = Set(clean(b.email).map(|e| e.to_lowercase()));
    }
    if b.phone.is_some() {
        am.phone = Set(clean(b.phone));
    }
    if b.address.is_some() {
        am.address = Set(clean(b.address));
    }
    if let Some(v) = b.properties_count {
        am.properties_count = Set(v);
    }
    if let Some(v) = b.doors {
        am.doors = Set(v);
    }
    if let Some(v) = b.source {
        am.source = Set(v);
    }
    if b.lost_reason.is_some() {
        am.lost_reason = Set(clean(b.lost_reason));
    }
    if let Some(v) = b.monthly_rent_cents {
        am.monthly_rent_cents = Set(v);
    }
    if b.fee_bps.is_some() {
        am.fee_bps = Set(b.fee_bps);
    }
    if b.notes.is_some() {
        am.notes = Set(clean(b.notes));
    }
    if b.assigned_to.is_some() {
        am.assigned_to = Set(b.assigned_to);
    }
    let moved = b.status.clone().filter(|s| *s != old_status);
    if let Some(s) = &moved {
        am.status = Set(s.clone());
        if s == "won" {
            am.won_at = Set(Some(Utc::now().into()));
        }
    }
    am.updated_at = Set(Utc::now().into());
    let l = am.update(&db).await?;
    if let Some(s) = moved {
        let now = Utc::now();
        entity::crm_note::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(scope.tenant_id),
            subject_type: Set("owner_lead".into()),
            subject_id: Set(l.id),
            property_id: Set(None),
            kind: Set("update".into()),
            body: Set(format!("Moved from {old_status} to {s}.")),
            pinned: Set(false),
            follow_up_on: Set(None),
            follow_up_done_at: Set(None),
            follow_up_done_by: Set(None),
            author_id: Set(Some(user.user_id)),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&db)
        .await?;
    }
    Ok(Json(
        lead_dtos(&db, scope.tenant_id, vec![l]).await?.remove(0),
    ))
}

/// `POST /crm/owner-leads/<id>/convert` — the lead signed: create the owner,
/// mark the lead won, and carry its timeline over.
#[rocket_okapi::openapi(tag = "CRM")]
#[post("/crm/owner-leads/<id>/convert")]
pub async fn convert_lead(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<OwnerLeadDto>> {
    user.require(Permission::EntityManage)?;
    let l = find_lead(&db, scope.tenant_id, id).await?;
    if l.owner_id.is_some() {
        return Err(ApiError::Conflict("this lead is already an owner".into()));
    }
    let o = insert_owner(
        &db,
        scope.tenant_id,
        OwnerReq {
            name: l.company.clone().unwrap_or_else(|| l.name.clone()),
            kind: Some(if l.company.is_some() {
                "company".into()
            } else {
                "individual".into()
            }),
            email: l.email.clone(),
            phone: l.phone.clone(),
            notes: l.notes.clone(),
        },
    )
    .await?;
    // The history comes along.
    for n in CrmNote::find()
        .filter(entity::crm_note::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::crm_note::Column::SubjectType.eq("owner_lead"))
        .filter(entity::crm_note::Column::SubjectId.eq(l.id))
        .all(&db)
        .await?
    {
        let mut am: entity::crm_note::ActiveModel = n.clone().into();
        am.id = Set(Uuid::new_v4());
        am.subject_type = Set("owner".into());
        am.subject_id = Set(o.id);
        am.insert(&db).await?;
    }
    let mut am: entity::owner_lead::ActiveModel = l.into();
    am.status = Set("won".into());
    am.owner_id = Set(Some(o.id));
    am.won_at = Set(Some(Utc::now().into()));
    am.updated_at = Set(Utc::now().into());
    let l = am.update(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::OWNER_LEAD_CONVERT,
        Some("owner_lead"),
        Some(l.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "owner_id": o.id })),
    )
    .await;
    Ok(Json(
        lead_dtos(&db, scope.tenant_id, vec![l]).await?.remove(0),
    ))
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct StageCount {
    pub status: String,
    pub leads: usize,
    pub doors: i64,
    pub monthly_fee_cents: i64,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct SourceRow {
    pub source: String,
    pub leads: usize,
    pub won: usize,
    pub win_bps: i64,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct PipelineSummary {
    pub stages: Vec<StageCount>,
    pub by_source: Vec<SourceRow>,
    /// Monthly fees weighted by how likely each stage is to close.
    pub weighted_monthly_fee_cents: i64,
    pub win_bps: i64,
    pub follow_ups_due: usize,
}

/// `GET /crm/owner-leads/summary` — the pipeline at a glance.
#[rocket_okapi::openapi(tag = "CRM")]
#[get("/crm/owner-leads/summary")]
pub async fn pipeline(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<PipelineSummary>> {
    user.require(Permission::EntityRead)?;
    let leads = OwnerLead::find()
        .filter(entity::owner_lead::Column::TenantId.eq(scope.tenant_id))
        .all(&db)
        .await?;
    let dtos = lead_dtos(&db, scope.tenant_id, leads).await?;
    let mut stages: BTreeMap<usize, StageCount> = BTreeMap::new();
    for (i, s) in LEAD_STATUSES.iter().enumerate() {
        stages.insert(
            i,
            StageCount {
                status: s.to_string(),
                leads: 0,
                doors: 0,
                monthly_fee_cents: 0,
            },
        );
    }
    let mut sources: BTreeMap<String, SourceRow> = BTreeMap::new();
    let mut weighted = 0;
    for l in &dtos {
        if let Some(i) = LEAD_STATUSES.iter().position(|s| *s == l.status) {
            let st = stages.get_mut(&i).expect("stage");
            st.leads += 1;
            st.doors += l.doors as i64;
            st.monthly_fee_cents += l.monthly_fee_cents;
        }
        if l.status != "won" {
            weighted += crate::workforce::overtime::div_round(
                l.monthly_fee_cents * stage_weight_bps(&l.status),
                10_000,
            );
        }
        let r = sources.entry(l.source.clone()).or_insert(SourceRow {
            source: l.source.clone(),
            leads: 0,
            won: 0,
            win_bps: 0,
        });
        r.leads += 1;
        if l.status == "won" {
            r.won += 1;
        }
    }
    let closed = dtos
        .iter()
        .filter(|l| l.status == "won" || l.status == "lost")
        .count();
    let won = dtos.iter().filter(|l| l.status == "won").count();
    let day = today(&db, scope.tenant_id).await.to_string();
    let due = CrmNote::find()
        .filter(entity::crm_note::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::crm_note::Column::FollowUpDoneAt.is_null())
        .filter(entity::crm_note::Column::FollowUpOn.lte(day))
        .all(&db)
        .await?
        .len();
    Ok(Json(PipelineSummary {
        stages: stages.into_values().collect(),
        by_source: sources
            .into_values()
            .map(|mut r| {
                r.win_bps = crate::workforce::costing::bps(r.won as i64, r.leads as i64);
                r
            })
            .collect(),
        weighted_monthly_fee_cents: weighted,
        win_bps: crate::workforce::costing::bps(won as i64, closed as i64),
        follow_ups_due: due,
    }))
}

/// `GET /crm/owner-leads/<id>/proposal.pdf` — a management proposal to print
/// or send: the portfolio, the fee and what's included.
#[rocket_okapi::openapi(skip)]
#[get("/crm/owner-leads/<id>/proposal.pdf")]
pub async fn proposal(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<ReportFile> {
    user.require(Permission::EntityRead)?;
    let l = find_lead(&db, scope.tenant_id, id).await?;
    let d = lead_dtos(&db, scope.tenant_id, vec![l]).await?.remove(0);
    let theme = Theme::find()
        .filter(entity::theme::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?;
    let org = theme.map(|t| t.company_name).unwrap_or_default();
    let late =
        crate::settings::get_i64(&db, scope.tenant_id, crate::settings::LATE_FEE_GRACE_DAYS).await;
    let who = d.company.clone().unwrap_or_else(|| d.name.clone());
    let doc = Document {
        title: format!("Property management proposal for {who}"),
        subtitle: Some(format!(
            "Prepared by {org} · {}",
            today(&db, scope.tenant_id).await.format("%B %-d, %Y")
        )),
        organization: org.clone(),
        landscape: false,
        blocks: vec![
            Block::Paragraph(format!(
                "Thank you for considering {org} to manage your {}. Here is what we'd take \
                 care of and what it costs — no setup surprises.",
                if d.properties_count == 1 { "property".to_string() } else { format!("{} properties", d.properties_count) }
            )),
            Block::KeyValues(vec![
                ("Owner".into(), who.clone()),
                ("Properties".into(), d.properties_count.to_string()),
                ("Doors".into(), d.doors.to_string()),
                ("Address".into(), d.address.clone().unwrap_or_default()),
                ("Monthly rent roll (estimated)".into(), money(d.monthly_rent_cents)),
                ("Management fee".into(), format!("{} of rent collected", pct(d.fee_bps))),
                ("Estimated monthly fee".into(), money(d.monthly_fee_cents)),
            ]),
            Block::Heading("What's included".into()),
            Block::Table(Table {
                columns: vec![Column::left("Service", 2.0), Column::left("How it works", 5.0)],
                rows: vec![
                    vec!["Leasing".into(), "Listings, showings, online applications, screening (credit, criminal, eviction) and e-signed leases.".into()],
                    vec!["Rent".into(), format!("Card or bank payments with autopay; late fees after {late} days per your lease.")],
                    vec!["Maintenance".into(), "24/7 requests from residents, our own technicians or vetted vendors; you approve anything large.".into()],
                    vec!["Books".into(), "Trust accounting kept separate as the law requires; monthly owner statement and payout.".into()],
                    vec!["Reports".into(), "Rent roll, delinquency, a year of profit and loss, and 1099s every January.".into()],
                    vec!["Your portal".into(), "Statements, payouts, work orders and documents whenever you want them.".into()],
                ],
                totals: None,
            }),
            Block::Paragraph(
                "Maintenance done by our own team is billed at the technician's hourly rate plus \
                 parts, and shows on your statement line by line. Outside vendors bill at cost."
                    .into(),
            ),
        ],
    };
    Ok(ReportFile::new(
        crate::pdfdoc::render(&doc),
        "application/pdf",
        format!(
            "management-proposal-{}.pdf",
            crate::routes::backoffice::reports::slug(&who)
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_weights() {
        assert_eq!(stage_weight_bps("proposal"), 5_000);
        assert_eq!(stage_weight_bps("lost"), 0);
        assert!(stage_weight_bps("new") < stage_weight_bps("contacted"));
    }
}
