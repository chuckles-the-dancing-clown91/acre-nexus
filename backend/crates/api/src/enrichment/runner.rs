//! Runs a single enrichment [`Source`] for a property: call the provider (live
//! or simulated), persist the result to the property-data tables, and return a
//! [`SourceOutcome`] (summary + which provider actually served it). A live
//! provider that is unavailable **falls back to simulation** rather than erroring
//! — only real failures (e.g. the database) return `Err`, which the scheduler
//! retries/fails. The caller ([`crate::modules`]) records the `enrichment_run`.

use super::data::{err, EnrichmentError};
use super::source::Source;
use super::{geocode, simulated};
use chrono::{Datelike, Utc};
use entity::prelude::*;
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use serde_json::{json, Value};
use uuid::Uuid;

/// Map a DB error into an [`EnrichmentError`].
fn db_err(e: sea_orm::DbErr) -> EnrichmentError {
    err(format!("db error: {e}"))
}

/// The result of running one source: the data summary plus **which provider
/// actually produced it**. `fell_back` is set when a live provider was
/// attempted but was unavailable and simulation stood in — the graceful-fallback
/// path is a success (the property still gets enriched), just an observable one.
pub struct SourceOutcome {
    pub summary: Value,
    pub provider: String,
    pub fell_back: bool,
    pub reason: Option<String>,
}

impl SourceOutcome {
    /// A source served by its deterministic simulated provider (no live attempt).
    fn simulated(summary: Value) -> Self {
        SourceOutcome {
            summary,
            provider: "simulated".into(),
            fell_back: false,
            reason: None,
        }
    }
}

/// Run one source against `property`, persisting results. Returns the outcome
/// (summary + provider used). A returned `Err` is a *real* failure (e.g. the
/// database) that the scheduler should retry — provider unavailability is not an
/// error, it falls back to simulation and returns `Ok` with `fell_back = true`.
pub async fn run_source<C: ConnectionTrait>(
    db: &C,
    property: &entity::property::Model,
    source: Source,
) -> Result<SourceOutcome, EnrichmentError> {
    match source {
        Source::Geocode => run_geocode(db, property).await,
        Source::Parcel => run_parcel(db, property).await,
        Source::Tax => run_tax(db, property).await,
        Source::Valuation => run_valuation(db, property).await,
        Source::Schools => run_schools(db, property).await,
        Source::Utilities => run_utilities(db, property).await,
    }
}

/// The one-line address handed to the geocoder.
fn full_address(p: &entity::property::Model) -> String {
    format!("{}, {}", p.address, p.city)
}

// ---------------------------------------------------------------------------
// property_detail upsert
// ---------------------------------------------------------------------------

/// Load the property's detail row, creating a blank one if absent.
async fn load_or_init_detail<C: ConnectionTrait>(
    db: &C,
    property: &entity::property::Model,
) -> Result<entity::property_detail::Model, EnrichmentError> {
    if let Some(m) = PropertyDetail::find_by_id(property.id)
        .one(db)
        .await
        .map_err(db_err)?
    {
        return Ok(m);
    }
    let now = Utc::now();
    let model = entity::property_detail::ActiveModel {
        property_id: Set(property.id),
        tenant_id: Set(property.tenant_id),
        beds: Set(None),
        baths: Set(None),
        sqft: Set(None),
        lot_size_sqft: Set(None),
        property_type: Set(None),
        stories: Set(None),
        parking_spaces: Set(None),
        heating: Set(None),
        cooling: Set(None),
        latitude: Set(None),
        longitude: Set(None),
        geocode_accuracy: Set(None),
        matched_address: Set(None),
        apn: Set(None),
        legal_description: Set(None),
        zoning: Set(None),
        subdivision: Set(None),
        county: Set(None),
        fips: Set(None),
        owner_of_record: Set(None),
        last_sale_date: Set(None),
        last_sale_price_cents: Set(None),
        flood_zone: Set(None),
        walk_score: Set(None),
        last_enriched_at: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    };
    model.insert(db).await.map_err(db_err)
}

async fn run_geocode<C: ConnectionTrait>(
    db: &C,
    property: &entity::property::Model,
) -> Result<SourceOutcome, EnrichmentError> {
    // Attempt the live Census geocoder; on any provider failure fall back to a
    // deterministic simulated geocode so enrichment still succeeds.
    let (geo, provider, fell_back, reason) = match geocode::geocode(&full_address(property)).await {
        Ok(g) => (g, "census_geocoder", false, None),
        Err(e) => {
            tracing::warn!(
                "live geocoder unavailable for {} ({e}); falling back to simulation",
                property.id
            );
            let g = simulated::geocode(property.id, &property.address, &property.city);
            (g, "simulated", true, Some(e.to_string()))
        }
    };

    let detail = load_or_init_detail(db, property).await?;
    let mut am: entity::property_detail::ActiveModel = detail.into();
    am.latitude = Set(Some(geo.latitude));
    am.longitude = Set(Some(geo.longitude));
    am.geocode_accuracy = Set(Some(geo.accuracy.clone()));
    am.matched_address = Set(Some(geo.matched_address.clone()));
    // Only stamp county/FIPS when the live layer resolved them — never clobber a
    // real value (or a simulated parcel's) with the fallback's `None`.
    if let Some(county) = geo.county.clone() {
        am.county = Set(Some(county));
    }
    if let Some(fips) = geo.fips.clone() {
        am.fips = Set(Some(fips));
    }
    am.last_enriched_at = Set(Some(Utc::now().into()));
    am.updated_at = Set(Utc::now().into());
    am.update(db).await.map_err(db_err)?;

    Ok(SourceOutcome {
        summary: json!({
            "latitude": geo.latitude,
            "longitude": geo.longitude,
            "matched_address": geo.matched_address,
            "county": geo.county,
            "fips": geo.fips,
        }),
        provider: provider.to_string(),
        fell_back,
        reason,
    })
}

async fn run_parcel<C: ConnectionTrait>(
    db: &C,
    property: &entity::property::Model,
) -> Result<SourceOutcome, EnrichmentError> {
    let mut rng = simulated::rng_for(property.id, &property.address);
    let parcel = simulated::parcel(&mut rng, &property.city, property.year_built);
    let detail = load_or_init_detail(db, property).await?;
    // Preserve real county / FIPS if the live geocoder already resolved them.
    let existing_county = detail.county.clone();
    let existing_fips = detail.fips.clone();
    let mut am: entity::property_detail::ActiveModel = detail.into();
    am.apn = Set(Some(parcel.apn.clone()));
    am.zoning = Set(Some(parcel.zoning));
    am.subdivision = Set(Some(parcel.subdivision));
    am.county = Set(Some(existing_county.unwrap_or(parcel.county)));
    am.fips = Set(Some(existing_fips.unwrap_or(parcel.fips)));
    am.owner_of_record = Set(Some(parcel.owner_of_record));
    am.last_sale_date = Set(Some(parcel.last_sale_date));
    am.last_sale_price_cents = Set(Some(parcel.last_sale_price_cents));
    am.lot_size_sqft = Set(Some(parcel.lot_size_sqft));
    am.property_type = Set(Some(parcel.property_type));
    am.beds = Set(Some(parcel.beds));
    am.baths = Set(Some(parcel.baths));
    am.sqft = Set(Some(parcel.sqft));
    am.stories = Set(Some(parcel.stories));
    am.parking_spaces = Set(Some(parcel.parking_spaces));
    am.heating = Set(Some(parcel.heating));
    am.cooling = Set(Some(parcel.cooling));
    am.flood_zone = Set(Some(parcel.flood_zone));
    am.walk_score = Set(Some(parcel.walk_score));
    am.legal_description = Set(Some(parcel.legal_description));
    am.last_enriched_at = Set(Some(Utc::now().into()));
    am.updated_at = Set(Utc::now().into());
    am.update(db).await.map_err(db_err)?;
    Ok(SourceOutcome::simulated(json!({ "apn": parcel.apn })))
}

async fn run_tax<C: ConnectionTrait>(
    db: &C,
    property: &entity::property::Model,
) -> Result<SourceOutcome, EnrichmentError> {
    let mut rng = simulated::rng_for(property.id, &property.address);
    let base_value = (property.monthly_rent_cents as f64 * 150.0) as i64;
    let base_assessed = (base_value as f64 * 0.85) as i64;
    let years = simulated::taxes(&mut rng, Utc::now().year(), base_assessed, 3);

    // Idempotent: replace any existing tax rows for this property.
    PropertyTax::delete_many()
        .filter(entity::property_tax::Column::PropertyId.eq(property.id))
        .exec(db)
        .await
        .map_err(db_err)?;
    for t in &years {
        entity::property_tax::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(property.tenant_id),
            property_id: Set(property.id),
            tax_year: Set(t.tax_year),
            assessed_value_cents: Set(Some(t.assessed_value_cents)),
            land_value_cents: Set(Some(t.land_value_cents)),
            improvement_value_cents: Set(Some(t.improvement_value_cents)),
            tax_amount_cents: Set(Some(t.tax_amount_cents)),
            tax_rate_bps: Set(Some(t.tax_rate_bps)),
            source: Set(Source::Tax.provider().to_string()),
            created_at: Set(Utc::now().into()),
        }
        .insert(db)
        .await
        .map_err(db_err)?;
    }
    Ok(SourceOutcome::simulated(json!({ "years": years.len() })))
}

async fn run_valuation<C: ConnectionTrait>(
    db: &C,
    property: &entity::property::Model,
) -> Result<SourceOutcome, EnrichmentError> {
    let mut rng = simulated::rng_for(property.id, &property.address);
    let base_value = (property.monthly_rent_cents as f64 * 150.0) as i64;
    let as_of = Utc::now().format("%Y-%m-%d").to_string();
    let v = simulated::valuation(&mut rng, as_of, base_value, property.monthly_rent_cents);
    entity::property_valuation::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(property.tenant_id),
        property_id: Set(property.id),
        as_of: Set(v.as_of.clone()),
        estimated_value_cents: Set(Some(v.estimated_value_cents)),
        value_low_cents: Set(Some(v.value_low_cents)),
        value_high_cents: Set(Some(v.value_high_cents)),
        estimated_rent_cents: Set(Some(v.estimated_rent_cents)),
        confidence: Set(Some(v.confidence)),
        source: Set(Source::Valuation.provider().to_string()),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await
    .map_err(db_err)?;
    Ok(SourceOutcome::simulated(json!({
        "estimated_value_cents": v.estimated_value_cents,
        "estimated_rent_cents": v.estimated_rent_cents,
    })))
}

async fn run_schools<C: ConnectionTrait>(
    db: &C,
    property: &entity::property::Model,
) -> Result<SourceOutcome, EnrichmentError> {
    let mut rng = simulated::rng_for(property.id, &property.address);
    let schools = simulated::schools(&mut rng);
    PropertySchool::delete_many()
        .filter(entity::property_school::Column::PropertyId.eq(property.id))
        .exec(db)
        .await
        .map_err(db_err)?;
    for s in &schools {
        entity::property_school::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(property.tenant_id),
            property_id: Set(property.id),
            name: Set(s.name.clone()),
            level: Set(s.level.clone()),
            district: Set(Some(s.district.clone())),
            rating: Set(Some(s.rating)),
            distance_mi: Set(Some(s.distance_mi)),
            grades: Set(Some(s.grades.clone())),
            source: Set(Source::Schools.provider().to_string()),
            created_at: Set(Utc::now().into()),
        }
        .insert(db)
        .await
        .map_err(db_err)?;
    }
    Ok(SourceOutcome::simulated(
        json!({ "schools": schools.len() }),
    ))
}

async fn run_utilities<C: ConnectionTrait>(
    db: &C,
    property: &entity::property::Model,
) -> Result<SourceOutcome, EnrichmentError> {
    let mut rng = simulated::rng_for(property.id, &property.address);
    let utils = simulated::utilities(&mut rng);
    PropertyUtility::delete_many()
        .filter(entity::property_utility::Column::PropertyId.eq(property.id))
        .exec(db)
        .await
        .map_err(db_err)?;
    for u in &utils {
        entity::property_utility::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(property.tenant_id),
            property_id: Set(property.id),
            utility_type: Set(u.utility_type.clone()),
            provider: Set(u.provider.clone()),
            est_monthly_cost_cents: Set(Some(u.est_monthly_cost_cents)),
            phone: Set(Some(u.phone.clone())),
            source: Set(Source::Utilities.provider().to_string()),
            created_at: Set(Utc::now().into()),
        }
        .insert(db)
        .await
        .map_err(db_err)?;
    }
    Ok(SourceOutcome::simulated(
        json!({ "utilities": utils.len() }),
    ))
}
