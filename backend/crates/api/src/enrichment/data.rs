//! Plain-data shapes returned by enrichment providers, plus the engine's error
//! type. Providers (live or simulated) produce these; [`super::runner`] persists
//! them to the property-data tables.

use std::fmt;

/// An enrichment failure (transport error, no match, bad payload …). Carries a
/// human-readable message that is surfaced on the `enrichment_run` and the job's
/// `last_error`.
#[derive(Debug, Clone)]
pub struct EnrichmentError(pub String);

impl fmt::Display for EnrichmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for EnrichmentError {}

/// Convenience: build an [`EnrichmentError`] from anything stringy.
pub fn err(msg: impl Into<String>) -> EnrichmentError {
    EnrichmentError(msg.into())
}

/// Coordinates + match quality from the geocoder. The Census *geographies*
/// endpoint also returns the real county + county FIPS; the simulated fallback
/// leaves those `None`.
pub struct GeoData {
    pub latitude: f64,
    pub longitude: f64,
    pub matched_address: String,
    pub accuracy: String,
    pub county: Option<String>,
    pub fips: Option<String>,
}

/// Parcel / county-record attributes.
pub struct ParcelData {
    pub apn: String,
    pub zoning: String,
    pub subdivision: String,
    pub county: String,
    pub fips: String,
    pub owner_of_record: String,
    pub last_sale_date: String,
    pub last_sale_price_cents: i64,
    pub lot_size_sqft: i64,
    pub property_type: String,
    pub beds: i32,
    pub baths: f64,
    pub sqft: i32,
    pub stories: i32,
    pub parking_spaces: i32,
    pub heating: String,
    pub cooling: String,
    pub flood_zone: String,
    pub walk_score: i32,
    pub legal_description: String,
}

/// One year of tax-assessment data.
pub struct TaxYear {
    pub tax_year: i32,
    pub assessed_value_cents: i64,
    pub land_value_cents: i64,
    pub improvement_value_cents: i64,
    pub tax_amount_cents: i64,
    pub tax_rate_bps: i32,
}

/// An automated valuation snapshot.
pub struct ValuationData {
    pub as_of: String,
    pub estimated_value_cents: i64,
    pub value_low_cents: i64,
    pub value_high_cents: i64,
    pub estimated_rent_cents: i64,
    pub confidence: i32,
}

/// A school assigned to / near the property.
pub struct SchoolData {
    pub name: String,
    pub level: String,
    pub district: String,
    pub rating: i32,
    pub distance_mi: f64,
    pub grades: String,
}

/// A utility servicing the property.
pub struct UtilityData {
    pub utility_type: String,
    pub provider: String,
    pub est_monthly_cost_cents: i64,
    pub phone: String,
}

/// One offense category's rates per 100,000 people over the period.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct CrimeOffense {
    /// `violent-crime` | `property-crime` | `burglary` | `motor-vehicle-theft`.
    pub key: String,
    pub label: String,
    /// Offenses the agency reported over the period.
    pub agency: i64,
    pub agency_rate: f64,
    pub state_rate: f64,
    pub us_rate: f64,
    /// The agency's rate over the year before, when the data goes back that far.
    pub prior_rate: Option<f64>,
}

/// Crime statistics for a property's area.
pub struct CrimeData {
    pub agency_ori: Option<String>,
    pub agency_name: String,
    pub agency_km: Option<f64>,
    pub period_from: String,
    pub period_to: String,
    pub population: Option<i64>,
    pub offenses: Vec<CrimeOffense>,
}

impl CrimeData {
    /// How the area compares with its state on violent plus property crime:
    /// `well_below` (under 60%), `below`, `about` (within 15%), `above`,
    /// `well_above` (over 160%), or `unknown`.
    pub fn verdict(&self) -> &'static str {
        let (mut agency, mut state) = (0.0, 0.0);
        for o in &self.offenses {
            if o.key == "violent-crime" || o.key == "property-crime" {
                agency += o.agency_rate;
                state += o.state_rate;
            }
        }
        if state <= 0.0 {
            return "unknown";
        }
        let ratio = agency / state;
        if ratio < 0.6 {
            "well_below"
        } else if ratio < 0.85 {
            "below"
        } else if ratio <= 1.15 {
            "about"
        } else if ratio <= 1.6 {
            "above"
        } else {
            "well_above"
        }
    }
}

/// The verdict in words.
pub fn verdict_words(v: &str) -> &'static str {
    match v {
        "well_below" => "Well below the state average",
        "below" => "Below the state average",
        "about" => "About the state average",
        "above" => "Above the state average",
        "well_above" => "Well above the state average",
        _ => "No comparison yet",
    }
}
