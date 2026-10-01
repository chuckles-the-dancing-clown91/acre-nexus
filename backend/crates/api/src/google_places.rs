//! The business on Google: finding its place, and its rating and reviews.
//!
//! Places API (New), with the workspace's Google Maps key from the vault (or
//! the platform's). Google's terms let us keep only the place id, so reviews
//! are never written to the database: they are fetched when needed and held
//! in memory for the workspace's refresh interval. Place Details returns at
//! most five reviews. Without a live key the calls answer with clearly marked
//! sample data, the same sandbox-first posture as the other providers.

use crate::error::{ApiError, ApiResult};
use crate::geo;
use sea_orm::ConnectionTrait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use uuid::Uuid;

const PLACES: &str = "https://places.googleapis.com/v1/places";
const SEARCH_FIELDS: &str =
    "places.id,places.displayName,places.formattedAddress,places.rating,places.userRatingCount,places.googleMapsUri";
const DETAIL_FIELDS: &str =
    "id,displayName,formattedAddress,rating,userRatingCount,googleMapsUri,reviews";

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PlaceCandidate {
    pub place_id: String,
    pub name: String,
    pub address: String,
    pub rating: Option<f64>,
    pub count: i64,
    pub maps_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Review {
    pub author: String,
    pub author_url: String,
    pub photo_url: String,
    pub rating: i32,
    pub text: String,
    pub when: String,
    pub review_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PlaceInfo {
    pub place_id: String,
    pub name: String,
    pub address: String,
    pub rating: Option<f64>,
    pub count: i64,
    pub maps_url: String,
    pub reviews: Vec<Review>,
    /// Sample data: no live Google key yet.
    pub simulated: bool,
}

type Cache = Mutex<HashMap<(Uuid, String), (Instant, PlaceInfo)>>;

fn cache() -> &'static Cache {
    static C: OnceLock<Cache> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Drop a workspace's cached place (after the place or key changes).
pub fn forget(tenant_id: Uuid) {
    if let Ok(mut c) = cache().lock() {
        c.retain(|(t, _), _| *t != tenant_id);
    }
}

/// The "write a review" link for a place.
pub fn write_review_url(place_id: &str) -> String {
    format!("https://search.google.com/local/writereview?placeid={place_id}")
}

fn google_error(what: &str, status: reqwest::StatusCode, body: &str) -> ApiError {
    let detail = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(str::to_string))
        .unwrap_or_default();
    tracing::warn!("google places {what} returned {status}: {detail}");
    ApiError::BadRequest(if detail.is_empty() {
        format!("Google answered {status}.")
    } else {
        format!("Google said: {detail}")
    })
}

async fn client() -> ApiResult<reqwest::Client> {
    crate::providers::client::build_http_client()
        .map_err(|e| ApiError::Internal(anyhow::anyhow!(e.0)))
}

/// Places matching a business name and town, to pick the business from.
pub async fn search(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    query: &str,
) -> ApiResult<Vec<PlaceCandidate>> {
    let q = query.trim();
    if q.len() < 2 {
        return Ok(vec![]);
    }
    if !geo::google_live(db, tenant_id).await {
        return Ok(vec![PlaceCandidate {
            place_id: "sample-place".into(),
            name: q.to_string(),
            address: "123 Main St (sample)".into(),
            rating: Some(4.8),
            count: 127,
            maps_url: "https://maps.google.com/".into(),
        }]);
    }
    let key = geo::maps_key(db, tenant_id).await.unwrap_or_default();
    let resp = client()
        .await?
        .post(format!("{PLACES}:searchText"))
        .header("X-Goog-Api-Key", key)
        .header("X-Goog-FieldMask", SEARCH_FIELDS)
        .json(&serde_json::json!({ "textQuery": q, "maxResultCount": 5 }))
        .send()
        .await
        .map_err(|e| ApiError::BadRequest(format!("Google did not answer: {e}")))?;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(google_error("search", status, &body));
    }
    let v: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    Ok(v["places"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|p| p["id"].is_string())
                .map(|p| PlaceCandidate {
                    place_id: p["id"].as_str().unwrap_or_default().into(),
                    name: p["displayName"]["text"].as_str().unwrap_or_default().into(),
                    address: p["formattedAddress"].as_str().unwrap_or_default().into(),
                    rating: p["rating"].as_f64(),
                    count: p["userRatingCount"].as_i64().unwrap_or(0),
                    maps_url: p["googleMapsUri"].as_str().unwrap_or_default().into(),
                })
                .collect()
        })
        .unwrap_or_default())
}

fn parse_review(r: &serde_json::Value) -> Review {
    let a = &r["authorAttribution"];
    Review {
        author: a["displayName"].as_str().unwrap_or("A Google user").into(),
        author_url: a["uri"].as_str().unwrap_or_default().into(),
        photo_url: a["photoUri"].as_str().unwrap_or_default().into(),
        rating: r["rating"].as_i64().unwrap_or(0) as i32,
        text: r["text"]["text"]
            .as_str()
            .or_else(|| r["originalText"]["text"].as_str())
            .unwrap_or_default()
            .trim()
            .into(),
        when: r["relativePublishTimeDescription"]
            .as_str()
            .unwrap_or_default()
            .into(),
        review_url: r["googleMapsUri"].as_str().unwrap_or_default().into(),
    }
}

fn sample(place_id: &str, name: &str) -> PlaceInfo {
    let r = |author: &str, rating: i32, text: &str, when: &str| Review {
        author: author.into(),
        author_url: String::new(),
        photo_url: String::new(),
        rating,
        text: text.into(),
        when: when.into(),
        review_url: String::new(),
    };
    PlaceInfo {
        place_id: place_id.into(),
        name: if name.is_empty() {
            "Your business".into()
        } else {
            name.into()
        },
        address: "123 Main St (sample)".into(),
        rating: Some(4.8),
        count: 127,
        maps_url: "https://maps.google.com/".into(),
        reviews: vec![
            r(
                "Dana R.",
                5,
                "Fast repairs and a manager who actually calls back.",
                "a month ago",
            ),
            r(
                "Chris M.",
                5,
                "Moving in was easy and the unit was spotless.",
                "2 months ago",
            ),
            r("Sam T.", 3, "Good place, parking is tight.", "3 months ago"),
        ],
        simulated: true,
    }
}

/// The place's rating, count and (up to five) reviews, from memory when
/// fresh. `minutes` is the workspace's refresh interval.
pub async fn place(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    place_id: &str,
    name: &str,
    minutes: i32,
    refresh: bool,
) -> ApiResult<PlaceInfo> {
    let key = (tenant_id, place_id.to_string());
    if !refresh {
        if let Ok(c) = cache().lock() {
            if let Some((at, info)) = c.get(&key) {
                if at.elapsed() < Duration::from_secs(minutes.max(1) as u64 * 60) {
                    return Ok(info.clone());
                }
            }
        }
    }
    let info = if !geo::google_live(db, tenant_id).await {
        sample(place_id, name)
    } else {
        let api_key = geo::maps_key(db, tenant_id).await.unwrap_or_default();
        let resp = client()
            .await?
            .get(format!("{PLACES}/{}?languageCode=en", urlencode(place_id)))
            .header("X-Goog-Api-Key", api_key)
            .header("X-Goog-FieldMask", DETAIL_FIELDS)
            .send()
            .await
            .map_err(|e| ApiError::BadRequest(format!("Google did not answer: {e}")))?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(google_error("details", status, &body));
        }
        let v: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
        PlaceInfo {
            place_id: v["id"].as_str().unwrap_or(place_id).into(),
            name: v["displayName"]["text"].as_str().unwrap_or_default().into(),
            address: v["formattedAddress"].as_str().unwrap_or_default().into(),
            rating: v["rating"].as_f64(),
            count: v["userRatingCount"].as_i64().unwrap_or(0),
            maps_url: v["googleMapsUri"].as_str().unwrap_or_default().into(),
            reviews: v["reviews"]
                .as_array()
                .map(|a| a.iter().map(parse_review).collect())
                .unwrap_or_default(),
            simulated: false,
        }
    };
    if let Ok(mut c) = cache().lock() {
        c.insert(key, (Instant::now(), info.clone()));
    }
    Ok(info)
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn place_ids_are_percent_encoded() {
        assert_eq!(urlencode("ChIJ-abc_1.~"), "ChIJ-abc_1.~");
        assert_eq!(urlencode("a b/c"), "a%20b%2Fc");
    }

    #[test]
    fn review_link_carries_the_place() {
        assert!(write_review_url("abc").ends_with("placeid=abc"));
    }

    #[test]
    fn sample_is_marked() {
        let s = sample("p", "Northwind");
        assert!(s.simulated);
        assert_eq!(s.name, "Northwind");
        assert!(!s.reviews.is_empty());
    }
}
