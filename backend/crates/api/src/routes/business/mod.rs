//! The **business profile**: how the business shows up to the public (contact
//! details, hours, social links), its Google Business Profile place, and how
//! its Google reviews are displayed. Clients edit it in Settings; Vantedge
//! staff acting in a workspace edit the same record, and those edits are
//! flagged as support changes in the audit trail.

use crate::audit::actions as act;
use crate::audit::change::{self, Ctx};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::geo;
use crate::google_places::{self as places, PlaceCandidate, PlaceInfo, Review};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::{PublicTenant, TenantScope};
use chrono::Utc;
use entity::prelude::BusinessProfile;
use rocket::serde::json::Json;
use rocket::{get, put, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct BusinessDto {
    pub business_name: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub website: Option<String>,
    pub address: Option<String>,
    pub hours: Option<String>,
    pub description: Option<String>,
    pub facebook_url: Option<String>,
    pub instagram_url: Option<String>,
    pub yelp_url: Option<String>,
    pub nextdoor_url: Option<String>,
    pub google_place_id: Option<String>,
    pub google_place_name: Option<String>,
    pub google_review_url: Option<String>,
    /// The link customers use to write a review (the override, else generated).
    pub review_link: Option<String>,
    pub show_reviews: bool,
    pub min_rating: i32,
    pub max_reviews: i32,
    pub refresh_minutes: i32,
    pub embed_enabled: bool,
    /// One `https://host` per line.
    pub embed_origins: Option<String>,
    /// A Google Maps key is stored (the workspace's or the platform's).
    pub google_key_set: bool,
    /// Google is really called; otherwise sample data answers.
    pub google_live: bool,
    pub updated_at: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct BusinessReq {
    pub business_name: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub website: Option<String>,
    pub address: Option<String>,
    pub hours: Option<String>,
    pub description: Option<String>,
    pub facebook_url: Option<String>,
    pub instagram_url: Option<String>,
    pub yelp_url: Option<String>,
    pub nextdoor_url: Option<String>,
    /// Set to pick the business on Google; an empty string clears it.
    pub google_place_id: Option<String>,
    pub google_place_name: Option<String>,
    pub google_review_url: Option<String>,
    pub show_reviews: Option<bool>,
    pub min_rating: Option<i32>,
    pub max_reviews: Option<i32>,
    pub refresh_minutes: Option<i32>,
    pub embed_enabled: Option<bool>,
    /// One `https://host` per line; an empty string clears the list.
    pub embed_origins: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct PublicReviews {
    pub business: String,
    pub rating: Option<f64>,
    pub count: i64,
    pub maps_url: String,
    pub write_url: String,
    pub reviews: Vec<Review>,
}

fn clean(v: &Option<String>) -> Option<String> {
    v.as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn url(label: &str, v: &Option<String>) -> Result<Option<String>, ApiError> {
    match clean(v) {
        None => Ok(None),
        Some(u) if u.starts_with("https://") || u.starts_with("http://") => Ok(Some(u)),
        Some(_) => Err(ApiError::BadRequest(format!(
            "{label} must start with http:// or https://"
        ))),
    }
}

async fn load(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
) -> ApiResult<Option<entity::business_profile::Model>> {
    Ok(BusinessProfile::find()
        .filter(entity::business_profile::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?)
}

async fn dto(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    p: Option<entity::business_profile::Model>,
) -> BusinessDto {
    let key_set = geo::maps_key(db, tenant_id).await.is_some();
    let live = geo::google_live(db, tenant_id).await;
    match p {
        Some(p) => BusinessDto {
            review_link: p
                .google_review_url
                .clone()
                .or_else(|| p.google_place_id.as_deref().map(places::write_review_url)),
            business_name: p.business_name,
            phone: p.phone,
            email: p.email,
            website: p.website,
            address: p.address,
            hours: p.hours,
            description: p.description,
            facebook_url: p.facebook_url,
            instagram_url: p.instagram_url,
            yelp_url: p.yelp_url,
            nextdoor_url: p.nextdoor_url,
            google_place_id: p.google_place_id,
            google_place_name: p.google_place_name,
            google_review_url: p.google_review_url,
            show_reviews: p.show_reviews,
            min_rating: p.min_rating,
            max_reviews: p.max_reviews,
            refresh_minutes: p.refresh_minutes,
            embed_enabled: p.embed_enabled,
            embed_origins: p.embed_origins,
            google_key_set: key_set,
            google_live: live,
            updated_at: Some(p.updated_at.to_rfc3339()),
        },
        None => BusinessDto {
            business_name: None,
            phone: None,
            email: None,
            website: None,
            address: None,
            hours: None,
            description: None,
            facebook_url: None,
            instagram_url: None,
            yelp_url: None,
            nextdoor_url: None,
            google_place_id: None,
            google_place_name: None,
            google_review_url: None,
            review_link: None,
            show_reviews: true,
            min_rating: 4,
            max_reviews: 5,
            refresh_minutes: 360,
            embed_enabled: true,
            embed_origins: None,
            google_key_set: key_set,
            google_live: live,
            updated_at: None,
        },
    }
}

/// `GET /business-profile` — the workspace's business profile.
#[rocket_okapi::openapi(tag = "Business")]
#[get("/business-profile")]
pub async fn get_profile(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<BusinessDto>> {
    user.require(Permission::IntegrationsManage)?;
    let p = load(&db, scope.tenant_id).await?;
    Ok(Json(dto(&db, scope.tenant_id, p).await))
}

/// `PUT /business-profile` — save the profile. Only the fields sent change;
/// an empty string clears a text field. Audited as who changed what.
#[rocket_okapi::openapi(tag = "Business")]
#[put("/business-profile", data = "<body>")]
pub async fn save_profile(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<BusinessReq>,
) -> ApiResult<Json<BusinessDto>> {
    user.require(Permission::IntegrationsManage)?;
    let b = body.into_inner();
    let now = Utc::now();
    let existing = load(&db, scope.tenant_id).await?;
    let was_new = existing.is_none();
    let before = match &existing {
        Some(p) => p.clone(),
        None => entity::business_profile::Model {
            id: Uuid::new_v4(),
            tenant_id: scope.tenant_id,
            business_name: None,
            phone: None,
            email: None,
            website: None,
            address: None,
            hours: None,
            description: None,
            facebook_url: None,
            instagram_url: None,
            yelp_url: None,
            nextdoor_url: None,
            google_place_id: None,
            google_place_name: None,
            google_review_url: None,
            show_reviews: true,
            min_rating: 4,
            max_reviews: 5,
            refresh_minutes: 360,
            embed_enabled: true,
            embed_origins: None,
            updated_by: None,
            created_at: now.into(),
            updated_at: now.into(),
        },
    };
    let mut after = before.clone();
    // Text fields: `None` leaves it, "" clears it.
    macro_rules! text {
        ($f:ident) => {
            if b.$f.is_some() {
                after.$f = clean(&b.$f);
            }
        };
    }
    text!(business_name);
    text!(phone);
    text!(email);
    text!(address);
    text!(hours);
    text!(description);
    macro_rules! link {
        ($f:ident, $label:expr) => {
            if b.$f.is_some() {
                after.$f = url($label, &b.$f)?;
            }
        };
    }
    link!(website, "website");
    link!(facebook_url, "Facebook link");
    link!(instagram_url, "Instagram link");
    link!(yelp_url, "Yelp link");
    link!(nextdoor_url, "Nextdoor link");
    link!(google_review_url, "review link");
    if b.google_place_id.is_some() {
        after.google_place_id = clean(&b.google_place_id);
        after.google_place_name = if after.google_place_id.is_some() {
            clean(&b.google_place_name)
        } else {
            None
        };
    }
    if let Some(v) = b.show_reviews {
        after.show_reviews = v;
    }
    if let Some(v) = b.min_rating {
        if !(1..=5).contains(&v) {
            return Err(ApiError::BadRequest("min_rating must be 1 to 5".into()));
        }
        after.min_rating = v;
    }
    if let Some(v) = b.max_reviews {
        if !(1..=5).contains(&v) {
            return Err(ApiError::BadRequest("max_reviews must be 1 to 5".into()));
        }
        after.max_reviews = v;
    }
    if let Some(v) = b.refresh_minutes {
        if !(5..=1440).contains(&v) {
            return Err(ApiError::BadRequest(
                "refresh_minutes must be 5 to 1440".into(),
            ));
        }
        after.refresh_minutes = v;
    }
    if let Some(v) = b.embed_enabled {
        after.embed_enabled = v;
    }
    if b.embed_origins.is_some() {
        after.embed_origins =
            crate::embed::normalize_origins(b.embed_origins.as_deref().unwrap_or(""))
                .map_err(ApiError::BadRequest)?;
    }
    after.updated_by = Some(user.user_id);
    after.updated_at = now.into();

    let saved = if was_new {
        let mut am: entity::business_profile::ActiveModel = after.clone().into();
        am.id = Set(before.id);
        am.insert(&db).await?
    } else {
        let am: entity::business_profile::ActiveModel = after.clone().into();
        // `updated_by` and the timestamp always differ; the audit diff drops them.
        am.reset_all().update(&db).await?
    };
    if before.google_place_id != saved.google_place_id {
        places::forget(scope.tenant_id);
    }
    change::change(
        &db,
        Ctx::new(&user, &scope),
        act::BUSINESS_PROFILE_SAVE,
        "business_profile",
        saved.id,
        None,
        "Business profile",
        &before,
        &saved,
    )
    .await;
    Ok(Json(dto(&db, scope.tenant_id, Some(saved)).await))
}

/// `GET /business-profile/google/search?q` — find the business on Google.
#[rocket_okapi::openapi(tag = "Business")]
#[get("/business-profile/google/search?<q>")]
pub async fn google_search(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    q: Option<String>,
) -> ApiResult<Json<Vec<PlaceCandidate>>> {
    user.require(Permission::IntegrationsManage)?;
    Ok(Json(
        places::search(&db, scope.tenant_id, q.as_deref().unwrap_or("")).await?,
    ))
}

/// `GET /business-profile/google/place?refresh` — the chosen place's rating
/// and reviews, as Google returns them (every review, before the display rules).
#[rocket_okapi::openapi(tag = "Business")]
#[get("/business-profile/google/place?<refresh>")]
pub async fn google_place(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    refresh: Option<bool>,
) -> ApiResult<Json<PlaceInfo>> {
    user.require(Permission::IntegrationsManage)?;
    let p = load(&db, scope.tenant_id)
        .await?
        .filter(|p| p.google_place_id.is_some())
        .ok_or_else(|| ApiError::NotFound("pick the business on Google first".into()))?;
    Ok(Json(
        places::place(
            &db,
            scope.tenant_id,
            p.google_place_id.as_deref().unwrap_or_default(),
            p.google_place_name.as_deref().unwrap_or_default(),
            p.refresh_minutes,
            refresh.unwrap_or(false),
        )
        .await?,
    ))
}

/// `GET /public/reviews` — the reviews the public website shows: rating,
/// count, the write-a-review link, and reviews filtered by the display rules.
#[rocket_okapi::openapi(tag = "Public Website")]
#[get("/public/reviews")]
pub async fn public_reviews(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    tenant: PublicTenant,
) -> ApiResult<Json<PublicReviews>> {
    let nothing = PublicReviews {
        business: String::new(),
        rating: None,
        count: 0,
        maps_url: String::new(),
        write_url: String::new(),
        reviews: vec![],
    };
    let Some(p) = load(&db, tenant.tenant_id).await? else {
        return Ok(Json(nothing));
    };
    let Some(pid) = p.google_place_id.clone().filter(|_| p.show_reviews) else {
        return Ok(Json(nothing));
    };
    // A failing Google never breaks the public page.
    let Ok(info) = places::place(
        &db,
        tenant.tenant_id,
        &pid,
        p.google_place_name.as_deref().unwrap_or_default(),
        p.refresh_minutes,
        false,
    )
    .await
    else {
        return Ok(Json(nothing));
    };
    Ok(Json(PublicReviews {
        business: p
            .business_name
            .clone()
            .or(p.google_place_name.clone())
            .unwrap_or(info.name),
        rating: info.rating,
        count: info.count,
        maps_url: info.maps_url,
        write_url: p
            .google_review_url
            .clone()
            .unwrap_or_else(|| places::write_review_url(&pid)),
        reviews: info
            .reviews
            .into_iter()
            .filter(|r| r.rating >= p.min_rating && !r.text.is_empty())
            .take(p.max_reviews.max(1) as usize)
            .collect(),
    }))
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct EmbedConfig {
    /// Widgets may be embedded at all.
    pub enabled: bool,
    /// Sites that may show them; empty means any site.
    pub allowed_origins: Vec<String>,
    pub business: String,
}

/// `GET /public/embed-config` — whether this workspace's widgets may be
/// embedded, and where. The widget pages read it to decide whether to show.
#[rocket_okapi::openapi(tag = "Public Website")]
#[get("/public/embed-config")]
pub async fn embed_config(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    tenant: PublicTenant,
) -> ApiResult<Json<EmbedConfig>> {
    let p = load(&db, tenant.tenant_id).await?;
    Ok(Json(match p {
        Some(p) => EmbedConfig {
            enabled: p.embed_enabled,
            allowed_origins: crate::embed::origins(p.embed_origins.as_deref()),
            business: p.business_name.unwrap_or_default(),
        },
        None => EmbedConfig {
            enabled: true,
            allowed_origins: vec![],
            business: String::new(),
        },
    }))
}
