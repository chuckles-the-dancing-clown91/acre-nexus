//! Rich, listing-grade physical + parcel attributes for a [`super::property`],
//! populated and validated by the **enrichment engine** (county-record / parcel
//! providers and the live geocoder). One row per property (PK = `property_id`).
//!
//! Everything here is nullable: a property starts as a thin record and is
//! progressively enriched, so each field is "best known value, or unknown".

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "property_detail")]
pub struct Model {
    /// 1:1 with `property`.
    #[sea_orm(primary_key, auto_increment = false)]
    pub property_id: Uuid,
    pub tenant_id: Uuid,
    // ---- Physical ----
    pub beds: Option<i32>,
    /// Bathrooms (supports halves, e.g. 2.5).
    pub baths: Option<f64>,
    pub sqft: Option<i32>,
    pub lot_size_sqft: Option<i64>,
    /// `single_family` | `multi_family` | `condo` | `townhome` | `commercial` …
    pub property_type: Option<String>,
    pub stories: Option<i32>,
    pub parking_spaces: Option<i32>,
    pub heating: Option<String>,
    pub cooling: Option<String>,
    // ---- Geo (from the live geocoder) ----
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    /// Normalised, match-quality label from the geocoder (e.g. `Exact`).
    pub geocode_accuracy: Option<String>,
    pub matched_address: Option<String>,
    // ---- Parcel / county record ----
    pub apn: Option<String>,
    pub legal_description: Option<String>,
    pub zoning: Option<String>,
    pub subdivision: Option<String>,
    pub county: Option<String>,
    /// Census FIPS / GEOID for the parcel's tract.
    pub fips: Option<String>,
    pub owner_of_record: Option<String>,
    pub last_sale_date: Option<String>,
    pub last_sale_price_cents: Option<i64>,
    // ---- Hazards / neighborhood ----
    pub flood_zone: Option<String>,
    pub walk_score: Option<i32>,
    /// When the enrichment engine last refreshed this row.
    pub last_enriched_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
