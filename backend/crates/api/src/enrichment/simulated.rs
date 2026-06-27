//! Deterministic **simulated** providers — the same interface a real county /
//! AVM / schools API would sit behind, but generating stable, plausible data
//! seeded from the property so repeated runs are idempotent and tests are
//! hermetic. Swapping in a real provider is a matter of replacing one function.

use super::data::{ParcelData, SchoolData, TaxYear, UtilityData, ValuationData};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// A tiny deterministic PRNG (LCG) so generated data is stable per property.
pub struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        // Use the high bits, which have the best statistical quality for an LCG.
        self.0 >> 16
    }

    /// Inclusive range `[lo, hi]`.
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        if hi <= lo {
            return lo;
        }
        lo + (self.next() % ((hi - lo + 1) as u64)) as i64
    }

    fn pick<'a, T>(&mut self, opts: &'a [T]) -> &'a T {
        &opts[self.range(0, opts.len() as i64 - 1) as usize]
    }
}

/// Seed an [`Rng`] deterministically from the property id + address.
pub fn rng_for(property_id: Uuid, address: &str) -> Rng {
    let mut h = Sha256::new();
    h.update(property_id.as_bytes());
    h.update(address.as_bytes());
    let digest = h.finalize();
    let mut seed = [0u8; 8];
    seed.copy_from_slice(&digest[..8]);
    Rng(u64::from_le_bytes(seed))
}

/// Extract a 2-letter state code from a `"City, ST"` string (best-effort).
fn state_of(city: &str) -> String {
    city.rsplit(',')
        .next()
        .map(|s| s.trim().to_string())
        .filter(|s| s.len() == 2)
        .unwrap_or_else(|| "US".into())
}

/// Simulated parcel / county record.
pub fn parcel(rng: &mut Rng, city: &str, year_built: i32) -> ParcelData {
    let zonings = ["R-1", "R-2", "RM-2", "C-1", "MX-2"];
    let types = ["single_family", "multi_family", "condo", "townhome"];
    let heatings = ["Forced air (gas)", "Heat pump", "Baseboard (electric)"];
    let coolings = ["Central A/C", "Heat pump", "None"];
    let floods = [
        "X (minimal)",
        "X (minimal)",
        "AE (1% annual)",
        "X (minimal)",
    ];
    let owners = [
        "Maple Holdings LLC",
        "Harbor LLC",
        "Private owner",
        "Elm Equity LLC",
    ];
    let counties = ["Multnomah", "Washington", "Clackamas", "King", "Pierce"];

    let state = state_of(city);
    let apn = format!(
        "{:02}-{:04}-{:03}",
        rng.range(1, 99),
        rng.range(1000, 9999),
        rng.range(100, 999)
    );
    let sale_year = rng.range((year_built as i64).max(2005), 2024);
    let sale_price = rng.range(280_000, 2_400_000) * 100;

    ParcelData {
        apn,
        zoning: rng.pick(&zonings).to_string(),
        subdivision: format!(
            "{} Addition",
            rng.pick(&["Maple", "Birch", "Harbor", "Elm", "Cedar"])
        ),
        county: format!("{} County, {state}", rng.pick(&counties)),
        fips: format!("{}", rng.range(41_000_000_000, 53_999_999_999)),
        owner_of_record: rng.pick(&owners).to_string(),
        last_sale_date: format!(
            "{sale_year}-{:02}-{:02}",
            rng.range(1, 12),
            rng.range(1, 28)
        ),
        last_sale_price_cents: sale_price,
        lot_size_sqft: rng.range(2_400, 14_000),
        property_type: rng.pick(&types).to_string(),
        beds: rng.range(1, 5) as i32,
        baths: rng.range(2, 7) as f64 / 2.0,
        sqft: rng.range(620, 3_400) as i32,
        stories: rng.range(1, 3) as i32,
        parking_spaces: rng.range(0, 3) as i32,
        heating: rng.pick(&heatings).to_string(),
        cooling: rng.pick(&coolings).to_string(),
        flood_zone: rng.pick(&floods).to_string(),
        walk_score: rng.range(28, 98) as i32,
        legal_description: format!(
            "LOT {} BLK {} {} ADD",
            rng.range(1, 40),
            rng.range(1, 12),
            rng.pick(&["MAPLE", "BIRCH", "HARBOR", "ELM"])
        ),
    }
}

/// Simulated tax-assessment history for the last `years` years.
pub fn taxes(
    rng: &mut Rng,
    current_year: i32,
    base_assessed_cents: i64,
    years: i32,
) -> Vec<TaxYear> {
    let rate_bps = rng.range(90, 180) as i32; // ~0.9%–1.8% effective
    let mut out = Vec::new();
    for back in 0..years {
        let year = current_year - back;
        // Older years assessed a little lower (gentle appreciation).
        let factor = 1.0 - (back as f64 * 0.04);
        let assessed = (base_assessed_cents as f64 * factor).round() as i64;
        let land = (assessed as f64 * 0.35).round() as i64;
        let tax = (assessed as f64 * (rate_bps as f64 / 10_000.0)).round() as i64;
        out.push(TaxYear {
            tax_year: year,
            assessed_value_cents: assessed,
            land_value_cents: land,
            improvement_value_cents: assessed - land,
            tax_amount_cents: tax,
            tax_rate_bps: rate_bps,
        });
    }
    out
}

/// Simulated AVM valuation + rent estimate.
pub fn valuation(
    rng: &mut Rng,
    as_of: String,
    base_value_cents: i64,
    base_rent_cents: i64,
) -> ValuationData {
    let jitter = rng.range(-7, 9) as f64 / 100.0;
    let value = (base_value_cents as f64 * (1.0 + jitter)).round() as i64;
    let spread = (value as f64 * 0.06).round() as i64;
    let rent = (base_rent_cents as f64 * (1.0 + (rng.range(-4, 6) as f64 / 100.0))).round() as i64;
    ValuationData {
        as_of,
        estimated_value_cents: value,
        value_low_cents: value - spread,
        value_high_cents: value + spread,
        estimated_rent_cents: rent,
        confidence: rng.range(72, 96) as i32,
    }
}

/// Simulated assigned schools (one per level).
pub fn schools(rng: &mut Rng) -> Vec<SchoolData> {
    let districts = [
        "Portland SD 1J",
        "Beaverton SD",
        "Lake Oswego SD",
        "Seattle SD 1",
    ];
    let district = rng.pick(&districts).to_string();
    let names = [
        "Maplewood",
        "Riverbend",
        "Cedar Ridge",
        "Harborview",
        "Elm Grove",
    ];
    let levels = [("elementary", "K-5"), ("middle", "6-8"), ("high", "9-12")];
    levels
        .iter()
        .map(|(level, grades)| SchoolData {
            name: format!("{} {}", rng.pick(&names), level_suffix(level)),
            level: (*level).to_string(),
            district: district.clone(),
            rating: rng.range(4, 10) as i32,
            distance_mi: rng.range(2, 38) as f64 / 10.0,
            grades: (*grades).to_string(),
        })
        .collect()
}

fn level_suffix(level: &str) -> &'static str {
    match level {
        "elementary" => "Elementary",
        "middle" => "Middle School",
        _ => "High School",
    }
}

/// Simulated utility providers.
pub fn utilities(rng: &mut Rng) -> Vec<UtilityData> {
    let plans: [(&str, &[&str], i64, i64); 6] = [
        (
            "electric",
            &["Portland General", "Pacific Power", "Seattle City Light"],
            70,
            180,
        ),
        ("gas", &["NW Natural", "Cascade Natural Gas"], 30, 110),
        (
            "water",
            &["City Water Bureau", "Tualatin Valley Water"],
            35,
            95,
        ),
        ("sewer", &["City Environmental Services"], 25, 70),
        ("trash", &["Recology", "Waste Management"], 18, 55),
        (
            "internet",
            &["Ziply Fiber", "Comcast Xfinity", "CenturyLink"],
            45,
            120,
        ),
    ];
    plans
        .iter()
        .map(|(kind, providers, lo, hi)| UtilityData {
            utility_type: (*kind).to_string(),
            provider: rng.pick(providers).to_string(),
            est_monthly_cost_cents: rng.range(*lo, *hi) * 100,
            phone: format!(
                "(5{:02}) 555-{:04}",
                rng.range(0, 99),
                rng.range(1000, 9999)
            ),
        })
        .collect()
}
