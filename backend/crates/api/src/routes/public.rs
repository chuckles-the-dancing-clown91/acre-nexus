//! Public website endpoints — no authentication. The tenant is resolved from the
//! `X-Tenant` header or `?tenant=<slug>` so the same API powers every client's
//! white-label site (or an embedded iframe).

use crate::dto::usd;
use crate::error::{ApiError, ApiResult};
use crate::scheduler;
use crate::state::AppState;
use crate::tenancy::PublicTenant;
use chrono::Utc;
use entity::prelude::{Listing, Theme};
use rocket::serde::json::Json;
use rocket::{get, post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct ListingResp {
    pub id: Uuid,
    pub title: String,
    pub address: String,
    pub city: String,
    pub beds: i32,
    pub baths: i32,
    pub sqft: i32,
    pub rent_cents: i64,
    pub rent_label: String,
    pub status: String,
    pub available_on: String,
    pub description: String,
}

impl From<entity::listing::Model> for ListingResp {
    fn from(l: entity::listing::Model) -> Self {
        ListingResp {
            rent_label: usd(l.rent_cents),
            id: l.id,
            title: l.title,
            address: l.address,
            city: l.city,
            beds: l.beds,
            baths: l.baths,
            sqft: l.sqft,
            rent_cents: l.rent_cents,
            status: l.status,
            available_on: l.available_on,
            description: l.description,
        }
    }
}

/// `GET /public/listings` — public, available listings for a tenant.
#[rocket_okapi::openapi(tag = "Public Website")]
#[get("/public/listings")]
pub async fn listings(
    state: &State<AppState>,
    tenant: PublicTenant,
) -> ApiResult<Json<Vec<ListingResp>>> {
    let rows = Listing::find()
        .filter(entity::listing::Column::TenantId.eq(tenant.tenant_id))
        .filter(entity::listing::Column::IsPublic.eq(true))
        .order_by_desc(entity::listing::Column::CreatedAt)
        .all(&state.db)
        .await?;
    Ok(Json(rows.into_iter().map(ListingResp::from).collect()))
}

/// `GET /public/listings/<id>` — a single public listing.
#[rocket_okapi::openapi(tag = "Public Website")]
#[get("/public/listings/<id>")]
pub async fn listing_detail(
    state: &State<AppState>,
    tenant: PublicTenant,
    id: &str,
) -> ApiResult<Json<ListingResp>> {
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let l = Listing::find_by_id(lid)
        .filter(entity::listing::Column::TenantId.eq(tenant.tenant_id))
        .filter(entity::listing::Column::IsPublic.eq(true))
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("listing not found".into()))?;
    Ok(Json(ListingResp::from(l)))
}

/// Public branding so a white-label site can theme itself before login.
#[derive(Serialize, schemars::JsonSchema)]
pub struct PublicTheme {
    pub company_name: String,
    pub logo_url: Option<String>,
    pub primary_color: String,
    pub accent_color: String,
    pub default_mode: String,
}

/// `GET /public/theme` — branding for the resolved tenant.
#[rocket_okapi::openapi(tag = "Public Website")]
#[get("/public/theme")]
pub async fn public_theme(
    state: &State<AppState>,
    tenant: PublicTenant,
) -> ApiResult<Json<PublicTheme>> {
    let t = Theme::find()
        .filter(entity::theme::Column::TenantId.eq(tenant.tenant_id))
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("theme not configured".into()))?;
    Ok(Json(PublicTheme {
        company_name: t.company_name,
        logo_url: t.logo_url,
        primary_color: t.primary_color,
        accent_color: t.accent_color,
        default_mode: t.default_mode,
    }))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ApplyReq {
    pub listing_id: Option<Uuid>,
    pub applicant_name: String,
    pub email: String,
    pub phone: Option<String>,
    pub annual_income_cents: Option<i64>,
    pub credit_score: Option<i32>,
    pub move_in: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ApplyResp {
    pub application_id: Uuid,
    pub status: String,
    /// Id of the enqueued background-screening job (Tokio scheduler).
    pub screening_job_id: Uuid,
    pub message: String,
}

/// `POST /public/applications` — submit a rental application.
///
/// Persists the application and enqueues a background-screening job that the
/// Tokio scheduler advances asynchronously (submit → await callback → completed).
#[rocket_okapi::openapi(tag = "Public Website")]
#[post("/public/applications", data = "<body>")]
pub async fn apply(
    state: &State<AppState>,
    tenant: PublicTenant,
    body: Json<ApplyReq>,
) -> ApiResult<Json<ApplyResp>> {
    let b = body.into_inner();
    let app_id = Uuid::new_v4();
    let model = entity::application::ActiveModel {
        id: Set(app_id),
        tenant_id: Set(tenant.tenant_id),
        listing_id: Set(b.listing_id),
        applicant_name: Set(b.applicant_name.clone()),
        email: Set(b.email),
        phone: Set(b.phone.unwrap_or_default()),
        annual_income_cents: Set(b.annual_income_cents.unwrap_or(0)),
        credit_score: Set(b.credit_score),
        status: Set("Screening".into()),
        move_in: Set(b.move_in.unwrap_or_default()),
        created_at: Set(Utc::now().into()),
    };
    model.insert(&state.db).await?;

    crate::audit::record(
        &state.db,
        None,
        crate::audit::actions::APPLICATION_SUBMIT,
        Some("application"),
        Some(app_id.to_string()),
        Some(tenant.tenant_id),
        Some(serde_json::json!({ "applicant": b.applicant_name })),
    )
    .await;

    let job_id = scheduler::enqueue(
        &state.db,
        tenant.tenant_id,
        "background_check",
        json!({ "application_id": app_id, "applicant": b.applicant_name }),
        0,
    )
    .await?;

    Ok(Json(ApplyResp {
        application_id: app_id,
        status: "Screening".into(),
        screening_job_id: job_id,
        message: "Application received — screening in progress".into(),
    }))
}
