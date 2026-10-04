//! Runs a single enrichment [`Source`] for a property: call the provider (live
//! or simulated), persist the result to the property-data tables, and return a
//! [`SourceOutcome`] (summary + which provider actually served it). A live
//! provider that is unavailable **falls back to simulation** rather than erroring
//! — only real failures (e.g. the database) return `Err`, which the scheduler
//! retries/fails. The caller ([`crate::modules`]) records the `enrichment_run`.

use super::data::ParcelData;
use super::data::{err, EnrichmentError};
use super::source::Source;
use super::{crime, geocode, live, rentcast, simulated};
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
        Source::Crime => run_crime(db, property).await,
    }
}

/// The one-line address with the state and ZIP, for a records provider.
fn postal_address(p: &entity::property::Model) -> String {
    let (city, state) = city_state(&p.city, &p.state, None);
    format!("{}, {city}, {state} {}", p.address, p.postal_code)
        .trim()
        .trim_end_matches(',')
        .to_string()
}

/// The city and two-letter state, even when the state field is blank and
/// the city reads "Portland, OR", or only the geocoder's matched address
/// ("… PORTLAND, OR, 97201") knows the state.
pub fn city_state(raw_city: &str, raw_state: &str, matched: Option<&str>) -> (String, String) {
    let mut city = raw_city.trim().to_string();
    let mut state = crate::geo::state_code(raw_state);
    if let Some((c, rest)) = raw_city.split_once(',') {
        let code = crate::geo::state_code(rest.split_whitespace().next().unwrap_or(""));
        if code.len() == 2 {
            city = c.trim().to_string();
            if state.len() != 2 {
                state = code;
            }
        }
    }
    if state.len() != 2 {
        if let Some(m) = matched {
            let parts: Vec<&str> = m.split(',').map(str::trim).collect();
            if parts.len() >= 3 {
                let code = crate::geo::state_code(parts[parts.len() - 2]);
                if code.len() == 2 {
                    state = code;
                }
            }
        }
    }
    (city, state)
}

/// What a live records call lacks is filled from what's already on file.
fn parcel_on_file(d: &entity::property_detail::Model) -> ParcelData {
    ParcelData {
        apn: d.apn.clone().unwrap_or_default(),
        zoning: d.zoning.clone().unwrap_or_default(),
        subdivision: d.subdivision.clone().unwrap_or_default(),
        county: d.county.clone().unwrap_or_default(),
        fips: d.fips.clone().unwrap_or_default(),
        owner_of_record: d.owner_of_record.clone().unwrap_or_default(),
        last_sale_date: d.last_sale_date.clone().unwrap_or_default(),
        last_sale_price_cents: d.last_sale_price_cents.unwrap_or(0),
        lot_size_sqft: d.lot_size_sqft.unwrap_or(0),
        year_built: d.year_built.unwrap_or(0),
        property_type: d.property_type.clone().unwrap_or_default(),
        beds: d.beds.unwrap_or(0),
        baths: d.baths.unwrap_or(0.0),
        sqft: d.sqft.unwrap_or(0),
        stories: d.stories.unwrap_or(0),
        parking_spaces: d.parking_spaces.unwrap_or(0),
        heating: d.heating.clone().unwrap_or_default(),
        cooling: d.cooling.clone().unwrap_or_default(),
        flood_zone: d.flood_zone.clone().unwrap_or_default(),
        walk_score: d.walk_score.unwrap_or(0),
        legal_description: d.legal_description.clone().unwrap_or_default(),
    }
}

fn opt(s: String) -> Option<String> {
    Some(s).filter(|x| !x.is_empty())
}

/// The one-line address handed to the geocoder.
fn full_address(p: &entity::property::Model) -> String {
    format!("{}, {}", p.address, p.city)
}

// ---------------------------------------------------------------------------
// property_detail upsert
// ---------------------------------------------------------------------------

/// Load the property's detail row, creating a blank one if absent.
pub(crate) async fn load_or_init_detail<C: ConnectionTrait>(
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
        year_built: Set(None),
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
        description: Set(None),
        features: Set(serde_json::json!({})),
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
    let detail = load_or_init_detail(db, property).await?;
    // Live records when the workspace chose a provider and gave it a key;
    // otherwise, or when the call fails, the deterministic simulation.
    let mut live_taxes = None;
    let mut coords = None;
    let (parcel, provider, fell_back, reason) =
        match live::rentcast_key(db, property.tenant_id).await {
            Some(key) => {
                let on_file = parcel_on_file(&detail);
                match rentcast::record(&key, &postal_address(property), Some(&on_file)).await {
                    Ok(r) => {
                        live_taxes = Some(r.taxes);
                        coords = r.latitude.zip(r.longitude);
                        (r.parcel, "rentcast", false, None)
                    }
                    Err(e) => {
                        tracing::warn!("RentCast record failed for {} ({e})", property.id);
                        let mut rng = simulated::rng_for(property.id, &property.address);
                        let p = simulated::parcel(&mut rng, &property.city, property.year_built);
                        (p, "simulated", true, Some(e.to_string()))
                    }
                }
            }
            None => {
                let mut rng = simulated::rng_for(property.id, &property.address);
                let p = simulated::parcel(&mut rng, &property.city, property.year_built);
                (p, "simulated", false, None)
            }
        };
    // Preserve real county / FIPS if the live geocoder already resolved them.
    let existing_county = detail.county.clone();
    let existing_fips = detail.fips.clone();
    let had_coords = detail.latitude.is_some();
    let mut am: entity::property_detail::ActiveModel = detail.into();
    am.apn = Set(opt(parcel.apn.clone()));
    am.zoning = Set(opt(parcel.zoning));
    am.subdivision = Set(opt(parcel.subdivision));
    am.county = Set(existing_county.or(opt(parcel.county)));
    am.fips = Set(existing_fips.or(opt(parcel.fips)));
    am.owner_of_record = Set(opt(parcel.owner_of_record));
    am.last_sale_date = Set(opt(parcel.last_sale_date));
    am.last_sale_price_cents = Set(Some(parcel.last_sale_price_cents));
    am.lot_size_sqft = Set(Some(parcel.lot_size_sqft));
    if parcel.year_built > 0 {
        am.year_built = Set(Some(parcel.year_built));
    }
    am.property_type = Set(opt(parcel.property_type));
    am.beds = Set(Some(parcel.beds));
    am.baths = Set(Some(parcel.baths));
    am.sqft = Set(Some(parcel.sqft));
    am.stories = Set(Some(parcel.stories));
    am.parking_spaces = Set(Some(parcel.parking_spaces));
    am.heating = Set(opt(parcel.heating));
    am.cooling = Set(opt(parcel.cooling));
    am.flood_zone = Set(opt(parcel.flood_zone));
    am.walk_score = Set(Some(parcel.walk_score));
    am.legal_description = Set(opt(parcel.legal_description));
    if let (false, Some((lat, lng))) = (had_coords, coords) {
        am.latitude = Set(Some(lat));
        am.longitude = Set(Some(lng));
        am.geocode_accuracy = Set(Some("provider".into()));
    }
    am.last_enriched_at = Set(Some(Utc::now().into()));
    am.updated_at = Set(Utc::now().into());
    am.update(db).await.map_err(db_err)?;
    // A live record carries the tax years too; file them so one call does both.
    if let Some(taxes) = live_taxes.filter(|t| !t.is_empty()) {
        replace_taxes(db, property, &taxes, "rentcast").await?;
    }
    Ok(SourceOutcome {
        summary: json!({ "apn": parcel.apn }),
        provider: provider.into(),
        fell_back,
        reason,
    })
}

/// Replace the property's tax years with `years`.
async fn replace_taxes<C: ConnectionTrait>(
    db: &C,
    property: &entity::property::Model,
    years: &[super::data::TaxYear],
    provider: &str,
) -> Result<(), EnrichmentError> {
    PropertyTax::delete_many()
        .filter(entity::property_tax::Column::PropertyId.eq(property.id))
        .exec(db)
        .await
        .map_err(db_err)?;
    for t in years {
        if !rentcast::plausible_year(t.tax_year) {
            continue;
        }
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
            source: Set(provider.to_string()),
            created_at: Set(Utc::now().into()),
        }
        .insert(db)
        .await
        .map_err(db_err)?;
    }
    Ok(())
}

async fn run_tax<C: ConnectionTrait>(
    db: &C,
    property: &entity::property::Model,
) -> Result<SourceOutcome, EnrichmentError> {
    if let Some(key) = live::rentcast_key(db, property.tenant_id).await {
        match rentcast::record(&key, &postal_address(property), None).await {
            Ok(r) if !r.taxes.is_empty() => {
                replace_taxes(db, property, &r.taxes, "rentcast").await?;
                return Ok(SourceOutcome {
                    summary: json!({ "years": r.taxes.len() }),
                    provider: "rentcast".into(),
                    fell_back: false,
                    reason: None,
                });
            }
            Ok(_) => tracing::warn!("RentCast had no tax years for {}", property.id),
            Err(e) => tracing::warn!("RentCast taxes failed for {} ({e})", property.id),
        }
    }
    let mut rng = simulated::rng_for(property.id, &property.address);
    let base_value = (property.monthly_rent_cents as f64 * 150.0) as i64;
    let base_assessed = (base_value as f64 * 0.85) as i64;
    let years = simulated::taxes(&mut rng, Utc::now().year(), base_assessed, 3);
    replace_taxes(db, property, &years, Source::Tax.provider()).await?;
    Ok(SourceOutcome::simulated(json!({ "years": years.len() })))
}

async fn run_valuation<C: ConnectionTrait>(
    db: &C,
    property: &entity::property::Model,
) -> Result<SourceOutcome, EnrichmentError> {
    let mut provider = Source::Valuation.provider().to_string();
    let mut fell_back = false;
    let mut reason = None;
    let live_v = match live::rentcast_key(db, property.tenant_id).await {
        Some(key) => {
            match rentcast::valuation(&key, &postal_address(property), property.monthly_rent_cents)
                .await
            {
                Ok(v) => {
                    provider = "rentcast".into();
                    Some(v)
                }
                Err(e) => {
                    tracing::warn!("RentCast valuation failed for {} ({e})", property.id);
                    fell_back = true;
                    reason = Some(e.to_string());
                    None
                }
            }
        }
        None => None,
    };
    let v = match live_v {
        Some(v) => v,
        None => {
            let mut rng = simulated::rng_for(property.id, &property.address);
            let base_value = (property.monthly_rent_cents as f64 * 150.0) as i64;
            let as_of = Utc::now().format("%Y-%m-%d").to_string();
            simulated::valuation(&mut rng, as_of, base_value, property.monthly_rent_cents)
        }
    };
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
        source: Set(provider.clone()),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await
    .map_err(db_err)?;
    Ok(SourceOutcome {
        summary: json!({
            "estimated_value_cents": v.estimated_value_cents,
            "estimated_rent_cents": v.estimated_rent_cents,
        }),
        provider,
        fell_back,
        reason,
    })
}

/// Crime statistics for the area: the FBI when the workspace has it on,
/// else the simulation. One row per property, replaced each run.
async fn run_crime<C: ConnectionTrait>(
    db: &C,
    property: &entity::property::Model,
) -> Result<SourceOutcome, EnrichmentError> {
    let detail = PropertyDetail::find_by_id(property.id)
        .one(db)
        .await
        .map_err(db_err)?;
    let (lat, lng, county) = detail
        .as_ref()
        .map(|d| (d.latitude, d.longitude, d.county.clone()))
        .unwrap_or((None, None, None));
    let (city, state) = city_state(
        &property.city,
        &property.state,
        detail.as_ref().and_then(|d| d.matched_address.as_deref()),
    );
    let (data, provider, fell_back, reason) = match live::fbi_key(db, property.tenant_id).await {
        Some(key) => match crime::fetch(&key, &state, &city, county.as_deref(), lat, lng).await {
            Ok(d) => (d, "fbi_cde", false, None),
            Err(e) => {
                tracing::warn!("FBI crime data failed for {} ({e})", property.id);
                let mut rng = simulated::rng_for(property.id, &property.address);
                (
                    simulated::crime(&mut rng, &city, &state),
                    "simulated",
                    true,
                    Some(e.to_string()),
                )
            }
        },
        None => {
            let mut rng = simulated::rng_for(property.id, &property.address);
            (
                simulated::crime(&mut rng, &city, &state),
                "simulated",
                false,
                None,
            )
        }
    };
    let verdict = data.verdict().to_string();
    let now = Utc::now();
    let existing = PropertyCrime::find_by_id(property.id)
        .one(db)
        .await
        .map_err(db_err)?;
    let offenses = serde_json::to_value(&data.offenses).unwrap_or_else(|_| json!([]));
    match existing {
        Some(row) => {
            let mut am: entity::property_crime::ActiveModel = row.into();
            am.agency_ori = Set(data.agency_ori.clone());
            am.agency_name = Set(data.agency_name.clone());
            am.agency_km = Set(data.agency_km);
            am.period_from = Set(data.period_from.clone());
            am.period_to = Set(data.period_to.clone());
            am.population = Set(data.population);
            am.offenses = Set(offenses);
            am.verdict = Set(verdict.clone());
            am.source = Set(provider.into());
            am.fetched_at = Set(now.into());
            am.updated_at = Set(now.into());
            am.update(db).await.map_err(db_err)?;
        }
        None => {
            entity::property_crime::ActiveModel {
                property_id: Set(property.id),
                tenant_id: Set(property.tenant_id),
                agency_ori: Set(data.agency_ori.clone()),
                agency_name: Set(data.agency_name.clone()),
                agency_km: Set(data.agency_km),
                period_from: Set(data.period_from.clone()),
                period_to: Set(data.period_to.clone()),
                population: Set(data.population),
                offenses: Set(offenses),
                verdict: Set(verdict.clone()),
                source: Set(provider.into()),
                fetched_at: Set(now.into()),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }
            .insert(db)
            .await
            .map_err(db_err)?;
        }
    }
    Ok(SourceOutcome {
        summary: json!({ "agency": data.agency_name, "verdict": verdict }),
        provider: provider.into(),
        fell_back,
        reason,
    })
}

/// `property_school.source` for a school the team added or edited.
pub const MANUAL_SCHOOL: &str = "manual";

async fn run_schools<C: ConnectionTrait>(
    db: &C,
    property: &entity::property::Model,
) -> Result<SourceOutcome, EnrichmentError> {
    let mut rng = simulated::rng_for(property.id, &property.address);
    let schools = simulated::schools(&mut rng);
    // A school the team added or edited is theirs: a refresh replaces only
    // what the data source put there, and skips a level the team has covered.
    PropertySchool::delete_many()
        .filter(entity::property_school::Column::PropertyId.eq(property.id))
        .filter(entity::property_school::Column::Source.ne(MANUAL_SCHOOL))
        .exec(db)
        .await
        .map_err(db_err)?;
    let kept: Vec<String> = PropertySchool::find()
        .filter(entity::property_school::Column::PropertyId.eq(property.id))
        .all(db)
        .await
        .map_err(db_err)?
        .into_iter()
        .map(|s| s.level)
        .collect();
    for s in schools.iter().filter(|s| !kept.contains(&s.level)) {
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
            // The source gives the zoned school for each level; the team
            // confirms the zone with the district.
            assigned: Set(true),
            zone_name: Set(Some(format!("{} attendance zone", s.name))),
            zone_verified_on: Set(None),
            address: Set(None),
            phone: Set(None),
            website: Set(None),
            enrollment: Set(None),
            notes: Set(None),
            updated_at: Set(None),
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

#[cfg(test)]
mod tests {
    use super::city_state;

    #[test]
    fn reads_the_state_out_of_the_city() {
        assert_eq!(
            city_state("Portland, OR", "", None),
            ("Portland".into(), "OR".into())
        );
        assert_eq!(
            city_state("Portland", "Oregon", None),
            ("Portland".into(), "OR".into())
        );
        assert_eq!(
            city_state("Portland", "wa", None),
            ("Portland".into(), "WA".into())
        );
        assert_eq!(
            city_state("Portland", "", Some("700 HARBOR DR, PORTLAND, OR, 97201")),
            ("Portland".into(), "OR".into())
        );
        assert_eq!(city_state("Nowhere", "", None).1, "");
    }
}
