//! **Tour requests**: a prospect asks to see a home from the public site;
//! the leasing team triages them in the console.

use crate::audit::actions as act;
use crate::audit::change::{self, Ctx};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::{PublicTenant, TenantScope};
use chrono::Utc;
use entity::prelude::{Listing, TourRequest};
use rocket::serde::json::Json;
use rocket::{get, patch, post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const STATUSES: &[&str] = &["new", "contacted", "scheduled", "closed"];

#[derive(Deserialize, schemars::JsonSchema)]
pub struct TourReq {
    pub listing_id: Option<Uuid>,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    pub preferred_times: Option<String>,
    pub message: Option<String>,
    /// The prospect agrees to be contacted about this request.
    pub consent: bool,
    /// A honeypot: real visitors never fill it in.
    pub website: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct TourAck {
    pub ok: bool,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct TourDto {
    pub id: Uuid,
    pub listing_id: Option<Uuid>,
    pub listing_title: Option<String>,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    pub preferred_times: Option<String>,
    pub message: Option<String>,
    pub status: String,
    pub note: Option<String>,
    pub created_at: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateTourReq {
    pub status: Option<String>,
    pub note: Option<String>,
}

fn clean(v: Option<String>, max: usize) -> Option<String> {
    v.map(|s| s.trim().chars().take(max).collect::<String>())
        .filter(|s| !s.is_empty())
}

fn plausible_email(e: &str) -> bool {
    let mut parts = e.split('@');
    matches!((parts.next(), parts.next(), parts.next()),
        (Some(a), Some(d), None) if !a.is_empty() && d.contains('.') && !d.starts_with('.') && !d.ends_with('.'))
        && !e.contains(char::is_whitespace)
}

/// `POST /public/tour-requests` — ask to tour a home.
#[rocket_okapi::openapi(tag = "Public Website")]
#[post("/public/tour-requests", data = "<body>")]
pub async fn request_tour(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    tenant: PublicTenant,
    body: Json<TourReq>,
) -> ApiResult<Json<TourAck>> {
    let b = body.into_inner();
    // The honeypot: a bot filled the hidden field. Say yes, store nothing.
    if b.website.as_deref().is_some_and(|w| !w.trim().is_empty()) {
        return Ok(Json(TourAck { ok: true }));
    }
    let name =
        clean(Some(b.name), 120).ok_or_else(|| ApiError::BadRequest("name is required".into()))?;
    let email = b.email.trim().to_lowercase();
    if !plausible_email(&email) {
        return Err(ApiError::BadRequest("enter a valid email".into()));
    }
    if !b.consent {
        return Err(ApiError::BadRequest(
            "please agree to be contacted about this request".into(),
        ));
    }
    let listing = match b.listing_id {
        Some(id) => Some(
            Listing::find_by_id(id)
                .filter(entity::listing::Column::TenantId.eq(tenant.tenant_id))
                .filter(entity::listing::Column::IsPublic.eq(true))
                .one(&db)
                .await?
                .ok_or_else(|| ApiError::NotFound("listing not found".into()))?,
        ),
        None => None,
    };
    let now = Utc::now();
    let saved = entity::tour_request::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant.tenant_id),
        listing_id: Set(listing.as_ref().map(|l| l.id)),
        name: Set(name.clone()),
        email: Set(email.clone()),
        phone: Set(clean(b.phone, 40)),
        preferred_times: Set(clean(b.preferred_times, 300)),
        message: Set(clean(b.message, 2000)),
        consent: Set(true),
        status: Set("new".into()),
        note: Set(None),
        handled_by: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;
    crate::notify::notify_staff(
        &db,
        tenant.tenant_id,
        "application:read",
        "tour_requested",
        serde_json::json!({
            "name": name,
            "home": listing.as_ref().map(|l| l.title.clone()).unwrap_or_else(|| "a home".into()),
            "times": saved.preferred_times.clone().unwrap_or_else(|| "none given".into()),
            "contact": saved.phone.clone().unwrap_or(email),
        }),
        Some(("tour_request", saved.id)),
        "created",
        None,
    )
    .await;
    Ok(Json(TourAck { ok: true }))
}

/// `GET /tour-requests?status` — tour requests, newest first.
#[rocket_okapi::openapi(tag = "Leasing")]
#[get("/tour-requests?<status>")]
pub async fn list_tours(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    status: Option<String>,
) -> ApiResult<Json<Vec<TourDto>>> {
    user.require(Permission::ApplicationRead)?;
    let mut q =
        TourRequest::find().filter(entity::tour_request::Column::TenantId.eq(scope.tenant_id));
    if let Some(s) = status.filter(|s| !s.trim().is_empty()) {
        q = q.filter(entity::tour_request::Column::Status.eq(s));
    }
    let rows = q
        .order_by_desc(entity::tour_request::Column::CreatedAt)
        .all(&db)
        .await?;
    let ids: Vec<Uuid> = rows.iter().filter_map(|r| r.listing_id).collect();
    let titles: std::collections::HashMap<Uuid, String> = Listing::find()
        .filter(entity::listing::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::listing::Column::Id.is_in(ids))
        .all(&db)
        .await?
        .into_iter()
        .map(|l| (l.id, l.title))
        .collect();
    Ok(Json(
        rows.into_iter()
            .map(|r| TourDto {
                listing_title: r.listing_id.and_then(|i| titles.get(&i).cloned()),
                id: r.id,
                listing_id: r.listing_id,
                name: r.name,
                email: r.email,
                phone: r.phone,
                preferred_times: r.preferred_times,
                message: r.message,
                status: r.status,
                note: r.note,
                created_at: r.created_at.to_rfc3339(),
            })
            .collect(),
    ))
}

/// `PATCH /tour-requests/<id>` — move a request along, with a note.
#[rocket_okapi::openapi(tag = "Leasing")]
#[patch("/tour-requests/<id>", data = "<body>")]
pub async fn update_tour(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateTourReq>,
) -> ApiResult<Json<TourAck>> {
    user.require(Permission::ApplicationWrite)?;
    let id = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let existing = TourRequest::find_by_id(id)
        .filter(entity::tour_request::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("tour request not found".into()))?;
    let b = body.into_inner();
    let before = existing.clone();
    let mut am: entity::tour_request::ActiveModel = existing.into();
    if let Some(s) = b.status {
        if !STATUSES.contains(&s.as_str()) {
            return Err(ApiError::BadRequest(format!(
                "status must be one of {}",
                STATUSES.join(", ")
            )));
        }
        am.status = Set(s);
    }
    if b.note.is_some() {
        am.note = Set(clean(b.note, 2000));
    }
    am.handled_by = Set(Some(user.user_id));
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&db).await?;
    change::change(
        &db,
        Ctx::new(&user, &scope),
        act::TOUR_REQUEST_UPDATE,
        "tour_request",
        saved.id,
        None,
        &format!("Tour request from {}", saved.name),
        &before,
        &saved,
    )
    .await;
    Ok(Json(TourAck { ok: true }))
}
