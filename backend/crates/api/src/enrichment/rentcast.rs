//! Live **property records** from RentCast (`api.rentcast.io`): one call for
//! the parcel record (attributes, owner, last sale, tax assessments and tax
//! bills by year) and one for the valuation (value and rent estimates). A key
//! comes from the vault; the free plan covers a small portfolio's monthly
//! refresh. Parsing is pure and unit-tested.

use super::data::{err, EnrichmentError, ParcelData, TaxYear, ValuationData};
use chrono::{Datelike, Utc};
use serde_json::Value;

const BASE: &str = "https://api.rentcast.io/v1";

/// What the property record call gives us: the parcel plus tax years.
pub struct Record {
    pub parcel: ParcelData,
    pub taxes: Vec<TaxYear>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

fn cents(v: &Value) -> Option<i64> {
    v.as_f64().map(|d| (d * 100.0).round() as i64)
}

fn text(v: &Value) -> String {
    v.as_str().unwrap_or("").trim().to_string()
}

/// `singleFamily` → `single_family`, and so on.
fn property_type(raw: &str) -> String {
    match raw.trim().to_lowercase().replace([' ', '-'], "").as_str() {
        "singlefamily" => "single_family",
        "multifamily" | "apartment" => "multi_family",
        "condo" => "condo",
        "townhouse" | "townhome" => "townhome",
        "manufactured" => "manufactured",
        "land" => "land",
        "" => "",
        other => other,
    }
    .to_string()
}

/// Parse the first property in `GET /properties?address=` (an array), keeping
/// anything the record lacks from `existing` so a thin record doesn't blank a
/// fuller one.
pub fn parse_record(
    body: &Value,
    existing: Option<&ParcelData>,
) -> Result<Record, EnrichmentError> {
    let p = body
        .as_array()
        .and_then(|a| a.first())
        .or(if body.is_object() { Some(body) } else { None })
        .ok_or_else(|| err("RentCast returned no property for that address"))?;
    let keep = |v: String, old: Option<&String>| -> String {
        if v.is_empty() {
            old.cloned().unwrap_or_default()
        } else {
            v
        }
    };
    let f = p.get("features").cloned().unwrap_or(Value::Null);
    let owner = p["owner"]["names"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|n| n.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let parcel = ParcelData {
        apn: keep(text(&p["assessorID"]), existing.map(|e| &e.apn)),
        zoning: keep(text(&p["zoning"]), existing.map(|e| &e.zoning)),
        subdivision: keep(text(&p["subdivision"]), existing.map(|e| &e.subdivision)),
        county: keep(text(&p["county"]), existing.map(|e| &e.county)),
        fips: keep(text(&p["countyFips"]), existing.map(|e| &e.fips)),
        owner_of_record: keep(owner, existing.map(|e| &e.owner_of_record)),
        last_sale_date: keep(
            text(&p["lastSaleDate"]).chars().take(10).collect(),
            existing.map(|e| &e.last_sale_date),
        ),
        last_sale_price_cents: cents(&p["lastSalePrice"])
            .or(existing.map(|e| e.last_sale_price_cents))
            .unwrap_or(0),
        lot_size_sqft: p["lotSize"]
            .as_f64()
            .map(|v| v as i64)
            .or(existing.map(|e| e.lot_size_sqft))
            .unwrap_or(0),
        property_type: keep(
            property_type(&text(&p["propertyType"])),
            existing.map(|e| &e.property_type),
        ),
        beds: p["bedrooms"]
            .as_f64()
            .map(|v| v as i32)
            .or(existing.map(|e| e.beds))
            .unwrap_or(0),
        baths: p["bathrooms"]
            .as_f64()
            .or(existing.map(|e| e.baths))
            .unwrap_or(0.0),
        sqft: p["squareFootage"]
            .as_f64()
            .map(|v| v as i32)
            .or(existing.map(|e| e.sqft))
            .unwrap_or(0),
        stories: f["floorCount"]
            .as_f64()
            .map(|v| v as i32)
            .or(existing.map(|e| e.stories))
            .unwrap_or(0),
        parking_spaces: f["garageSpaces"]
            .as_f64()
            .map(|v| v as i32)
            .or(existing.map(|e| e.parking_spaces))
            .unwrap_or(0),
        heating: keep(text(&f["heatingType"]), existing.map(|e| &e.heating)),
        cooling: keep(text(&f["coolingType"]), existing.map(|e| &e.cooling)),
        flood_zone: existing.map(|e| e.flood_zone.clone()).unwrap_or_default(),
        walk_score: existing.map(|e| e.walk_score).unwrap_or(0),
        legal_description: keep(
            text(&p["legalDescription"]),
            existing.map(|e| &e.legal_description),
        ),
    };
    // Assessments and bills are keyed by year: {"2024": {"value":..,"land":..,"improvements":..}}
    // and {"2024": {"total": ..}}.
    let mut taxes: Vec<TaxYear> = vec![];
    if let Some(assess) = p["taxAssessments"].as_object() {
        for (year, a) in assess {
            let Ok(y) = year.parse::<i32>() else { continue };
            let assessed = cents(&a["value"]).unwrap_or(0);
            let bill = cents(&p["propertyTaxes"][year]["total"]).unwrap_or(0);
            taxes.push(TaxYear {
                tax_year: y,
                assessed_value_cents: assessed,
                land_value_cents: cents(&a["land"]).unwrap_or(0),
                improvement_value_cents: cents(&a["improvements"]).unwrap_or(0),
                tax_amount_cents: bill,
                tax_rate_bps: if assessed > 0 {
                    ((bill as f64 / assessed as f64) * 10_000.0).round() as i32
                } else {
                    0
                },
            });
        }
    }
    taxes.sort_by_key(|t| std::cmp::Reverse(t.tax_year));
    Ok(Record {
        parcel,
        taxes,
        latitude: p["latitude"].as_f64(),
        longitude: p["longitude"].as_f64(),
    })
}

/// Parse `GET /avm/value?address=` into a valuation; `rent_cents` comes from
/// the rent estimate call when made, else the property's own rent.
pub fn parse_value(body: &Value, rent_cents: i64) -> Result<ValuationData, EnrichmentError> {
    let price = cents(&body["price"]).ok_or_else(|| err("RentCast returned no value estimate"))?;
    let low = cents(&body["priceRangeLow"]).unwrap_or(price);
    let high = cents(&body["priceRangeHigh"]).unwrap_or(price);
    // Confidence from the band: a tight band is a confident estimate.
    let spread = if price > 0 {
        (high - low) as f64 / price as f64
    } else {
        1.0
    };
    let confidence = ((1.0 - spread.min(1.0)) * 100.0).round().clamp(10.0, 95.0) as i32;
    Ok(ValuationData {
        as_of: Utc::now().format("%Y-%m-%d").to_string(),
        estimated_value_cents: price,
        value_low_cents: low,
        value_high_cents: high,
        estimated_rent_cents: rent_cents,
        confidence,
    })
}

async fn get(
    client: &reqwest::Client,
    key: &str,
    path: &str,
    address: &str,
) -> Result<Value, EnrichmentError> {
    let resp = client
        .get(format!("{BASE}{path}"))
        .header("X-Api-Key", key)
        .header("Accept", "application/json")
        .query(&[("address", address)])
        .send()
        .await
        .map_err(|e| err(format!("RentCast request failed: {e}")))?;
    if !resp.status().is_success() {
        return Err(err(format!("RentCast returned HTTP {}", resp.status())));
    }
    resp.json()
        .await
        .map_err(|e| err(format!("RentCast returned invalid JSON: {e}")))
}

/// The property record for an address.
pub async fn record(
    key: &str,
    address: &str,
    existing: Option<&ParcelData>,
) -> Result<Record, EnrichmentError> {
    let client = crate::providers::client::build_http_client()
        .map_err(|e| err(format!("http client: {e}")))?;
    let body = get(&client, key, "/properties", address).await?;
    parse_record(&body, existing)
}

/// The value and rent estimates for an address.
pub async fn valuation(
    key: &str,
    address: &str,
    fallback_rent_cents: i64,
) -> Result<ValuationData, EnrichmentError> {
    let client = crate::providers::client::build_http_client()
        .map_err(|e| err(format!("http client: {e}")))?;
    let value = get(&client, key, "/avm/value", address).await?;
    let rent = get(&client, key, "/avm/rent/long-term", address)
        .await
        .ok()
        .and_then(|r| cents(&r["rent"]))
        .unwrap_or(fallback_rent_cents);
    parse_value(&value, rent)
}

/// A year that's plausibly a tax year.
pub fn plausible_year(y: i32) -> bool {
    (1990..=Utc::now().year() + 1).contains(&y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_a_record_with_taxes() {
        let body = json!([{
            "assessorID": "R123456", "county": "Multnomah", "countyFips": "41051",
            "propertyType": "Single Family", "bedrooms": 3, "bathrooms": 2.5, "squareFootage": 1640,
            "lotSize": 5200, "zoning": "R5", "lastSaleDate": "2021-06-15T00:00:00.000Z", "lastSalePrice": 512000,
            "owner": {"names": ["Maple Holdings LLC"]},
            "features": {"floorCount": 2, "garageSpaces": 1, "heatingType": "Forced Air", "coolingType": "Central"},
            "taxAssessments": {"2023": {"value": 410000, "land": 150000, "improvements": 260000}, "2024": {"value": 425000, "land": 155000, "improvements": 270000}},
            "propertyTaxes": {"2023": {"total": 5125}, "2024": {"total": 5312.5}},
            "latitude": 45.52, "longitude": -122.68
        }]);
        let r = parse_record(&body, None).unwrap();
        assert_eq!(r.parcel.apn, "R123456");
        assert_eq!(r.parcel.property_type, "single_family");
        assert_eq!(r.parcel.last_sale_date, "2021-06-15");
        assert_eq!(r.parcel.last_sale_price_cents, 51_200_000);
        assert_eq!(r.parcel.baths, 2.5);
        assert_eq!(r.parcel.owner_of_record, "Maple Holdings LLC");
        assert_eq!(r.taxes.len(), 2);
        assert_eq!(r.taxes[0].tax_year, 2024);
        assert_eq!(r.taxes[0].tax_amount_cents, 531_250);
        assert_eq!(r.taxes[0].tax_rate_bps, 125);
        assert_eq!(r.latitude, Some(45.52));
        assert!(parse_record(&json!([]), None).is_err());
    }

    #[test]
    fn keeps_what_a_thin_record_lacks() {
        let old = ParcelData {
            apn: "OLD".into(),
            zoning: "R-1".into(),
            subdivision: "".into(),
            county: "Multnomah".into(),
            fips: "41051".into(),
            owner_of_record: "Someone".into(),
            last_sale_date: "2019-01-01".into(),
            last_sale_price_cents: 1,
            lot_size_sqft: 10,
            property_type: "condo".into(),
            beds: 2,
            baths: 1.0,
            sqft: 900,
            stories: 1,
            parking_spaces: 0,
            heating: "".into(),
            cooling: "".into(),
            flood_zone: "X (minimal)".into(),
            walk_score: 77,
            legal_description: "".into(),
        };
        let r = parse_record(&json!([{"bedrooms": 3}]), Some(&old)).unwrap();
        assert_eq!(r.parcel.beds, 3);
        assert_eq!(r.parcel.apn, "OLD");
        assert_eq!(r.parcel.walk_score, 77);
        assert_eq!(r.parcel.flood_zone, "X (minimal)");
    }

    #[test]
    fn values_with_a_confidence_from_the_band() {
        let v = parse_value(
            &json!({"price": 500000, "priceRangeLow": 475000, "priceRangeHigh": 525000}),
            250000,
        )
        .unwrap();
        assert_eq!(v.estimated_value_cents, 50_000_000);
        assert_eq!(v.confidence, 90);
        assert_eq!(v.estimated_rent_cents, 250000);
        assert!(parse_value(&json!({}), 0).is_err());
    }
}
