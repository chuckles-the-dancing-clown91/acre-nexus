//! **Campground reservations** (roadmap area 8): the sites drawn on a
//! campground's site map become rentable.
//!
//! Staff (`property:read` to look, `property:write` to change): booking rules
//! and seasons per map, an availability grid, quotes and stays, a check-in
//! board (arriving, departing, in house, to clean, requests), and stay
//! actions (confirm, check in, check out, cancel, cleaned, payment). Public,
//! for a published map with booking open: the sites and which are free for
//! some dates, a quote, a stay request (held until staff confirm) and the
//! guest's own page by link, where a stay can be cancelled.

use crate::auth::AuthUser;
use crate::campground::{self as cg, Addon, AddonLine, Quote, Rates, Season, HOLDING};
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::tenancy::{Access, PublicTenant, TenantScope};
use chrono::{NaiveDate, Utc};
use entity::prelude::{CampgroundConfig, CampgroundSeason, Property, SiteFeature, SiteMap, Stay};
use rocket::serde::json::Json;
use rocket::{delete, get, patch, post, put};
use schemars::JsonSchema;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
    Statement,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const CAMP_KINDS: &[&str] = &["campground", "rv_park"];

fn date(s: &str, what: &str) -> ApiResult<NaiveDate> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d")
        .map_err(|_| ApiError::BadRequest(format!("{what} must be YYYY-MM-DD")))
}

fn uuid(s: &str, what: &str) -> ApiResult<Uuid> {
    Uuid::parse_str(s).map_err(|_| ApiError::NotFound(format!("{what} not found")))
}

// ---- dto -------------------------------------------------------------------

#[derive(Serialize, JsonSchema, Clone)]
pub struct ConfigDto {
    pub booking_open: bool,
    pub deposit_pct: i32,
    pub check_in_time: String,
    pub check_out_time: String,
    pub max_nights: i32,
    pub addons: Vec<Addon>,
    pub policies: Option<String>,
}

#[derive(Serialize, JsonSchema, Clone)]
pub struct SeasonDto {
    pub id: Uuid,
    pub name: String,
    pub start_md: String,
    pub end_md: String,
    pub adjust_pct: i32,
    pub min_nights: i32,
}

#[derive(Serialize, JsonSchema, Clone)]
pub struct SiteDto {
    pub id: Uuid,
    pub name: String,
    pub site_type: Option<String>,
    pub max_length_ft: Option<i64>,
    pub max_guests: Option<i64>,
    pub power_amps: Option<i64>,
    pub water: bool,
    pub sewer: bool,
    pub pull_through: bool,
    pub pets: bool,
    pub rate_cents_night: Option<i64>,
    pub rate_cents_week: Option<i64>,
    pub rate_cents_month: Option<i64>,
    /// Blocked or out of service on the map.
    pub closed: bool,
}

#[derive(Serialize, JsonSchema, Clone)]
pub struct StayDto {
    pub id: Uuid,
    pub map_id: Uuid,
    pub site_id: Uuid,
    pub site_name: String,
    pub guest_name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub check_in: String,
    pub check_out: String,
    pub nights: i64,
    pub guests: i32,
    pub vehicle: Option<String>,
    pub rig_length_ft: Option<i32>,
    pub addons: Vec<AddonLine>,
    pub total_cents: i64,
    pub deposit_cents: i64,
    pub paid_cents: i64,
    pub balance_cents: i64,
    pub status: String,
    pub source: String,
    pub note: Option<String>,
    pub checked_in_at: Option<String>,
    pub checked_out_at: Option<String>,
    pub cleaned_at: Option<String>,
}

impl From<entity::stay::Model> for StayDto {
    fn from(s: entity::stay::Model) -> Self {
        StayDto {
            nights: (s.check_out - s.check_in).num_days(),
            balance_cents: s.total_cents - s.paid_cents,
            addons: serde_json::from_value(s.addons.clone()).unwrap_or_default(),
            id: s.id,
            map_id: s.map_id,
            site_id: s.site_id,
            site_name: s.site_name,
            guest_name: s.guest_name,
            email: s.email,
            phone: s.phone,
            check_in: s.check_in.to_string(),
            check_out: s.check_out.to_string(),
            guests: s.guests,
            vehicle: s.vehicle,
            rig_length_ft: s.rig_length_ft,
            total_cents: s.total_cents,
            deposit_cents: s.deposit_cents,
            paid_cents: s.paid_cents,
            status: s.status,
            source: s.source,
            note: s.note,
            checked_in_at: s.checked_in_at.map(|d| d.to_rfc3339()),
            checked_out_at: s.checked_out_at.map(|d| d.to_rfc3339()),
            cleaned_at: s.cleaned_at.map(|d| d.to_rfc3339()),
        }
    }
}

fn site_dto(f: &entity::site_feature::Model) -> SiteDto {
    let a = &f.attrs;
    let n = |k: &str| a[k].as_f64().map(|v| v as i64);
    let b = |k: &str| a[k].as_bool().unwrap_or(false);
    let r = Rates::from_attrs(a);
    SiteDto {
        id: f.id,
        name: f.name.clone().unwrap_or_else(|| "Site".into()),
        site_type: a["site_type"].as_str().map(str::to_string),
        max_length_ft: n("max_length_ft"),
        max_guests: n("max_guests"),
        power_amps: n("power_amps"),
        water: b("water"),
        sewer: b("sewer"),
        pull_through: b("pull_through"),
        pets: b("pets"),
        rate_cents_night: r.night,
        rate_cents_week: r.week,
        rate_cents_month: r.month,
        closed: matches!(
            a["site_status"].as_str(),
            Some("blocked" | "out_of_service")
        ),
    }
}

// ---- loading ---------------------------------------------------------------

struct Camp {
    map: entity::site_map::Model,
    config: ConfigDto,
    seasons: Vec<entity::campground_season::Model>,
    sites: Vec<entity::site_feature::Model>,
}

fn default_config() -> ConfigDto {
    ConfigDto {
        booking_open: false,
        deposit_pct: 25,
        check_in_time: "15:00".into(),
        check_out_time: "11:00".into(),
        max_nights: 180,
        addons: vec![],
        policies: None,
    }
}

async fn load(db: &impl ConnectionTrait, tenant_id: Uuid, map_id: Uuid) -> ApiResult<Camp> {
    let map = SiteMap::find_by_id(map_id)
        .one(db)
        .await?
        .filter(|m| m.tenant_id == tenant_id && CAMP_KINDS.contains(&m.kind.as_str()))
        .ok_or_else(|| ApiError::NotFound("campground not found".into()))?;
    let config = match CampgroundConfig::find_by_id(map_id).one(db).await? {
        Some(c) => ConfigDto {
            booking_open: c.booking_open,
            deposit_pct: c.deposit_pct,
            check_in_time: c.check_in_time,
            check_out_time: c.check_out_time,
            max_nights: c.max_nights,
            addons: serde_json::from_value(c.addons).unwrap_or_default(),
            policies: c.policies,
        },
        None => default_config(),
    };
    let seasons = CampgroundSeason::find()
        .filter(entity::campground_season::Column::MapId.eq(map_id))
        .order_by_asc(entity::campground_season::Column::StartMd)
        .all(db)
        .await?;
    let sites = SiteFeature::find()
        .filter(entity::site_feature::Column::MapId.eq(map_id))
        .filter(entity::site_feature::Column::Kind.eq("site"))
        .order_by_asc(entity::site_feature::Column::Position)
        .all(db)
        .await?;
    Ok(Camp {
        map,
        config,
        seasons,
        sites,
    })
}

fn seasons_of(c: &Camp) -> Vec<Season> {
    c.seasons
        .iter()
        .map(|s| Season {
            name: s.name.clone(),
            start: s.start_md.clone(),
            end: s.end_md.clone(),
            adjust_pct: s.adjust_pct,
            min_nights: s.min_nights,
        })
        .collect()
}

async fn holding_stays(
    db: &impl ConnectionTrait,
    map_id: Uuid,
    from: NaiveDate,
    to: NaiveDate,
) -> ApiResult<Vec<entity::stay::Model>> {
    Ok(Stay::find()
        .filter(entity::stay::Column::MapId.eq(map_id))
        .filter(entity::stay::Column::Status.is_in(HOLDING.to_vec()))
        .filter(entity::stay::Column::CheckIn.lt(to))
        .filter(entity::stay::Column::CheckOut.gt(from))
        .all(db)
        .await?)
}

// ---- booking core ------------------------------------------------------------

#[derive(Deserialize, JsonSchema, Clone)]
pub struct QuoteReq {
    pub site_id: Uuid,
    pub check_in: String,
    pub check_out: String,
    /// `[[key, qty]]`
    #[serde(default)]
    pub addons: Vec<(String, i32)>,
}

fn price(
    c: &Camp,
    site: &entity::site_feature::Model,
    q: &QuoteReq,
) -> ApiResult<(NaiveDate, NaiveDate, Quote)> {
    let ci = date(&q.check_in, "check_in")?;
    let co = date(&q.check_out, "check_out")?;
    let quote = cg::quote(
        ci,
        co,
        &Rates::from_attrs(&site.attrs),
        &seasons_of(c),
        &c.config.addons,
        &q.addons,
        c.config.deposit_pct,
        c.config.max_nights,
    )
    .map_err(|e| ApiError::BadRequest(e.message()))?;
    Ok((ci, co, quote))
}

fn site_of(c: &Camp, id: Uuid) -> ApiResult<&entity::site_feature::Model> {
    c.sites
        .iter()
        .find(|s| s.id == id)
        .ok_or_else(|| ApiError::NotFound("site not found".into()))
}

#[derive(Deserialize, JsonSchema, Clone)]
pub struct StayReq {
    #[serde(flatten)]
    pub quote: QuoteReq,
    pub guest_name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub guests: Option<i32>,
    pub vehicle: Option<String>,
    pub rig_length_ft: Option<i32>,
    pub note: Option<String>,
    /// A honeypot (public form only).
    pub website: Option<String>,
}

#[allow(clippy::too_many_arguments)]
/// Book a site: fits the site, priced, and free (checked under a lock on the
/// site, so two people can't take the same nights).
async fn book(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    c: &Camp,
    b: &StayReq,
    status: &str,
    source: &str,
    created_by: Option<Uuid>,
    token_hash: Option<String>,
) -> ApiResult<entity::stay::Model> {
    let name = b.guest_name.trim();
    if name.is_empty() || name.chars().count() > 120 {
        return Err(ApiError::BadRequest("a guest name is required".into()));
    }
    let email = b
        .email
        .as_deref()
        .map(|e| e.trim().to_lowercase())
        .filter(|e| !e.is_empty());
    if let Some(e) = &email {
        if !e.contains('@') || e.len() > 200 {
            return Err(ApiError::BadRequest("that email doesn't look right".into()));
        }
    }
    let site = site_of(c, b.quote.site_id)?;
    let s = site_dto(site);
    if s.closed {
        return Err(ApiError::Conflict(format!("{} is closed", s.name)));
    }
    let guests = b.guests.unwrap_or(1).clamp(1, 100);
    if let Some(max) = s.max_guests.filter(|m| *m > 0) {
        if guests as i64 > max {
            return Err(ApiError::BadRequest(format!(
                "{} fits {max} guests",
                s.name
            )));
        }
    }
    if let (Some(len), Some(max)) = (b.rig_length_ft, s.max_length_ft.filter(|m| *m > 0)) {
        if len as i64 > max {
            return Err(ApiError::BadRequest(format!(
                "{} fits rigs up to {max} ft",
                s.name
            )));
        }
    }
    let (ci, co, q) = price(c, site, &b.quote)?;
    if source == "public" && ci < Utc::now().date_naive() {
        return Err(ApiError::BadRequest("check-in can't be in the past".into()));
    }
    db.execute(Statement::from_sql_and_values(
        sea_orm::DbBackend::Postgres,
        "SELECT pg_advisory_xact_lock(hashtext($1))",
        [site.id.to_string().into()],
    ))
    .await?;
    let clash = holding_stays(db, c.map.id, ci, co)
        .await?
        .into_iter()
        .any(|x| x.site_id == site.id);
    if clash {
        return Err(ApiError::Conflict(format!(
            "{} is taken for some of those nights",
            s.name
        )));
    }
    let now = Utc::now();
    Ok(entity::stay::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        map_id: Set(c.map.id),
        site_id: Set(site.id),
        site_name: Set(s.name),
        guest_name: Set(name.to_string()),
        email: Set(email),
        phone: Set(b
            .phone
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(str::to_string)),
        check_in: Set(ci),
        check_out: Set(co),
        guests: Set(guests),
        vehicle: Set(b
            .vehicle
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(|v| v.chars().take(120).collect())),
        rig_length_ft: Set(b.rig_length_ft),
        addons: Set(serde_json::to_value(&q.addons).unwrap_or_default()),
        total_cents: Set(q.total_cents),
        deposit_cents: Set(q.deposit_cents),
        paid_cents: Set(0),
        status: Set(status.into()),
        source: Set(source.into()),
        note: Set(b
            .note
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(|v| v.chars().take(1000).collect())),
        token_hash: Set(token_hash),
        checked_in_at: Set(None),
        checked_out_at: Set(None),
        cleaned_at: Set(None),
        created_by: Set(created_by),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?)
}

fn guest_vars(s: &entity::stay::Model, camp: &str, link: Option<String>) -> serde_json::Value {
    serde_json::json!({
        "name": s.guest_name,
        "campground": camp,
        "site": s.site_name,
        "check_in": s.check_in.to_string(),
        "check_out": s.check_out.to_string(),
        "total": crate::owner_approvals::money(s.total_cents),
        "deposit": crate::owner_approvals::money(s.deposit_cents),
        "link": link.unwrap_or_default(),
    })
}

fn stay_link(token: &str) -> String {
    format!("{}/stay/{token}", crate::oauth::public_app_url())
}

// ---- staff routes ------------------------------------------------------------

#[derive(Serialize, JsonSchema)]
pub struct CampSummary {
    pub map_id: Uuid,
    pub name: String,
    pub property_id: Uuid,
    pub property_name: String,
    pub sites: usize,
    pub published: bool,
    pub booking_open: bool,
    pub in_house: usize,
    pub arriving_today: usize,
}

/// `GET /campgrounds` — the campground and RV park maps in reach.
#[rocket_okapi::openapi(tag = "Campgrounds")]
#[get("/campgrounds")]
pub async fn list(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
) -> ApiResult<Json<Vec<CampSummary>>> {
    user.require(Permission::PropertyRead)?;
    let today = Utc::now().date_naive();
    let maps = SiteMap::find()
        .filter(entity::site_map::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::site_map::Column::Kind.is_in(CAMP_KINDS.to_vec()))
        .all(&db)
        .await?;
    let mut out = Vec::new();
    for m in maps.into_iter().filter(|m| access.sees(m.property_id)) {
        let c = load(&db, scope.tenant_id, m.id).await?;
        let stays = holding_stays(&db, m.id, today, today + chrono::Duration::days(1)).await?;
        let prop = Property::find_by_id(m.property_id)
            .one(&db)
            .await?
            .map(|p| p.name)
            .unwrap_or_default();
        out.push(CampSummary {
            map_id: m.id,
            name: m.name.clone(),
            property_id: m.property_id,
            property_name: prop,
            sites: c.sites.len(),
            published: m.published,
            booking_open: c.config.booking_open,
            in_house: stays.iter().filter(|s| s.status == "checked_in").count(),
            arriving_today: stays
                .iter()
                .filter(|s| s.check_in == today && s.status != "checked_in")
                .count(),
        });
    }
    Ok(Json(out))
}

#[derive(Serialize, JsonSchema)]
pub struct CampDetail {
    pub map_id: Uuid,
    pub name: String,
    pub property_id: Uuid,
    pub published: bool,
    pub config: ConfigDto,
    pub seasons: Vec<SeasonDto>,
    pub sites: Vec<SiteDto>,
}

fn detail_of(c: &Camp) -> CampDetail {
    CampDetail {
        map_id: c.map.id,
        name: c.map.name.clone(),
        property_id: c.map.property_id,
        published: c.map.published,
        config: c.config.clone(),
        seasons: c
            .seasons
            .iter()
            .map(|s| SeasonDto {
                id: s.id,
                name: s.name.clone(),
                start_md: s.start_md.clone(),
                end_md: s.end_md.clone(),
                adjust_pct: s.adjust_pct,
                min_nights: s.min_nights,
            })
            .collect(),
        sites: c.sites.iter().map(site_dto).collect(),
    }
}

async fn staff_camp(
    db: &crate::db::RequestDb,
    user: &AuthUser,
    scope: &TenantScope,
    access: &Access,
    id: &str,
    write: bool,
) -> ApiResult<Camp> {
    user.require(if write {
        Permission::PropertyWrite
    } else {
        Permission::PropertyRead
    })?;
    let c = load(db, scope.tenant_id, uuid(id, "campground")?).await?;
    if !access.sees(c.map.property_id) {
        return Err(ApiError::NotFound("campground not found".into()));
    }
    Ok(c)
}

/// `GET /campgrounds/<id>` — rules, seasons and sites.
#[rocket_okapi::openapi(tag = "Campgrounds")]
#[get("/campgrounds/<id>")]
pub async fn detail(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    id: &str,
) -> ApiResult<Json<CampDetail>> {
    let c = staff_camp(&db, &user, &scope, &access, id, false).await?;
    Ok(Json(detail_of(&c)))
}

#[derive(Deserialize, JsonSchema)]
pub struct ConfigReq {
    pub booking_open: bool,
    pub deposit_pct: i32,
    pub check_in_time: String,
    pub check_out_time: String,
    pub max_nights: i32,
    pub addons: Vec<Addon>,
    pub policies: Option<String>,
}

fn hhmm(s: &str) -> bool {
    chrono::NaiveTime::parse_from_str(s, "%H:%M").is_ok()
}

/// `PUT /campgrounds/<id>/config` — booking rules and add-ons.
#[rocket_okapi::openapi(tag = "Campgrounds")]
#[put("/campgrounds/<id>/config", data = "<body>")]
pub async fn set_config(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    id: &str,
    body: Json<ConfigReq>,
) -> ApiResult<Json<CampDetail>> {
    let c = staff_camp(&db, &user, &scope, &access, id, true).await?;
    let b = body.into_inner();
    if !(0..=100).contains(&b.deposit_pct) || !(1..=366).contains(&b.max_nights) {
        return Err(ApiError::BadRequest(
            "deposit is 0 to 100 percent; stays 1 to 366 nights".into(),
        ));
    }
    if !hhmm(&b.check_in_time) || !hhmm(&b.check_out_time) {
        return Err(ApiError::BadRequest("times are HH:MM".into()));
    }
    let mut keys = std::collections::HashSet::new();
    for a in &b.addons {
        if a.key.trim().is_empty()
            || a.label.trim().is_empty()
            || a.price_cents < 0
            || !["stay", "night"].contains(&a.per.as_str())
            || !keys.insert(a.key.clone())
        {
            return Err(ApiError::BadRequest(
                "each add-on needs a unique key, a label, a price, and per stay or per night"
                    .into(),
            ));
        }
    }
    let row = entity::campground_config::ActiveModel {
        map_id: Set(c.map.id),
        tenant_id: Set(scope.tenant_id),
        booking_open: Set(b.booking_open),
        deposit_pct: Set(b.deposit_pct),
        check_in_time: Set(b.check_in_time),
        check_out_time: Set(b.check_out_time),
        max_nights: Set(b.max_nights),
        addons: Set(serde_json::to_value(&b.addons).unwrap_or_default()),
        policies: Set(b.policies.map(|p| p.chars().take(4000).collect())),
        updated_at: Set(Utc::now().into()),
    };
    if CampgroundConfig::find_by_id(c.map.id)
        .one(&db)
        .await?
        .is_some()
    {
        row.update(&db).await?;
    } else {
        row.insert(&db).await?;
    }
    let c = load(&db, scope.tenant_id, c.map.id).await?;
    Ok(Json(detail_of(&c)))
}

#[derive(Deserialize, JsonSchema)]
pub struct SeasonReq {
    pub name: String,
    pub start_md: String,
    pub end_md: String,
    pub adjust_pct: i32,
    pub min_nights: i32,
}

/// `POST /campgrounds/<id>/seasons`
#[rocket_okapi::openapi(tag = "Campgrounds")]
#[post("/campgrounds/<id>/seasons", data = "<body>")]
pub async fn add_season(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    id: &str,
    body: Json<SeasonReq>,
) -> ApiResult<Json<CampDetail>> {
    let c = staff_camp(&db, &user, &scope, &access, id, true).await?;
    let b = body.into_inner();
    if b.name.trim().is_empty() || !cg::valid_md(&b.start_md) || !cg::valid_md(&b.end_md) {
        return Err(ApiError::BadRequest(
            "a season needs a name and MM-DD dates".into(),
        ));
    }
    if !(-90..=500).contains(&b.adjust_pct) || !(1..=60).contains(&b.min_nights) {
        return Err(ApiError::BadRequest(
            "adjustment is -90 to 500 percent; minimum stay 1 to 60 nights".into(),
        ));
    }
    entity::campground_season::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        map_id: Set(c.map.id),
        name: Set(b.name.trim().into()),
        start_md: Set(b.start_md),
        end_md: Set(b.end_md),
        adjust_pct: Set(b.adjust_pct),
        min_nights: Set(b.min_nights),
        created_at: Set(Utc::now().into()),
    }
    .insert(&db)
    .await?;
    let c = load(&db, scope.tenant_id, c.map.id).await?;
    Ok(Json(detail_of(&c)))
}

/// `DELETE /campground-seasons/<id>`
#[rocket_okapi::openapi(tag = "Campgrounds")]
#[delete("/campground-seasons/<id>")]
pub async fn delete_season(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::PropertyWrite)?;
    let s = CampgroundSeason::find_by_id(uuid(id, "season")?)
        .one(&db)
        .await?
        .filter(|s| s.tenant_id == scope.tenant_id)
        .ok_or_else(|| ApiError::NotFound("season not found".into()))?;
    CampgroundSeason::delete_by_id(s.id).exec(&db).await?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}

#[derive(Serialize, JsonSchema)]
pub struct AvailabilityRow {
    pub site: SiteDto,
    pub stays: Vec<StayDto>,
    pub free: bool,
}

/// `GET /campgrounds/<id>/availability?from&to` — each site and its stays in
/// the window.
#[rocket_okapi::openapi(tag = "Campgrounds")]
#[get("/campgrounds/<id>/availability?<from>&<to>")]
pub async fn availability(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    id: &str,
    from: &str,
    to: &str,
) -> ApiResult<Json<Vec<AvailabilityRow>>> {
    let c = staff_camp(&db, &user, &scope, &access, id, false).await?;
    let (f, t) = (date(from, "from")?, date(to, "to")?);
    if t <= f || (t - f).num_days() > 92 {
        return Err(ApiError::BadRequest("pick a window of 1 to 92 days".into()));
    }
    let stays = holding_stays(&db, c.map.id, f, t).await?;
    Ok(Json(
        c.sites
            .iter()
            .map(|s| {
                let mine: Vec<StayDto> = stays
                    .iter()
                    .filter(|x| x.site_id == s.id)
                    .cloned()
                    .map(Into::into)
                    .collect();
                let d = site_dto(s);
                AvailabilityRow {
                    free: mine.is_empty() && !d.closed,
                    site: d,
                    stays: mine,
                }
            })
            .collect(),
    ))
}

/// `POST /campgrounds/<id>/quote`
#[rocket_okapi::openapi(tag = "Campgrounds")]
#[post("/campgrounds/<id>/quote", data = "<body>")]
pub async fn staff_quote(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    id: &str,
    body: Json<QuoteReq>,
) -> ApiResult<Json<Quote>> {
    let c = staff_camp(&db, &user, &scope, &access, id, false).await?;
    let site = site_of(&c, body.site_id)?;
    Ok(Json(price(&c, site, &body)?.2))
}

/// `POST /campgrounds/<id>/stays` — staff book a stay (confirmed).
#[rocket_okapi::openapi(tag = "Campgrounds")]
#[post("/campgrounds/<id>/stays", data = "<body>")]
pub async fn create_stay(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    id: &str,
    body: Json<StayReq>,
) -> ApiResult<Json<StayDto>> {
    let c = staff_camp(&db, &user, &scope, &access, id, true).await?;
    let token = crate::auth::random_secret(24);
    let s = book(
        &db,
        scope.tenant_id,
        &c,
        &body,
        "confirmed",
        "staff",
        Some(user.user_id),
        Some(crate::auth::hash_secret(&token)),
    )
    .await?;
    if let Some(e) = s.email.as_deref() {
        crate::notify::notify_person(
            &db,
            scope.tenant_id,
            e,
            "stay_confirmed",
            guest_vars(&s, &c.map.name, Some(stay_link(&token))),
            Some(("stay", s.id)),
            &format!("stay_confirmed:{}", s.id),
        )
        .await;
    }
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::STAY_CREATE,
        Some("stay"),
        Some(s.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "site": s.site_name, "check_in": s.check_in.to_string() })),
    )
    .await;
    Ok(Json(s.into()))
}

#[derive(Serialize, JsonSchema)]
pub struct Board {
    pub date: String,
    pub arriving: Vec<StayDto>,
    pub departing: Vec<StayDto>,
    pub in_house: Vec<StayDto>,
    pub to_clean: Vec<StayDto>,
    pub requests: Vec<StayDto>,
    pub sites: usize,
    pub occupied: usize,
}

/// `GET /campgrounds/<id>/board?date` — the front desk for a day.
#[rocket_okapi::openapi(tag = "Campgrounds")]
#[get("/campgrounds/<id>/board?<date>")]
pub async fn board(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    id: &str,
    date: Option<&str>,
) -> ApiResult<Json<Board>> {
    let c = staff_camp(&db, &user, &scope, &access, id, false).await?;
    let day = match date {
        Some(d) => self::date(d, "date")?,
        None => Utc::now().date_naive(),
    };
    let all = Stay::find()
        .filter(entity::stay::Column::MapId.eq(c.map.id))
        .filter(entity::stay::Column::CheckIn.lte(day + chrono::Duration::days(120)))
        .filter(entity::stay::Column::CheckOut.gte(day - chrono::Duration::days(14)))
        .order_by_asc(entity::stay::Column::CheckIn)
        .all(&db)
        .await?;
    let dto = |v: Vec<&entity::stay::Model>| {
        v.into_iter()
            .cloned()
            .map(StayDto::from)
            .collect::<Vec<_>>()
    };
    let arriving = dto(all
        .iter()
        .filter(|s| s.check_in == day && s.status == "confirmed")
        .collect());
    let departing = dto(all
        .iter()
        .filter(|s| s.check_out == day && s.status == "checked_in")
        .collect());
    let in_house: Vec<&entity::stay::Model> =
        all.iter().filter(|s| s.status == "checked_in").collect();
    let occupied = in_house.len();
    Ok(Json(Board {
        date: day.to_string(),
        arriving,
        departing,
        in_house: dto(in_house),
        to_clean: dto(all
            .iter()
            .filter(|s| s.status == "checked_out" && s.cleaned_at.is_none())
            .collect()),
        requests: dto(all.iter().filter(|s| s.status == "held").collect()),
        sites: c.sites.len(),
        occupied,
    }))
}

#[derive(Deserialize, JsonSchema)]
pub struct StayAction {
    /// `confirm` | `check_in` | `check_out` | `cancel` | `cleaned` | `payment`
    pub action: String,
    /// For `payment`: cents received (negative for a refund).
    pub amount_cents: Option<i64>,
    pub note: Option<String>,
}

/// The pure rule: which actions a stay in `status` allows.
pub fn allowed(status: &str, action: &str) -> bool {
    matches!(
        (status, action),
        ("held", "confirm")
            | ("held" | "confirmed", "cancel")
            | ("confirmed", "check_in")
            | ("checked_in", "check_out")
            | ("checked_out", "cleaned")
            | (
                "held" | "confirmed" | "checked_in" | "checked_out",
                "payment"
            )
    )
}

/// `PATCH /stays/<id>` — move a stay along, or record a payment.
#[rocket_okapi::openapi(tag = "Campgrounds")]
#[patch("/stays/<id>", data = "<body>")]
pub async fn act(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    id: &str,
    body: Json<StayAction>,
) -> ApiResult<Json<StayDto>> {
    user.require(Permission::PropertyWrite)?;
    let s = Stay::find_by_id(uuid(id, "stay")?)
        .one(&db)
        .await?
        .filter(|s| s.tenant_id == scope.tenant_id)
        .ok_or_else(|| ApiError::NotFound("stay not found".into()))?;
    let c = load(&db, scope.tenant_id, s.map_id).await?;
    if !access.sees(c.map.property_id) {
        return Err(ApiError::NotFound("stay not found".into()));
    }
    let b = body.into_inner();
    if !allowed(&s.status, &b.action) {
        return Err(ApiError::Conflict(format!(
            "a {} stay can't be {}",
            s.status.replace('_', " "),
            b.action.replace('_', " ")
        )));
    }
    let now = Utc::now();
    let mut am: entity::stay::ActiveModel = s.clone().into();
    let mut tell: Option<&str> = None;
    match b.action.as_str() {
        "confirm" => {
            am.status = Set("confirmed".into());
            tell = Some("stay_confirmed");
        }
        "cancel" => {
            am.status = Set("cancelled".into());
            tell = Some("stay_cancelled");
        }
        "check_in" => {
            am.status = Set("checked_in".into());
            am.checked_in_at = Set(Some(now.into()));
        }
        "check_out" => {
            am.status = Set("checked_out".into());
            am.checked_out_at = Set(Some(now.into()));
        }
        "cleaned" => am.cleaned_at = Set(Some(now.into())),
        "payment" => {
            let amt = b
                .amount_cents
                .filter(|a| *a != 0)
                .ok_or_else(|| ApiError::BadRequest("say how much was paid".into()))?;
            let paid = s.paid_cents + amt;
            if paid < 0 {
                return Err(ApiError::BadRequest(
                    "that refunds more than was paid".into(),
                ));
            }
            am.paid_cents = Set(paid);
        }
        _ => unreachable!("checked by allowed()"),
    }
    if let Some(n) = b.note.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        let prev = s.note.clone().unwrap_or_default();
        am.note = Set(Some(if prev.is_empty() {
            n.to_string()
        } else {
            format!("{prev}\n{n}")
        }));
    }
    am.updated_at = Set(now.into());
    let saved = am.update(&db).await?;
    if let (Some(t), Some(e)) = (tell, saved.email.as_deref()) {
        crate::notify::notify_person(
            &db,
            scope.tenant_id,
            e,
            t,
            guest_vars(&saved, &c.map.name, None),
            Some(("stay", saved.id)),
            &format!("{t}:{}", saved.id),
        )
        .await;
    }
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::STAY_UPDATE,
        Some("stay"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "action": b.action, "amount_cents": b.amount_cents })),
    )
    .await;
    Ok(Json(saved.into()))
}

// ---- public routes ---------------------------------------------------------

#[derive(Serialize, JsonSchema)]
pub struct PublicSite {
    pub site: SiteDto,
    /// Free for the dates asked, when dates were given.
    pub free: Option<bool>,
}

#[derive(Serialize, JsonSchema)]
pub struct PublicCamp {
    pub map_id: Uuid,
    pub name: String,
    pub company: String,
    pub check_in_time: String,
    pub check_out_time: String,
    pub max_nights: i32,
    pub deposit_pct: i32,
    pub addons: Vec<Addon>,
    pub policies: Option<String>,
    pub sites: Vec<PublicSite>,
}

async fn public_camp(db: &impl ConnectionTrait, tenant_id: Uuid, id: &str) -> ApiResult<Camp> {
    let c = load(db, tenant_id, uuid(id, "campground")?).await?;
    if !c.map.published || !c.config.booking_open {
        return Err(ApiError::NotFound("campground not found".into()));
    }
    Ok(c)
}

/// `GET /public/campgrounds/<id>?tenant=&from&to` — sites and which are free.
#[rocket_okapi::openapi(tag = "Campgrounds (Public)")]
#[get("/public/campgrounds/<id>?<from>&<to>")]
pub async fn public_view(
    db: crate::db::RequestDb,
    tenant: PublicTenant,
    id: &str,
    from: Option<&str>,
    to: Option<&str>,
) -> ApiResult<Json<PublicCamp>> {
    let c = public_camp(&db, tenant.tenant_id, id).await?;
    let window = match (from, to) {
        (Some(f), Some(t)) => {
            let (f, t) = (date(f, "from")?, date(t, "to")?);
            if t <= f {
                return Err(ApiError::BadRequest(
                    "check-out must be after check-in".into(),
                ));
            }
            Some(holding_stays(&db, c.map.id, f, t).await?)
        }
        _ => None,
    };
    let company = entity::prelude::Tenant::find_by_id(tenant.tenant_id)
        .one(&db)
        .await?
        .map(|t| t.name)
        .unwrap_or_default();
    Ok(Json(PublicCamp {
        map_id: c.map.id,
        name: c.map.name.clone(),
        company,
        check_in_time: c.config.check_in_time.clone(),
        check_out_time: c.config.check_out_time.clone(),
        max_nights: c.config.max_nights,
        deposit_pct: c.config.deposit_pct,
        addons: c.config.addons.clone(),
        policies: c.config.policies.clone(),
        sites: c
            .sites
            .iter()
            .map(|s| {
                let d = site_dto(s);
                let free = window
                    .as_ref()
                    .map(|w| !d.closed && w.iter().all(|x| x.site_id != s.id));
                PublicSite { site: d, free }
            })
            .collect(),
    }))
}

/// `POST /public/campgrounds/<id>/quote?tenant=`
#[rocket_okapi::openapi(tag = "Campgrounds (Public)")]
#[post("/public/campgrounds/<id>/quote", data = "<body>")]
pub async fn public_quote(
    db: crate::db::RequestDb,
    tenant: PublicTenant,
    id: &str,
    body: Json<QuoteReq>,
) -> ApiResult<Json<Quote>> {
    let c = public_camp(&db, tenant.tenant_id, id).await?;
    let site = site_of(&c, body.site_id)?;
    Ok(Json(price(&c, site, &body)?.2))
}

#[derive(Serialize, JsonSchema)]
pub struct StayRequested {
    pub stay: StayDto,
    /// The guest's own page for this stay.
    pub link: String,
}

/// `POST /public/campgrounds/<id>/stays?tenant=` — a guest asks for a stay.
/// It holds the site until staff confirm; the guest gets their link by email.
#[rocket_okapi::openapi(tag = "Campgrounds (Public)")]
#[post("/public/campgrounds/<id>/stays", data = "<body>")]
pub async fn public_request(
    db: crate::db::RequestDb,
    tenant: PublicTenant,
    id: &str,
    body: Json<StayReq>,
) -> ApiResult<Json<StayRequested>> {
    let c = public_camp(&db, tenant.tenant_id, id).await?;
    if body
        .website
        .as_deref()
        .is_some_and(|w| !w.trim().is_empty())
    {
        return Err(ApiError::BadRequest("couldn't take that request".into()));
    }
    if body.email.as_deref().is_none_or(|e| !e.contains('@')) {
        return Err(ApiError::BadRequest(
            "an email is required so we can confirm".into(),
        ));
    }
    let token = crate::auth::random_secret(24);
    let s = book(
        &db,
        tenant.tenant_id,
        &c,
        &body,
        "held",
        "public",
        None,
        Some(crate::auth::hash_secret(&token)),
    )
    .await?;
    let link = stay_link(&token);
    if let Some(e) = s.email.as_deref() {
        crate::notify::notify_person(
            &db,
            tenant.tenant_id,
            e,
            "stay_requested",
            guest_vars(&s, &c.map.name, Some(link.clone())),
            Some(("stay", s.id)),
            &format!("stay_requested:{}", s.id),
        )
        .await;
    }
    crate::notify::notify_staff(
        &db,
        tenant.tenant_id,
        "property:write",
        "stay_request_staff",
        serde_json::json!({ "guest": s.guest_name, "campground": c.map.name, "site": s.site_name, "check_in": s.check_in.to_string(), "check_out": s.check_out.to_string() }),
        Some(("stay", s.id)),
        "stay_request",
        None,
    )
    .await;
    Ok(Json(StayRequested {
        stay: s.into(),
        link,
    }))
}

async fn by_token(db: &impl ConnectionTrait, token: &str) -> ApiResult<entity::stay::Model> {
    if token.trim().len() < 16 {
        return Err(ApiError::NotFound("that link isn't valid".into()));
    }
    Stay::find()
        .filter(entity::stay::Column::TokenHash.eq(crate::auth::hash_secret(token.trim())))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("that link isn't valid".into()))
}

#[derive(Serialize, JsonSchema)]
pub struct GuestStay {
    pub stay: StayDto,
    pub campground: String,
    pub check_in_time: String,
    pub check_out_time: String,
    pub policies: Option<String>,
    pub can_cancel: bool,
}

async fn guest_view(db: &impl ConnectionTrait, s: entity::stay::Model) -> ApiResult<GuestStay> {
    let c = load(db, s.tenant_id, s.map_id).await?;
    let can_cancel =
        matches!(s.status.as_str(), "held" | "confirmed") && s.check_in > Utc::now().date_naive();
    Ok(GuestStay {
        campground: c.map.name.clone(),
        check_in_time: c.config.check_in_time.clone(),
        check_out_time: c.config.check_out_time.clone(),
        policies: c.config.policies.clone(),
        can_cancel,
        stay: s.into(),
    })
}

/// `GET /public/stays/<token>` — the guest's page.
#[rocket_okapi::openapi(tag = "Campgrounds (Public)")]
#[get("/public/stays/<token>")]
pub async fn guest(db: crate::db::RequestDb, token: &str) -> ApiResult<Json<GuestStay>> {
    let s = by_token(&db, token).await?;
    Ok(Json(guest_view(&db, s).await?))
}

/// `POST /public/stays/<token>/cancel` — the guest cancels before arrival.
#[rocket_okapi::openapi(tag = "Campgrounds (Public)")]
#[post("/public/stays/<token>/cancel")]
pub async fn guest_cancel(db: crate::db::RequestDb, token: &str) -> ApiResult<Json<GuestStay>> {
    let s = by_token(&db, token).await?;
    if !(matches!(s.status.as_str(), "held" | "confirmed") && s.check_in > Utc::now().date_naive())
    {
        return Err(ApiError::Conflict(
            "this stay can't be cancelled online; call the office".into(),
        ));
    }
    let mut am: entity::stay::ActiveModel = s.clone().into();
    am.status = Set("cancelled".into());
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&db).await?;
    let camp = SiteMap::find_by_id(saved.map_id)
        .one(&db)
        .await?
        .map(|m| m.name)
        .unwrap_or_default();
    crate::notify::notify_staff(
        &db,
        saved.tenant_id,
        "property:write",
        "stay_cancelled_staff",
        serde_json::json!({ "guest": saved.guest_name, "campground": camp, "site": saved.site_name, "check_in": saved.check_in.to_string() }),
        Some(("stay", saved.id)),
        "stay_cancelled",
        None,
    )
    .await;
    Ok(Json(guest_view(&db, saved).await?))
}

#[cfg(test)]
mod tests {
    use super::allowed;

    #[test]
    fn stay_moves() {
        assert!(allowed("held", "confirm"));
        assert!(allowed("confirmed", "check_in"));
        assert!(allowed("checked_in", "check_out"));
        assert!(allowed("checked_out", "cleaned"));
        assert!(!allowed("held", "check_in"));
        assert!(!allowed("checked_in", "cancel"));
        assert!(!allowed("cancelled", "payment"));
        assert!(allowed("checked_out", "payment"));
    }
}
