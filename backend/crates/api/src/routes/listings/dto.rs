//! Shapes for console listing management.

use crate::dto::usd;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A listing as the console sees it (includes visibility, unlike the public DTO).
#[derive(Serialize, schemars::JsonSchema)]
pub struct ConsoleListingResp {
    pub id: Uuid,
    pub property_id: Option<Uuid>,
    pub title: String,
    pub address: String,
    pub city: String,
    pub beds: i32,
    pub baths: i32,
    pub sqft: i32,
    pub rent_cents: i64,
    pub rent_label: String,
    /// `Available` | `New` | `Pending` | `Leased`.
    pub status: String,
    pub available_on: String,
    pub description: String,
    /// Whether the listing shows on the public website.
    pub is_public: bool,
    pub created_at: String,
}

impl From<entity::listing::Model> for ConsoleListingResp {
    fn from(l: entity::listing::Model) -> Self {
        ConsoleListingResp {
            rent_label: usd(l.rent_cents),
            id: l.id,
            property_id: l.property_id,
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
            is_public: l.is_public,
            created_at: l.created_at.to_rfc3339(),
        }
    }
}

/// Create a listing for a property. Address/city always come from the
/// property; beds/baths/sqft default from its enrichment detail when known.
#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateListingReq {
    pub title: Option<String>,
    pub rent_cents: i64,
    pub beds: Option<i32>,
    pub baths: Option<i32>,
    pub sqft: Option<i32>,
    /// Human availability label, e.g. `Now` or `Aug 1`.
    pub available_on: Option<String>,
    pub description: Option<String>,
    /// Defaults to public (visible on the website).
    pub is_public: Option<bool>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateListingReq {
    pub title: Option<String>,
    pub rent_cents: Option<i64>,
    pub beds: Option<i32>,
    pub baths: Option<i32>,
    pub sqft: Option<i32>,
    pub available_on: Option<String>,
    pub description: Option<String>,
    /// `Available` | `New` | `Pending` | `Leased`.
    pub status: Option<String>,
    pub is_public: Option<bool>,
}
