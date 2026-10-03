//! Listing syndication (see [`crate::portals`]): the channels and their feed
//! URLs, what's keeping each listing off the portals, and the feeds the
//! portals pull.

use crate::audit::actions as act;
use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::{ApiError, ApiResult};
use crate::portals::{self, ChannelDef, Contact, FeedListing, Issue};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{
    BusinessProfile, Domain, Listing, ListingPhoto, Property, PropertyDetail, SyndicationChannel,
    SyndicationPull, Tenant, Theme,
};
use rand::Rng;
use rocket::http::ContentType;
use rocket::serde::json::Json;
use rocket::{get, patch, post, State};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

const MODULE: &str = "leasing";

fn new_token() -> String {
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut r = rand::thread_rng();
    (0..32)
        .map(|_| CHARS[r.gen_range(0..CHARS.len())] as char)
        .collect()
}

pub fn feed_url(channel: &str, token: &str) -> String {
    format!(
        "{}/feeds/{channel}/{token}.xml",
        crate::oauth::public_api_url().trim_end_matches('/')
    )
}

/// Every channel for the workspace, made (off, with a fresh token) on first look.
async fn channels(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
) -> ApiResult<Vec<entity::syndication_channel::Model>> {
    let mut have = SyndicationChannel::find()
        .filter(entity::syndication_channel::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?;
    for c in portals::CHANNELS {
        if have.iter().any(|h| h.channel == c.key) {
            continue;
        }
        let now = Utc::now();
        have.push(
            entity::syndication_channel::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                channel: Set(c.key.into()),
                enabled: Set(false),
                feed_token: Set(new_token()),
                contact_name: Set(None),
                contact_email: Set(None),
                contact_phone: Set(None),
                last_pulled_at: Set(None),
                last_pull_agent: Set(None),
                pull_count: Set(0),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }
            .insert(db)
            .await?,
        );
    }
    have.sort_by_key(|c| portals::CHANNELS.iter().position(|d| d.key == c.channel));
    Ok(have)
}

/// Who renters reach: what's set on the channel, else the business profile.
async fn contact(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    ch: &entity::syndication_channel::Model,
) -> ApiResult<Contact> {
    let bp = BusinessProfile::find()
        .filter(entity::business_profile::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?;
    let theme = Theme::find()
        .filter(entity::theme::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?;
    let tenant = Tenant::find_by_id(tenant_id).one(db).await?;
    let pick = |a: &Option<String>, b: Option<&Option<String>>| {
        a.clone()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| b.and_then(|x| x.clone()).filter(|s| !s.trim().is_empty()))
            .unwrap_or_default()
    };
    let company = bp
        .as_ref()
        .and_then(|b| b.business_name.clone())
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            theme
                .map(|t| t.company_name)
                .filter(|s| !s.trim().is_empty())
        })
        .or_else(|| tenant.map(|t| t.name))
        .unwrap_or_default();
    Ok(Contact {
        name: ch
            .contact_name
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "Leasing office".into()),
        email: pick(&ch.contact_email, bp.as_ref().map(|b| &b.email)),
        phone: pick(&ch.contact_phone, bp.as_ref().map(|b| &b.phone)),
        website: bp
            .as_ref()
            .and_then(|b| b.website.clone())
            .unwrap_or_default(),
        company,
    })
}

/// Every listing as the feeds see it.
async fn feed_listings(db: &impl ConnectionTrait, tenant_id: Uuid) -> ApiResult<Vec<FeedListing>> {
    let listings = Listing::find()
        .filter(entity::listing::Column::TenantId.eq(tenant_id))
        .order_by_desc(entity::listing::Column::CreatedAt)
        .all(db)
        .await?;
    let props: HashMap<Uuid, entity::property::Model> = Property::find()
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?
        .into_iter()
        .map(|p| (p.id, p))
        .collect();
    let details: HashMap<Uuid, entity::property_detail::Model> = PropertyDetail::find()
        .filter(entity::property_detail::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?
        .into_iter()
        .map(|d| (d.property_id, d))
        .collect();
    let mut photos: HashMap<Uuid, Vec<entity::listing_photo::Model>> = HashMap::new();
    for p in ListingPhoto::find()
        .filter(entity::listing_photo::Column::TenantId.eq(tenant_id))
        .order_by_asc(entity::listing_photo::Column::Position)
        .all(db)
        .await?
    {
        photos.entry(p.listing_id).or_default().push(p);
    }
    // A verified website domain gives each listing its own page.
    let site = Domain::find()
        .filter(entity::domain::Column::TenantId.eq(tenant_id))
        .filter(entity::domain::Column::VerifiedAt.is_not_null())
        .one(db)
        .await?
        .map(|d| format!("https://{}", d.hostname));
    Ok(listings
        .into_iter()
        .map(|l| {
            let p = l.property_id.and_then(|id| props.get(&id));
            let d = l.property_id.and_then(|id| details.get(&id));
            FeedListing {
                id: l.id,
                property_id: l.property_id,
                property_name: p.map(|p| p.name.clone()).unwrap_or_default(),
                property_type: p.map(|p| p.property_type.clone()).unwrap_or_default(),
                // "Portland, OR" written in the city: the city alone, and the state.
                city: l.city.split(',').next().unwrap_or("").trim().to_string(),
                state: if l.state.trim().is_empty() {
                    p.map(|p| p.state.clone()).unwrap_or_else(|| {
                        crate::geo::state_code(l.city.rsplit(',').next().unwrap_or("").trim())
                    })
                } else {
                    l.state.clone()
                },
                zip: if l.postal_code.trim().is_empty() {
                    p.map(|p| p.postal_code.clone()).unwrap_or_default()
                } else {
                    l.postal_code.clone()
                },
                latitude: d.and_then(|d| d.latitude),
                longitude: d.and_then(|d| d.longitude),
                photos: photos
                    .get(&l.id)
                    .map(|ps| {
                        ps.iter()
                            .map(|ph| {
                                (
                                    crate::routes::listings::photos::public_url(ph.id),
                                    ph.caption.clone().or_else(|| Some(ph.alt_text.clone())),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                website: site.as_ref().map(|s| format!("{s}/listings/{}", l.id)),
                updated: l
                    .created_at
                    .naive_utc()
                    .format("%Y-%m-%dT%H:%M:%S")
                    .to_string(),
                title: l.title,
                address: l.address,
                beds: l.beds,
                baths: l.baths,
                sqft: l.sqft,
                rent_cents: l.rent_cents,
                description: l.description,
                available: l.available_on,
                status: l.status,
                is_public: l.is_public,
                syndicate: l.syndicate,
            }
        })
        .collect())
}

async fn render(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    ch: &entity::syndication_channel::Model,
) -> ApiResult<(String, usize)> {
    let slug = Tenant::find_by_id(tenant_id)
        .one(db)
        .await?
        .map(|t| t.slug)
        .unwrap_or_default();
    let who = contact(db, tenant_id, ch).await?;
    let ready: Vec<FeedListing> = feed_listings(db, tenant_id)
        .await?
        .into_iter()
        .filter(portals::ready)
        .collect();
    let xml = match ch.channel.as_str() {
        "zillow" => portals::zillow_feed(&slug, &who, &ready),
        _ => portals::mits_feed(&slug, &who, &ready),
    };
    Ok((xml, ready.len()))
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct PullDto {
    pub at: String,
    pub agent: Option<String>,
    pub listings: i32,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ChannelDto {
    #[serde(flatten)]
    pub def: ChannelDef,
    pub enabled: bool,
    pub feed_url: String,
    /// What's set on the channel (blank falls back to the business profile).
    pub contact_name: Option<String>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
    /// What the feed actually shows.
    pub shown_name: String,
    pub shown_email: String,
    pub shown_phone: String,
    pub company: String,
    pub issues: Vec<Issue>,
    /// Listings in the feed right now.
    pub listings: usize,
    pub last_pulled_at: Option<String>,
    pub last_pull_agent: Option<String>,
    pub pull_count: i32,
    pub pulls: Vec<PullDto>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ListingSyndication {
    pub id: Uuid,
    pub property_id: Option<Uuid>,
    pub title: String,
    pub address: String,
    pub city: String,
    pub state: String,
    pub postal_code: String,
    pub rent_label: String,
    pub status: String,
    pub is_public: bool,
    pub syndicate: bool,
    pub photos: usize,
    pub ready: bool,
    pub issues: Vec<Issue>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct SyndicationDto {
    pub channels: Vec<ChannelDto>,
    pub listings: Vec<ListingSyndication>,
}

async fn channel_dto(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    ch: entity::syndication_channel::Model,
    ready: usize,
) -> ApiResult<ChannelDto> {
    let def = *portals::channel(&ch.channel)
        .ok_or_else(|| ApiError::NotFound("no such channel".into()))?;
    let who = contact(db, tenant_id, &ch).await?;
    let pulls = SyndicationPull::find()
        .filter(entity::syndication_pull::Column::TenantId.eq(tenant_id))
        .filter(entity::syndication_pull::Column::Channel.eq(ch.channel.clone()))
        .order_by_desc(entity::syndication_pull::Column::PulledAt)
        .limit(5)
        .all(db)
        .await?
        .into_iter()
        .map(|p| PullDto {
            at: p.pulled_at.to_rfc3339(),
            agent: p.user_agent,
            listings: p.listings,
        })
        .collect();
    Ok(ChannelDto {
        def,
        enabled: ch.enabled,
        feed_url: feed_url(&ch.channel, &ch.feed_token),
        issues: portals::contact_issues(&who),
        shown_name: who.name,
        shown_email: who.email,
        shown_phone: who.phone,
        company: who.company,
        contact_name: ch.contact_name,
        contact_email: ch.contact_email,
        contact_phone: ch.contact_phone,
        listings: ready,
        last_pulled_at: ch.last_pulled_at.map(|d| d.to_rfc3339()),
        last_pull_agent: ch.last_pull_agent,
        pull_count: ch.pull_count,
        pulls,
    })
}

/// `GET /syndication` — the channels, their feed URLs and pulls, and what's
/// keeping each listing off the portals.
#[rocket_okapi::openapi(tag = "Listings")]
#[get("/syndication")]
pub async fn overview(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<SyndicationDto>> {
    user.require(Permission::ListingRead)?;
    crate::modules::require_enabled(&db, scope.tenant_id, MODULE).await?;
    let listings = feed_listings(&db, scope.tenant_id).await?;
    let ready = listings.iter().filter(|l| portals::ready(l)).count();
    let mut out = vec![];
    for ch in channels(&db, scope.tenant_id).await? {
        out.push(channel_dto(&db, scope.tenant_id, ch, ready).await?);
    }
    Ok(Json(SyndicationDto {
        channels: out,
        listings: listings
            .into_iter()
            .map(|l| ListingSyndication {
                ready: portals::ready(&l),
                issues: portals::readiness(&l),
                photos: l.photos.len(),
                rent_label: usd(l.rent_cents),
                id: l.id,
                property_id: l.property_id,
                title: l.title,
                address: l.address,
                city: l.city,
                state: l.state,
                postal_code: l.zip,
                status: l.status,
                is_public: l.is_public,
                syndicate: l.syndicate,
            })
            .collect(),
    }))
}

async fn find_channel(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    key: &str,
) -> ApiResult<entity::syndication_channel::Model> {
    channels(db, tenant_id)
        .await?
        .into_iter()
        .find(|c| c.channel == key)
        .ok_or_else(|| ApiError::NotFound("no such channel".into()))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ChannelReq {
    pub enabled: Option<bool>,
    /// "" clears it (the business profile's is used).
    pub contact_name: Option<String>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
}

fn blank_none(s: String) -> Option<String> {
    Some(s.trim().to_string()).filter(|s| !s.is_empty())
}

/// `PATCH /syndication/<channel>` — turn a channel on or off, set its contact.
#[rocket_okapi::openapi(tag = "Listings")]
#[patch("/syndication/<channel>", data = "<body>")]
pub async fn update_channel(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    channel: &str,
    body: Json<ChannelReq>,
) -> ApiResult<Json<ChannelDto>> {
    user.require(Permission::ListingWrite)?;
    crate::modules::require_enabled(&db, scope.tenant_id, MODULE).await?;
    let ch = find_channel(&db, scope.tenant_id, channel).await?;
    let b = body.into_inner();
    let mut am: entity::syndication_channel::ActiveModel = ch.clone().into();
    if let Some(e) = b.contact_email.clone() {
        let e = e.trim().to_lowercase();
        if !e.is_empty() && !(e.contains('@') && e.contains('.')) {
            return Err(ApiError::BadRequest("that email doesn't look right".into()));
        }
        am.contact_email = Set(blank_none(e));
    }
    if let Some(v) = b.contact_name {
        am.contact_name = Set(blank_none(v));
    }
    if let Some(v) = b.contact_phone {
        am.contact_phone = Set(blank_none(v));
    }
    if let Some(on) = b.enabled {
        am.enabled = Set(on);
    }
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&db).await?;
    if let Some(on) = b.enabled {
        if on {
            let who = contact(&db, scope.tenant_id, &saved).await?;
            if let Some(i) = portals::contact_issues(&who)
                .into_iter()
                .find(|i| i.blocking)
            {
                return Err(ApiError::BadRequest(i.message));
            }
        }
    }
    crate::audit::record(
        &db,
        Some(user.user_id),
        act::SYNDICATION_SAVE,
        Some("syndication_channel"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(json!({ "channel": saved.channel, "enabled": saved.enabled })),
    )
    .await;
    let ready = feed_listings(&db, scope.tenant_id)
        .await?
        .iter()
        .filter(|l| portals::ready(l))
        .count();
    Ok(Json(channel_dto(&db, scope.tenant_id, saved, ready).await?))
}

/// `POST /syndication/<channel>/rotate` — a new feed URL; the old one stops
/// working (the portal has to be given the new one).
#[rocket_okapi::openapi(tag = "Listings")]
#[post("/syndication/<channel>/rotate")]
pub async fn rotate(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    channel: &str,
) -> ApiResult<Json<ChannelDto>> {
    user.require(Permission::ListingWrite)?;
    let ch = find_channel(&db, scope.tenant_id, channel).await?;
    let mut am: entity::syndication_channel::ActiveModel = ch.into();
    am.feed_token = Set(new_token());
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        act::SYNDICATION_SAVE,
        Some("syndication_channel"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(json!({ "channel": saved.channel, "rotated": true })),
    )
    .await;
    let ready = feed_listings(&db, scope.tenant_id)
        .await?
        .iter()
        .filter(|l| portals::ready(l))
        .count();
    Ok(Json(channel_dto(&db, scope.tenant_id, saved, ready).await?))
}

/// `GET /syndication/<channel>/preview` — the feed as the portal would get
/// it, for checking (doesn't count as a pull, works while the channel is off).
#[rocket_okapi::openapi(skip)]
#[get("/syndication/<channel>/preview")]
pub async fn preview(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    channel: &str,
) -> ApiResult<(ContentType, String)> {
    user.require(Permission::ListingRead)?;
    let ch = find_channel(&db, scope.tenant_id, channel).await?;
    let (xml, _) = render(&db, scope.tenant_id, &ch).await?;
    Ok((ContentType::XML, xml))
}

/// The portal's user agent, for the pull log.
pub struct Agent(Option<String>);

#[rocket::async_trait]
impl<'r> rocket::request::FromRequest<'r> for Agent {
    type Error = ();
    async fn from_request(
        req: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        rocket::request::Outcome::Success(Agent(
            req.headers()
                .get_one("User-Agent")
                .map(|s| s.chars().take(200).collect()),
        ))
    }
}

/// `GET /feeds/<channel>/<token>.xml` — the feed a portal pulls. The token is
/// the secret; a wrong one, or a channel that's off, is a 404.
#[rocket_okapi::openapi(skip)]
#[get("/feeds/<channel>/<token>")]
pub async fn feed(
    state: &State<AppState>,
    agent: Agent,
    channel: &str,
    token: &str,
) -> ApiResult<(ContentType, String)> {
    let token = token.trim_end_matches(".xml");
    let not_found = || ApiError::NotFound("feed not found".into());
    if token.len() < 16 {
        return Err(not_found());
    }
    let ch = SyndicationChannel::find()
        .filter(entity::syndication_channel::Column::FeedToken.eq(token))
        .filter(entity::syndication_channel::Column::Channel.eq(channel))
        .one(&state.db)
        .await?
        .ok_or_else(not_found)?;
    if !ch.enabled {
        return Err(not_found());
    }
    if crate::modules::require_enabled(&state.db, ch.tenant_id, MODULE)
        .await
        .is_err()
    {
        return Err(not_found());
    }
    let (xml, n) = render(&state.db, ch.tenant_id, &ch).await?;
    let now = Utc::now();
    let _ = entity::syndication_pull::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(ch.tenant_id),
        channel: Set(ch.channel.clone()),
        user_agent: Set(agent.0.clone()),
        listings: Set(n as i32),
        pulled_at: Set(now.into()),
    }
    .insert(&state.db)
    .await;
    let count = ch.pull_count + 1;
    let mut am: entity::syndication_channel::ActiveModel = ch.into();
    am.last_pulled_at = Set(Some(now.into()));
    am.last_pull_agent = Set(agent.0);
    am.pull_count = Set(count);
    let _ = am.update(&state.db).await;
    Ok((ContentType::XML, xml))
}
