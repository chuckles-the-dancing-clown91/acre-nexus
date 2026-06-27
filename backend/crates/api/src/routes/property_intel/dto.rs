//! Request/response shapes for the Property Intelligence endpoints.

use crate::dto::usd;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Label an optional cents amount as USD.
fn label(cents: Option<i64>) -> Option<String> {
    cents.map(usd)
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct PropertyDetailDto {
    pub property_id: Uuid,
    pub beds: Option<i32>,
    pub baths: Option<f64>,
    pub sqft: Option<i32>,
    pub lot_size_sqft: Option<i64>,
    pub property_type: Option<String>,
    pub stories: Option<i32>,
    pub parking_spaces: Option<i32>,
    pub heating: Option<String>,
    pub cooling: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub geocode_accuracy: Option<String>,
    pub matched_address: Option<String>,
    pub apn: Option<String>,
    pub legal_description: Option<String>,
    pub zoning: Option<String>,
    pub subdivision: Option<String>,
    pub county: Option<String>,
    pub fips: Option<String>,
    pub owner_of_record: Option<String>,
    pub last_sale_date: Option<String>,
    pub last_sale_price_cents: Option<i64>,
    pub last_sale_price_label: Option<String>,
    pub flood_zone: Option<String>,
    pub walk_score: Option<i32>,
    pub last_enriched_at: Option<String>,
}

impl From<entity::property_detail::Model> for PropertyDetailDto {
    fn from(d: entity::property_detail::Model) -> Self {
        PropertyDetailDto {
            last_sale_price_label: label(d.last_sale_price_cents),
            property_id: d.property_id,
            beds: d.beds,
            baths: d.baths,
            sqft: d.sqft,
            lot_size_sqft: d.lot_size_sqft,
            property_type: d.property_type,
            stories: d.stories,
            parking_spaces: d.parking_spaces,
            heating: d.heating,
            cooling: d.cooling,
            latitude: d.latitude,
            longitude: d.longitude,
            geocode_accuracy: d.geocode_accuracy,
            matched_address: d.matched_address,
            apn: d.apn,
            legal_description: d.legal_description,
            zoning: d.zoning,
            subdivision: d.subdivision,
            county: d.county,
            fips: d.fips,
            owner_of_record: d.owner_of_record,
            last_sale_date: d.last_sale_date,
            last_sale_price_cents: d.last_sale_price_cents,
            flood_zone: d.flood_zone,
            walk_score: d.walk_score,
            last_enriched_at: d.last_enriched_at.map(|t| t.to_rfc3339()),
        }
    }
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct TaxDto {
    pub tax_year: i32,
    pub assessed_value_cents: Option<i64>,
    pub assessed_value_label: Option<String>,
    pub tax_amount_cents: Option<i64>,
    pub tax_amount_label: Option<String>,
    pub tax_rate_bps: Option<i32>,
    pub source: String,
}

impl From<entity::property_tax::Model> for TaxDto {
    fn from(t: entity::property_tax::Model) -> Self {
        TaxDto {
            assessed_value_label: label(t.assessed_value_cents),
            tax_amount_label: label(t.tax_amount_cents),
            tax_year: t.tax_year,
            assessed_value_cents: t.assessed_value_cents,
            tax_amount_cents: t.tax_amount_cents,
            tax_rate_bps: t.tax_rate_bps,
            source: t.source,
        }
    }
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ValuationDto {
    pub as_of: String,
    pub estimated_value_cents: Option<i64>,
    pub estimated_value_label: Option<String>,
    pub value_low_cents: Option<i64>,
    pub value_high_cents: Option<i64>,
    pub estimated_rent_cents: Option<i64>,
    pub estimated_rent_label: Option<String>,
    pub confidence: Option<i32>,
    pub source: String,
}

impl From<entity::property_valuation::Model> for ValuationDto {
    fn from(v: entity::property_valuation::Model) -> Self {
        ValuationDto {
            estimated_value_label: label(v.estimated_value_cents),
            estimated_rent_label: label(v.estimated_rent_cents),
            as_of: v.as_of,
            estimated_value_cents: v.estimated_value_cents,
            value_low_cents: v.value_low_cents,
            value_high_cents: v.value_high_cents,
            estimated_rent_cents: v.estimated_rent_cents,
            confidence: v.confidence,
            source: v.source,
        }
    }
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct SchoolDto {
    pub name: String,
    pub level: String,
    pub district: Option<String>,
    pub rating: Option<i32>,
    pub distance_mi: Option<f64>,
    pub grades: Option<String>,
}

impl From<entity::property_school::Model> for SchoolDto {
    fn from(s: entity::property_school::Model) -> Self {
        SchoolDto {
            name: s.name,
            level: s.level,
            district: s.district,
            rating: s.rating,
            distance_mi: s.distance_mi,
            grades: s.grades,
        }
    }
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct UtilityDto {
    pub utility_type: String,
    pub provider: String,
    pub est_monthly_cost_cents: Option<i64>,
    pub est_monthly_cost_label: Option<String>,
    pub phone: Option<String>,
}

impl From<entity::property_utility::Model> for UtilityDto {
    fn from(u: entity::property_utility::Model) -> Self {
        UtilityDto {
            est_monthly_cost_label: label(u.est_monthly_cost_cents),
            utility_type: u.utility_type,
            provider: u.provider,
            est_monthly_cost_cents: u.est_monthly_cost_cents,
            phone: u.phone,
        }
    }
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct EnrichmentRunDto {
    pub id: Uuid,
    pub source: String,
    pub status: String,
    pub provider: String,
    pub job_id: Option<Uuid>,
    pub detail: Option<serde_json::Value>,
    pub created_at: String,
}

impl From<entity::enrichment_run::Model> for EnrichmentRunDto {
    fn from(r: entity::enrichment_run::Model) -> Self {
        EnrichmentRunDto {
            id: r.id,
            source: r.source,
            status: r.status,
            provider: r.provider,
            job_id: r.job_id,
            detail: r.detail,
            created_at: r.created_at.to_rfc3339(),
        }
    }
}

/// Aggregated property-intelligence payload for the detail page.
#[derive(Serialize, schemars::JsonSchema)]
pub struct IntelResp {
    pub detail: Option<PropertyDetailDto>,
    pub valuations: Vec<ValuationDto>,
    pub taxes: Vec<TaxDto>,
    pub schools: Vec<SchoolDto>,
    pub utilities: Vec<UtilityDto>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct EnrichReq {
    /// Sources to refresh; omit/empty to refresh all
    /// (`geocode`, `parcel`, `tax`, `valuation`, `schools`, `utilities`).
    #[serde(default)]
    pub sources: Vec<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct EnrichResp {
    /// The orchestrator job enqueued on the durable queue.
    pub job_id: Uuid,
    /// The sources that will be refreshed.
    pub scheduled: Vec<String>,
}
