//! **Addresses and photos** (Vantedge phase 2C) — the map side of a property.
//!
//! * [`suggest`] — address suggestions as you type: the workspace's own
//!   properties first ("known"), then the map: **Photon** (free, no key) or
//!   **Google Places** when `google.maps_api_key` is in the vault. Picking one
//!   fills street, city, state and ZIP; the Census geocoder fixes the
//!   coordinates afterwards through enrichment, as before.
//! * [`fetch_photo`] — a street photo for a property, fetched once and stored
//!   as an ordinary property document (category `photo`) that becomes the
//!   hero: Google **Street View Static** when a panorama exists at the
//!   address, else the **satellite Static Map**. Without a live key a
//!   placeholder card is drawn instead (`photo_status = placeholder`), so the
//!   nightly job fills it in the moment a key arrives.
//! * The nightly `property_photo_scan` job (per workspace) finds properties
//!   with no photo — never tried, tried and failed over a week ago, or a
//!   placeholder now that a key exists — and fetches up to ten a night.
//!
//! Sandbox-first like every provider: Google is called only when
//! `LIVE_PROVIDERS` lists `maps` (Photon is keyless and always allowed).

use crate::error::{ApiError, ApiResult};
use crate::providers::client::build_http_client;
use crate::storage::{sha256_hex, ObjectStore};
use chrono::{Duration, Utc};
use entity::prelude::Property;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The vault key holding a Google Maps Platform API key (Places, Street View
/// Static, Maps Static enabled).
pub const MAPS_KEY: &str = "google.maps_api_key";
/// The provider key in `LIVE_PROVIDERS`.
pub const PROVIDER: &str = "maps";
/// The nightly per-workspace scan.
pub const SCAN_KIND: &str = "property_photo_scan";
/// One property's photo, fetched in the background right after it's created.
pub const FETCH_KIND: &str = "property_photo";
/// Photos a night, so a big import doesn't burn the quota at once.
const PER_NIGHT: usize = 10;
/// How long a failed fetch waits before it's tried again.
const RETRY_AFTER_DAYS: i64 = 7;
const PHOTON_URL: &str = "https://photon.komoot.io/api/";
const PLACES_URL: &str = "https://places.googleapis.com/v1/places:autocomplete";
const STREETVIEW_META: &str = "https://maps.googleapis.com/maps/api/streetview/metadata";
const STREETVIEW_URL: &str = "https://maps.googleapis.com/maps/api/streetview";
const STATICMAP_URL: &str = "https://maps.googleapis.com/maps/api/staticmap";

/// One address suggestion.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema, PartialEq)]
pub struct Place {
    /// One line, as shown in the list.
    pub label: String,
    pub address: String,
    pub city: String,
    pub state: String,
    pub postal_code: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    /// `address` — a house was found; `street` — only the street, check the number.
    pub precision: String,
    /// `known` (a property you already have) | `photon` | `google`
    pub source: String,
    /// The existing property, for a `known` suggestion.
    pub property_id: Option<Uuid>,
}

/// The maps key, if the workspace (or platform) has one in the vault.
pub async fn maps_key(db: &impl ConnectionTrait, tenant_id: Uuid) -> Option<String> {
    crate::secrets::reveal(db, Some(tenant_id), MAPS_KEY)
        .await
        .ok()
        .flatten()
        .filter(|k| !k.trim().is_empty())
}

/// Whether Google is really called: a key in the vault and `maps` live.
pub async fn google_live(db: &impl ConnectionTrait, tenant_id: Uuid) -> bool {
    crate::providers::is_live(PROVIDER) && maps_key(db, tenant_id).await.is_some()
}

// ---------------------------------------------------------------------------
// Suggestions
// ---------------------------------------------------------------------------

/// Common US street suffixes, shortened the way the post office writes them.
pub fn shorten_suffix(street: &str) -> String {
    let pairs = [
        ("Street", "St"),
        ("Avenue", "Ave"),
        ("Boulevard", "Blvd"),
        ("Drive", "Dr"),
        ("Road", "Rd"),
        ("Lane", "Ln"),
        ("Court", "Ct"),
        ("Circle", "Cir"),
        ("Place", "Pl"),
        ("Terrace", "Ter"),
        ("Highway", "Hwy"),
        ("Parkway", "Pkwy"),
        ("Trail", "Trl"),
        ("Way", "Way"),
    ];
    let mut words: Vec<String> = street.split_whitespace().map(str::to_string).collect();
    if let Some(last) = words.last_mut() {
        for (long, short) in pairs {
            if last.eq_ignore_ascii_case(long) {
                *last = short.to_string();
            }
        }
    }
    words.join(" ")
}

/// A state name or code → its two-letter code (unknown values pass through).
pub fn state_code(s: &str) -> String {
    let t = s.trim();
    if t.len() == 2 {
        return t.to_uppercase();
    }
    const STATES: &[(&str, &str)] = &[
        ("alabama", "AL"),
        ("alaska", "AK"),
        ("arizona", "AZ"),
        ("arkansas", "AR"),
        ("california", "CA"),
        ("colorado", "CO"),
        ("connecticut", "CT"),
        ("delaware", "DE"),
        ("florida", "FL"),
        ("georgia", "GA"),
        ("hawaii", "HI"),
        ("idaho", "ID"),
        ("illinois", "IL"),
        ("indiana", "IN"),
        ("iowa", "IA"),
        ("kansas", "KS"),
        ("kentucky", "KY"),
        ("louisiana", "LA"),
        ("maine", "ME"),
        ("maryland", "MD"),
        ("massachusetts", "MA"),
        ("michigan", "MI"),
        ("minnesota", "MN"),
        ("mississippi", "MS"),
        ("missouri", "MO"),
        ("montana", "MT"),
        ("nebraska", "NE"),
        ("nevada", "NV"),
        ("new hampshire", "NH"),
        ("new jersey", "NJ"),
        ("new mexico", "NM"),
        ("new york", "NY"),
        ("north carolina", "NC"),
        ("north dakota", "ND"),
        ("ohio", "OH"),
        ("oklahoma", "OK"),
        ("oregon", "OR"),
        ("pennsylvania", "PA"),
        ("rhode island", "RI"),
        ("south carolina", "SC"),
        ("south dakota", "SD"),
        ("tennessee", "TN"),
        ("texas", "TX"),
        ("utah", "UT"),
        ("vermont", "VT"),
        ("virginia", "VA"),
        ("washington", "WA"),
        ("west virginia", "WV"),
        ("wisconsin", "WI"),
        ("wyoming", "WY"),
        ("district of columbia", "DC"),
    ];
    STATES
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(t))
        .map(|(_, c)| c.to_string())
        .unwrap_or_else(|| t.to_string())
}

fn label_of(address: &str, city: &str, state: &str, zip: &str) -> String {
    let mut s = address.to_string();
    if !city.is_empty() {
        s.push_str(", ");
        s.push_str(city);
    }
    if !state.is_empty() {
        s.push_str(", ");
        s.push_str(state);
    }
    if !zip.is_empty() {
        s.push(' ');
        s.push_str(zip);
    }
    s
}

/// Photon's GeoJSON → places (US only). A result with no house number keeps
/// the number the person typed and is marked `street`.
pub fn parse_photon(body: &serde_json::Value, typed: &str) -> Vec<Place> {
    let typed_number: Option<String> = typed
        .split_whitespace()
        .next()
        .filter(|w| w.chars().all(|c| c.is_ascii_digit()) && !w.is_empty())
        .map(str::to_string);
    let mut out = Vec::new();
    for f in body["features"].as_array().into_iter().flatten() {
        let p = &f["properties"];
        if p["countrycode"].as_str().map(|c| c.to_uppercase()) != Some("US".into()) {
            continue;
        }
        let street = p["street"]
            .as_str()
            .or(p["name"].as_str())
            .unwrap_or("")
            .trim();
        if street.is_empty() {
            continue;
        }
        let number = p["housenumber"].as_str().map(str::to_string);
        let (address, precision) = match (&number, &typed_number) {
            (Some(n), _) => (format!("{n} {}", shorten_suffix(street)), "address"),
            (None, Some(n)) => (format!("{n} {}", shorten_suffix(street)), "street"),
            (None, None) => (shorten_suffix(street), "street"),
        };
        let city = p["city"]
            .as_str()
            .or(p["county"].as_str())
            .unwrap_or("")
            .to_string();
        let state = state_code(p["state"].as_str().unwrap_or(""));
        let zip = p["postcode"].as_str().unwrap_or("").to_string();
        let coords = f["geometry"]["coordinates"].as_array();
        out.push(Place {
            label: label_of(&address, &city, &state, &zip),
            address,
            city,
            state,
            postal_code: zip,
            longitude: coords.and_then(|c| c.first()).and_then(|v| v.as_f64()),
            latitude: coords.and_then(|c| c.get(1)).and_then(|v| v.as_f64()),
            precision: precision.into(),
            source: "photon".into(),
            property_id: None,
        });
    }
    out
}

/// Google Places (New) autocomplete → places. Only street-level results; the
/// city / state / ZIP come from the secondary text.
pub fn parse_places(body: &serde_json::Value) -> Vec<Place> {
    let mut out = Vec::new();
    for s in body["suggestions"].as_array().into_iter().flatten() {
        let pp = &s["placePrediction"];
        let types: Vec<&str> = pp["types"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|t| t.as_str())
            .collect();
        let ok = types.iter().any(|t| {
            [
                "street_address",
                "premise",
                "subpremise",
                "route",
                "geocode",
            ]
            .contains(t)
        });
        if !ok {
            continue;
        }
        let main = pp["structuredFormat"]["mainText"]["text"]
            .as_str()
            .unwrap_or("")
            .trim();
        let secondary = pp["structuredFormat"]["secondaryText"]["text"]
            .as_str()
            .unwrap_or("");
        if main.is_empty() {
            continue;
        }
        // "Hesperia, CA 92345, USA" → city / state / zip
        let parts: Vec<&str> = secondary.split(',').map(str::trim).collect();
        let city = parts.first().copied().unwrap_or("").to_string();
        let (state, zip) = parts
            .get(1)
            .map(|sz| {
                let mut it = sz.split_whitespace();
                (
                    state_code(it.next().unwrap_or("")),
                    it.next().unwrap_or("").to_string(),
                )
            })
            .unwrap_or_default();
        let precision = if main.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            "address"
        } else {
            "street"
        };
        let address = shorten_suffix(main);
        out.push(Place {
            label: label_of(&address, &city, &state, &zip),
            address,
            city,
            state,
            postal_code: zip,
            latitude: None,
            longitude: None,
            precision: precision.into(),
            source: "google".into(),
            property_id: None,
        });
    }
    out
}

/// Address suggestions for `q`: known properties, then the map.
pub async fn suggest(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    q: &str,
    limit: usize,
) -> ApiResult<Vec<Place>> {
    let q = q.trim();
    if q.chars().count() < 3 {
        return Ok(vec![]);
    }
    let limit = limit.clamp(1, 10);
    let mut out: Vec<Place> = Vec::new();

    // Known: your own properties whose address starts the same way.
    let needle = q.to_lowercase();
    let known = Property::find()
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .order_by_asc(entity::property::Column::Address)
        .limit(200)
        .all(db)
        .await?;
    for p in known {
        if p.address.to_lowercase().starts_with(&needle) && out.len() < 3 {
            out.push(Place {
                label: format!(
                    "{} · {}",
                    label_of(&p.address, &p.city, &p.state, &p.postal_code),
                    p.name
                ),
                address: p.address.clone(),
                city: p.city.clone(),
                state: p.state.clone(),
                postal_code: p.postal_code.clone(),
                latitude: None,
                longitude: None,
                precision: "address".into(),
                source: "known".into(),
                property_id: Some(p.id),
            });
        }
    }

    // The map.
    let http = build_http_client().map_err(|e| ApiError::Internal(anyhow::anyhow!(e.0)))?;
    let from_map: Vec<Place> = if google_live(db, tenant_id).await {
        let key = maps_key(db, tenant_id).await.unwrap_or_default();
        let body = serde_json::json!({
            "input": q,
            "includedRegionCodes": ["us"],
            "languageCode": "en",
        });
        match http
            .post(PLACES_URL)
            .header("X-Goog-Api-Key", key)
            .json(&body)
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => r
                .json::<serde_json::Value>()
                .await
                .map(|v| parse_places(&v))
                .unwrap_or_default(),
            Ok(r) => {
                tracing::warn!("places autocomplete returned {}", r.status());
                vec![]
            }
            Err(e) => {
                tracing::warn!("places autocomplete failed: {e}");
                vec![]
            }
        }
    } else {
        // Bias toward where the workspace already is: any property with
        // coordinates (Alpha biases toward the shop the same way).
        let near = entity::prelude::PropertyDetail::find()
            .filter(entity::property_detail::Column::Latitude.is_not_null())
            .filter(
                entity::property_detail::Column::PropertyId.is_in(
                    Property::find()
                        .filter(entity::property::Column::TenantId.eq(tenant_id))
                        .limit(50)
                        .all(db)
                        .await?
                        .into_iter()
                        .map(|p| p.id)
                        .collect::<Vec<_>>(),
                ),
            )
            .one(db)
            .await?
            .and_then(|d| Some((d.latitude?, d.longitude?)));
        let mut params: Vec<(&str, String)> = vec![
            ("q", q.to_string()),
            ("limit", (limit * 2).to_string()),
            ("lang", "en".into()),
            ("layer", "house".into()),
            ("layer", "street".into()),
        ];
        if let Some((lat, lon)) = near {
            params.push(("lat", lat.to_string()));
            params.push(("lon", lon.to_string()));
        }
        match http.get(PHOTON_URL).query(&params).send().await {
            Ok(r) if r.status().is_success() => r
                .json::<serde_json::Value>()
                .await
                .map(|v| parse_photon(&v, q))
                .unwrap_or_default(),
            Ok(r) => {
                tracing::warn!("photon returned {}", r.status());
                vec![]
            }
            Err(e) => {
                tracing::warn!("photon failed: {e}");
                vec![]
            }
        }
    };
    for p in from_map {
        let dup = out.iter().any(|o| {
            o.address.eq_ignore_ascii_case(&p.address) && o.city.eq_ignore_ascii_case(&p.city)
        });
        if !dup && out.len() < limit {
            out.push(p);
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Photos
// ---------------------------------------------------------------------------

/// One line for the map: street, city, state ZIP.
pub fn full_address(p: &entity::property::Model) -> String {
    label_of(&p.address, &p.city, &p.state, &p.postal_code)
}

/// The placeholder card drawn when no live key is set: the address on the
/// Vantedge teal, so a property is never a grey box.
pub fn placeholder_svg(p: &entity::property::Model) -> String {
    let esc = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    let line2 = {
        let mut s = p.city.clone();
        if !p.state.is_empty() {
            s.push_str(", ");
            s.push_str(&p.state);
        }
        if !p.postal_code.is_empty() {
            s.push(' ');
            s.push_str(&p.postal_code);
        }
        s
    };
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="800" height="500" viewBox="0 0 800 500">
<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#3cc3c9"/><stop offset="0.55" stop-color="#0e7c86"/><stop offset="1" stop-color="#0b3a44"/></linearGradient></defs>
<rect width="800" height="500" fill="url(#g)"/>
<path d="M120 330 L400 150 L680 330" fill="none" stroke="#f2fbfb" stroke-opacity="0.35" stroke-width="18" stroke-linecap="round" stroke-linejoin="round"/>
<path d="M170 330 V420 H630 V330" fill="none" stroke="#f2fbfb" stroke-opacity="0.35" stroke-width="18" stroke-linecap="round" stroke-linejoin="round"/>
<text x="400" y="455" text-anchor="middle" font-family="Helvetica, Arial, sans-serif" font-size="30" font-weight="700" fill="#f2fbfb">{}</text>
<text x="400" y="488" text-anchor="middle" font-family="Helvetica, Arial, sans-serif" font-size="18" fill="#f2fbfb" fill-opacity="0.85">{}</text>
</svg>"##,
        esc(&p.address),
        esc(&line2)
    )
}

/// Whether a property should be (re)tried tonight.
pub fn wants_photo(
    status: &str,
    attempted_at: Option<chrono::DateTime<chrono::FixedOffset>>,
    live: bool,
    now: chrono::DateTime<Utc>,
) -> bool {
    match status {
        "stored" => false,
        "placeholder" => live,
        "failed" => attempted_at
            .map(|a| now - a.with_timezone(&Utc) > Duration::days(RETRY_AFTER_DAYS))
            .unwrap_or(true),
        _ => true,
    }
}

/// Store bytes as a stored document and return its id.
#[allow(clippy::too_many_arguments)]
pub async fn store_document(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    owner_type: &str,
    owner_id: Uuid,
    filename: &str,
    mime: &str,
    category: &str,
    bytes: &[u8],
) -> anyhow::Result<Uuid> {
    let id = Uuid::new_v4();
    let key = format!("{tenant_id}/{id}");
    ObjectStore::from_env()?.put_bytes(&key, bytes).await?;
    let now = Utc::now();
    entity::document::ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant_id),
        owner_type: Set(owner_type.into()),
        owner_id: Set(owner_id),
        filename: Set(filename.into()),
        category: Set(Some(category.into())),
        requires_wet_ink: Set(false),
        physical_location: Set(None),
        mime_type: Set(mime.into()),
        size_bytes: Set(bytes.len() as i64),
        checksum: Set(Some(sha256_hex(bytes))),
        version: Set(1),
        previous_version_id: Set(None),
        storage_key: Set(key),
        status: Set("stored".into()),
        retention_expires_at: Set(None),
        created_by: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    Ok(id)
}

/// What a fetch produced.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct PhotoResult {
    /// `stored` | `placeholder` | `failed`
    pub status: String,
    /// `streetview` | `satellite` | `placeholder`
    pub source: Option<String>,
    pub document_id: Option<Uuid>,
    pub error: Option<String>,
}

#[derive(Deserialize)]
struct StreetViewMeta {
    status: String,
}

/// Fetch (or draw) the property's photo, store it, make it the hero, and
/// record the outcome on the property.
pub async fn fetch_photo(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    property: entity::property::Model,
) -> anyhow::Result<PhotoResult> {
    let addr = full_address(&property);
    let live = google_live(db, tenant_id).await;
    let mut result = if live {
        match fetch_google(db, tenant_id, &addr).await {
            Ok((bytes, source)) => {
                match store_document(
                    db,
                    tenant_id,
                    "property",
                    property.id,
                    &format!("{source}.jpg"),
                    "image/jpeg",
                    "photo",
                    &bytes,
                )
                .await
                {
                    Ok(id) => PhotoResult {
                        status: "stored".into(),
                        source: Some(source),
                        document_id: Some(id),
                        error: None,
                    },
                    Err(e) => PhotoResult {
                        status: "failed".into(),
                        source: None,
                        document_id: None,
                        error: Some(format!("couldn't store the photo: {e}")),
                    },
                }
            }
            Err(e) => PhotoResult {
                status: "failed".into(),
                source: None,
                document_id: None,
                error: Some(e),
            },
        }
    } else {
        PhotoResult {
            status: "placeholder".into(),
            source: Some("placeholder".into()),
            document_id: None,
            error: None,
        }
    };
    // A failure, or no key, still leaves the property with a card to show —
    // but only the first time, so a real photo never gets replaced by a card.
    if result.document_id.is_none() && property.photo_status != "stored" {
        let svg = placeholder_svg(&property);
        if let Ok(id) = store_document(
            db,
            tenant_id,
            "property",
            property.id,
            "placeholder.svg",
            "image/svg+xml",
            "photo",
            svg.as_bytes(),
        )
        .await
        {
            result.document_id = Some(id);
        }
    }
    let mut am: entity::property::ActiveModel = property.clone().into();
    if let Some(id) = result.document_id {
        // Only take the hero spot when nothing was chosen by hand.
        let hand_picked = property
            .image_url
            .as_deref()
            .is_some_and(|u| !u.is_empty() && property.photo_status == "none");
        if !hand_picked || result.status == "stored" {
            am.image_url = Set(Some(format!("doc:{id}")));
        }
    }
    am.photo_status = Set(result.status.clone());
    am.photo_attempted_at = Set(Some(Utc::now().into()));
    am.photo_error = Set(result.error.clone());
    am.update(db).await?;
    crate::audit::record(
        db,
        None,
        crate::audit::actions::PROPERTY_PHOTO,
        Some("property"),
        Some(property.id.to_string()),
        Some(tenant_id),
        Some(serde_json::json!({ "status": result.status, "source": result.source })),
    )
    .await;
    Ok(result)
}

/// Street View when a panorama exists at the address, else the satellite map.
async fn fetch_google(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    addr: &str,
) -> Result<(Vec<u8>, String), String> {
    let key = maps_key(db, tenant_id).await.ok_or("no maps key")?;
    let http = build_http_client().map_err(|e| e.0)?;
    let meta = http
        .get(STREETVIEW_META)
        .query(&[("location", addr), ("source", "outdoor"), ("key", &key)])
        .send()
        .await
        .map_err(|e| format!("street view metadata: {e}"))?;
    let has_pano = meta
        .json::<StreetViewMeta>()
        .await
        .map(|m| m.status == "OK")
        .unwrap_or(false);
    let (url, params, source): (&str, Vec<(&str, String)>, &str) = if has_pano {
        (
            STREETVIEW_URL,
            vec![
                ("size", "800x500".into()),
                ("location", addr.into()),
                ("source", "outdoor".into()),
                ("fov", "80".into()),
                ("key", key.clone()),
            ],
            "streetview",
        )
    } else {
        (
            STATICMAP_URL,
            vec![
                ("center", addr.into()),
                ("zoom", "19".into()),
                ("size", "800x500".into()),
                ("maptype", "satellite".into()),
                ("key", key.clone()),
            ],
            "satellite",
        )
    };
    let resp = http
        .get(url)
        .query(&params)
        .send()
        .await
        .map_err(|e| format!("{source}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("{source} returned HTTP {}", resp.status()));
    }
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    if !ct.starts_with("image/") {
        return Err(format!("{source} returned {ct}, not an image"));
    }
    let bytes = resp.bytes().await.map_err(|e| format!("{source}: {e}"))?;
    if bytes.len() < 2_000 {
        return Err(format!("{source} returned a blank image"));
    }
    Ok((bytes.to_vec(), source.into()))
}

/// The nightly scan: a few properties without a photo, oldest first.
pub async fn scan(db: &impl ConnectionTrait, tenant_id: Uuid) -> anyhow::Result<serde_json::Value> {
    let live = google_live(db, tenant_id).await;
    let now = Utc::now();
    let candidates = Property::find()
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .filter(entity::property::Column::PhotoStatus.ne("stored"))
        .order_by_asc(entity::property::Column::PhotoAttemptedAt)
        .all(db)
        .await?;
    let (mut stored, mut placeholders, mut failed) = (0, 0, 0);
    for p in candidates
        .into_iter()
        .filter(|p| wants_photo(&p.photo_status, p.photo_attempted_at, live, now))
        .take(PER_NIGHT)
    {
        match fetch_photo(db, tenant_id, p).await {
            Ok(r) if r.status == "stored" => stored += 1,
            Ok(r) if r.status == "placeholder" => placeholders += 1,
            _ => failed += 1,
        }
        if live {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    }
    Ok(serde_json::json!({
        "live": live, "stored": stored, "placeholders": placeholders, "failed": failed,
    }))
}

/// Ensure every workspace has one live nightly scan (boot + provisioning).
pub async fn ensure_recurring_jobs(db: &sea_orm::DatabaseConnection) {
    let Ok(tenants) = entity::prelude::Tenant::find().all(db).await else {
        return;
    };
    for t in tenants {
        let live = entity::prelude::BackgroundJob::find()
            .filter(entity::background_job::Column::TenantId.eq(t.id))
            .filter(entity::background_job::Column::Kind.eq(SCAN_KIND))
            .filter(entity::background_job::Column::Status.is_in([
                "pending",
                "running",
                "awaiting_callback",
            ]))
            .one(db)
            .await;
        if matches!(live, Ok(None)) {
            let _ = crate::scheduler::enqueue(db, t.id, SCAN_KIND, serde_json::json!({}), 60).await;
        }
    }
}

/// Run the scan, then sleep until tomorrow night.
pub async fn handle_scan_job(
    db: &sea_orm::DatabaseConnection,
    job: &entity::background_job::Model,
) -> crate::modules::JobOutcome {
    let summary = match scan(db, job.tenant_id).await {
        Ok(s) => s,
        Err(e) => serde_json::json!({ "error": e.to_string() }),
    };
    let mut out = crate::modules::JobOutcome::reschedule("pending", 24 * 3600);
    out.result = Some(summary);
    out
}

/// One property's photo, right after it's created.
pub async fn handle_fetch_job(
    db: &sea_orm::DatabaseConnection,
    job: &entity::background_job::Model,
) -> crate::modules::JobOutcome {
    let Some(pid) = job
        .payload
        .get("property_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    else {
        return crate::modules::JobOutcome::failed("payload missing property_id");
    };
    let property = match Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(job.tenant_id))
        .one(db)
        .await
    {
        Ok(Some(p)) => p,
        Ok(None) => return crate::modules::JobOutcome::failed("property not found"),
        Err(e) => {
            return crate::modules::JobOutcome::retry(
                crate::providers::backoff(job.attempts),
                e.to_string(),
            )
        }
    };
    if property.photo_status == "stored" {
        return crate::modules::JobOutcome::completed(
            serde_json::json!({ "skipped": "already has a photo" }),
        );
    }
    match fetch_photo(db, job.tenant_id, property).await {
        Ok(r) => crate::modules::JobOutcome::completed(serde_json::to_value(r).unwrap_or_default()),
        Err(e) => crate::modules::JobOutcome::retry(
            crate::providers::backoff(job.attempts),
            e.to_string(),
        ),
    }
}

/// Queue a photo fetch for a just-created property (best-effort).
pub async fn queue_fetch(db: &impl ConnectionTrait, tenant_id: Uuid, property_id: Uuid) {
    let _ = crate::scheduler::enqueue(
        db,
        tenant_id,
        FETCH_KIND,
        serde_json::json!({ "property_id": property_id.to_string() }),
        0,
    )
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suffixes_and_states() {
        assert_eq!(shorten_suffix("Glendale Avenue"), "Glendale Ave");
        assert_eq!(shorten_suffix("Main St"), "Main St");
        assert_eq!(state_code("California"), "CA");
        assert_eq!(state_code("ca"), "CA");
        assert_eq!(state_code("Ontario"), "Ontario");
    }

    #[test]
    fn photon_results_become_places() {
        let body = serde_json::json!({ "features": [
            { "properties": { "countrycode": "US", "housenumber": "8929", "street": "Glendale Avenue",
              "city": "Hesperia", "state": "California", "postcode": "92345" },
              "geometry": { "coordinates": [-117.3, 34.42] } },
            { "properties": { "countrycode": "US", "name": "Glendale Avenue", "city": "Hesperia", "state": "CA" },
              "geometry": { "coordinates": [-117.31, 34.43] } },
            { "properties": { "countrycode": "CA", "housenumber": "1", "street": "Yonge Street", "city": "Toronto" } }
        ]});
        let places = parse_photon(&body, "8929 glen");
        assert_eq!(places.len(), 2, "the Canadian result is dropped");
        assert_eq!(places[0].address, "8929 Glendale Ave");
        assert_eq!(places[0].state, "CA");
        assert_eq!(places[0].precision, "address");
        assert_eq!(places[0].latitude, Some(34.42));
        assert_eq!(
            places[1].address, "8929 Glendale Ave",
            "the typed number is kept"
        );
        assert_eq!(places[1].precision, "street");
        assert_eq!(places[0].label, "8929 Glendale Ave, Hesperia, CA 92345");
    }

    #[test]
    fn places_results_become_places() {
        let body = serde_json::json!({ "suggestions": [
            { "placePrediction": { "types": ["street_address", "geocode"],
              "structuredFormat": { "mainText": { "text": "8929 Glendale Avenue" },
                                    "secondaryText": { "text": "Hesperia, CA 92345, USA" } } } },
            { "placePrediction": { "types": ["establishment"],
              "structuredFormat": { "mainText": { "text": "Home Depot" }, "secondaryText": { "text": "Hesperia, CA, USA" } } } }
        ]});
        let places = parse_places(&body);
        assert_eq!(places.len(), 1, "businesses are dropped");
        assert_eq!(places[0].address, "8929 Glendale Ave");
        assert_eq!(places[0].city, "Hesperia");
        assert_eq!(places[0].state, "CA");
        assert_eq!(places[0].postal_code, "92345");
        assert_eq!(places[0].source, "google");
    }

    #[test]
    fn retry_rules() {
        let now = Utc::now();
        let old: chrono::DateTime<chrono::FixedOffset> = (now - Duration::days(8)).into();
        let recent: chrono::DateTime<chrono::FixedOffset> = (now - Duration::days(1)).into();
        assert!(wants_photo("none", None, false, now));
        assert!(!wants_photo("stored", Some(recent), true, now));
        assert!(
            wants_photo("placeholder", Some(recent), true, now),
            "a key arrived"
        );
        assert!(!wants_photo("placeholder", Some(recent), false, now));
        assert!(wants_photo("failed", Some(old), true, now));
        assert!(!wants_photo("failed", Some(recent), true, now));
    }

    #[test]
    fn placeholder_escapes_the_address() {
        let p = entity::property::Model {
            id: Uuid::nil(),
            tenant_id: Uuid::nil(),
            llc_id: None,
            portfolio_id: None,
            name: "x".into(),
            address: "1 <Main> & Co".into(),
            city: "Hesperia".into(),
            units: 1,
            occupied_units: 0,
            monthly_rent_cents: 0,
            status: "Active".into(),
            year_built: 2000,
            manager: String::new(),
            property_type: String::new(),
            strategy: String::new(),
            workflow_stage: String::new(),
            purchase_price_cents: None,
            acquired_on: None,
            image_url: None,
            state: "CA".into(),
            postal_code: "92345".into(),
            photo_status: "none".into(),
            photo_attempted_at: None,
            photo_error: None,
            created_at: Utc::now().into(),
        };
        let svg = placeholder_svg(&p);
        assert!(svg.contains("1 &lt;Main&gt; &amp; Co"));
        assert!(svg.contains("Hesperia, CA 92345"));
        assert!(svg.starts_with("<svg"));
    }
}
