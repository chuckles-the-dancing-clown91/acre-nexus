//! Crime statistics from the **FBI Crime Data Explorer** (`api.usa.gov`): the
//! agencies in the property's state with their coordinates, then the nearest
//! agency's monthly offense rates per 100,000 people for the last two years,
//! with the state and national rates on the same scale. Parsing is pure and
//! unit-tested; the network part is small.

use super::data::{err, CrimeData, CrimeOffense, EnrichmentError};
use chrono::{Datelike, NaiveDate, Utc};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const BASE: &str = "https://api.usa.gov/crime/fbi/cde";

/// The offense categories shown, in order.
pub const OFFENSES: &[(&str, &str)] = &[
    ("violent-crime", "Violent crime"),
    ("property-crime", "Property crime"),
    ("burglary", "Burglary"),
    ("motor-vehicle-theft", "Vehicle theft"),
];

/// A reporting agency, from the by-state listing.
#[derive(Clone, Debug)]
pub struct Agency {
    pub ori: String,
    pub name: String,
    pub county: Option<String>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
}

/// Parse `GET /agency/byStateAbbr/{ST}`: a map of county → agencies.
pub fn parse_agencies(body: &Value) -> Vec<Agency> {
    let mut out = vec![];
    if let Some(map) = body.as_object() {
        for (county, list) in map {
            for a in list.as_array().into_iter().flatten() {
                let Some(ori) = a["ori"].as_str() else {
                    continue;
                };
                out.push(Agency {
                    ori: ori.to_string(),
                    name: a["agency_name"].as_str().unwrap_or(ori).to_string(),
                    county: Some(county.clone()).filter(|c| !c.is_empty()),
                    lat: a["latitude"].as_f64(),
                    lng: a["longitude"].as_f64(),
                });
            }
        }
    }
    out
}

/// Great-circle distance in km.
pub fn km(lat1: f64, lng1: f64, lat2: f64, lng2: f64) -> f64 {
    let r = 6371.0;
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dp = (lat2 - lat1).to_radians();
    let dl = (lng2 - lng1).to_radians();
    let a = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * r * a.sqrt().atan2((1.0 - a).sqrt())
}

/// The agency that covers the property: the city's own police department
/// when the name matches, else the nearest city or county agency by
/// distance, else the first agency in the property's county.
pub fn pick_agency<'a>(
    agencies: &'a [Agency],
    city: &str,
    county: Option<&str>,
    lat: Option<f64>,
    lng: Option<f64>,
) -> Option<(&'a Agency, Option<f64>)> {
    let city_l = city.trim().to_lowercase();
    let is_police = |a: &Agency| {
        let n = a.name.to_lowercase();
        n.contains("police") || n.contains("sheriff") || n.contains("public safety")
    };
    if !city_l.is_empty() {
        if let Some(a) = agencies.iter().find(|a| {
            let n = a.name.to_lowercase();
            is_police(a) && n.starts_with(&format!("{city_l} ")) && n.contains("police")
        }) {
            let d = match (lat, lng, a.lat, a.lng) {
                (Some(x), Some(y), Some(ax), Some(ay)) => Some(km(x, y, ax, ay)),
                _ => None,
            };
            return Some((a, d));
        }
    }
    if let (Some(x), Some(y)) = (lat, lng) {
        let mut best: Option<(&Agency, f64)> = None;
        for a in agencies.iter().filter(|a| is_police(a)) {
            if let (Some(ax), Some(ay)) = (a.lat, a.lng) {
                let d = km(x, y, ax, ay);
                if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                    best = Some((a, d));
                }
            }
        }
        if let Some((a, d)) = best {
            return Some((a, Some(d)));
        }
    }
    if let Some(c) = county.map(|c| c.to_lowercase().replace(" county", "")) {
        if let Some(a) = agencies.iter().find(|a| {
            a.county
                .as_deref()
                .map(|x| x.to_lowercase() == c)
                .unwrap_or(false)
                && is_police(a)
        }) {
            return Some((a, None));
        }
    }
    None
}

/// `MM-YYYY` for a month.
fn month_key(d: NaiveDate) -> String {
    format!("{:02}-{}", d.month(), d.year())
}

/// The last 12 complete months ending `end`, plus the 12 before them.
pub fn periods(today: NaiveDate) -> (Vec<String>, Vec<String>) {
    let mut months = vec![];
    let mut d = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap();
    for _ in 0..25 {
        d = d.pred_opt().unwrap();
        d = NaiveDate::from_ymd_opt(d.year(), d.month(), 1).unwrap();
        months.push(month_key(d));
    }
    // months[0] is last month; the data lags a couple of months, so start
    // the window three months back.
    let recent: Vec<String> = months[2..14].to_vec();
    let prior: Vec<String> = months[14..].to_vec();
    (recent, prior)
}

/// One series from the summarized response: the agency's key is the one that
/// isn't the state or "United States".
fn series<'a>(rates: &'a Value, suffix: &str, state_name: &str) -> Option<(&'a str, &'a Value)> {
    rates.as_object()?.iter().find_map(|(k, v)| {
        let base = k.strip_suffix(suffix)?;
        if base == state_name || base == "United States" {
            None
        } else {
            Some((base, v))
        }
    })
}

fn mean_over(v: &Value, months: &[String]) -> Option<f64> {
    let vals: Vec<f64> = months
        .iter()
        .filter_map(|m| v.get(m).and_then(|x| x.as_f64()))
        .collect();
    if vals.is_empty() {
        None
    } else {
        Some(vals.iter().sum::<f64>() / vals.len() as f64)
    }
}

fn sum_over(v: &Value, months: &[String]) -> i64 {
    months
        .iter()
        .filter_map(|m| v.get(m).and_then(|x| x.as_f64()))
        .sum::<f64>()
        .round() as i64
}

/// Parse one offense's summarized response into a [`CrimeOffense`]. Rates
/// are monthly per 100,000; the period's rate is their sum (an annualised
/// rate when the window is 12 months). Returns the agency name it found too.
pub fn parse_offense(
    body: &Value,
    key: &str,
    label: &str,
    state_name: &str,
    recent: &[String],
    prior: &[String],
) -> Option<(CrimeOffense, String, Option<i64>)> {
    let rates = &body["offenses"]["rates"];
    let (agency_name, agency_rates) = series(rates, " Offenses", state_name)?;
    let state_rates = rates.get(format!("{state_name} Offenses"))?;
    let us_rates = rates.get("United States Offenses")?;
    let actuals = &body["offenses"]["actuals"];
    let agency_actuals = actuals
        .get(format!("{agency_name} Offenses"))
        .cloned()
        .unwrap_or(Value::Null);
    let population = body["populations"]["population"]
        .get(agency_name)
        .and_then(|p| p.as_object())
        .and_then(|m| m.values().next_back())
        .and_then(|v| v.as_i64());
    let n = recent.len().max(1) as f64;
    let agency_rate = mean_over(agency_rates, recent)? * n;
    let state_rate = mean_over(state_rates, recent).unwrap_or(0.0) * n;
    let us_rate = mean_over(us_rates, recent).unwrap_or(0.0) * n;
    let prior_rate = mean_over(agency_rates, prior).map(|m| m * prior.len() as f64);
    Some((
        CrimeOffense {
            key: key.into(),
            label: label.into(),
            agency: sum_over(&agency_actuals, recent),
            agency_rate: (agency_rate * 10.0).round() / 10.0,
            state_rate: (state_rate * 10.0).round() / 10.0,
            us_rate: (us_rate * 10.0).round() / 10.0,
            prior_rate: prior_rate.map(|r| (r * 10.0).round() / 10.0),
        },
        agency_name.to_string(),
        population,
    ))
}

/// State postal code → the name the FBI uses in series keys.
pub fn state_name(code: &str) -> &'static str {
    match code.trim().to_uppercase().as_str() {
        "AL" => "Alabama",
        "AK" => "Alaska",
        "AZ" => "Arizona",
        "AR" => "Arkansas",
        "CA" => "California",
        "CO" => "Colorado",
        "CT" => "Connecticut",
        "DE" => "Delaware",
        "DC" => "District of Columbia",
        "FL" => "Florida",
        "GA" => "Georgia",
        "HI" => "Hawaii",
        "ID" => "Idaho",
        "IL" => "Illinois",
        "IN" => "Indiana",
        "IA" => "Iowa",
        "KS" => "Kansas",
        "KY" => "Kentucky",
        "LA" => "Louisiana",
        "ME" => "Maine",
        "MD" => "Maryland",
        "MA" => "Massachusetts",
        "MI" => "Michigan",
        "MN" => "Minnesota",
        "MS" => "Mississippi",
        "MO" => "Missouri",
        "MT" => "Montana",
        "NE" => "Nebraska",
        "NV" => "Nevada",
        "NH" => "New Hampshire",
        "NJ" => "New Jersey",
        "NM" => "New Mexico",
        "NY" => "New York",
        "NC" => "North Carolina",
        "ND" => "North Dakota",
        "OH" => "Ohio",
        "OK" => "Oklahoma",
        "OR" => "Oregon",
        "PA" => "Pennsylvania",
        "RI" => "Rhode Island",
        "SC" => "South Carolina",
        "SD" => "South Dakota",
        "TN" => "Tennessee",
        "TX" => "Texas",
        "UT" => "Utah",
        "VT" => "Vermont",
        "VA" => "Virginia",
        "WA" => "Washington",
        "WV" => "West Virginia",
        "WI" => "Wisconsin",
        "WY" => "Wyoming",
        "PR" => "Puerto Rico",
        _ => "",
    }
}

async fn get(client: &reqwest::Client, url: &str, key: &str) -> Result<Value, EnrichmentError> {
    let resp = client
        .get(url)
        .query(&[("API_KEY", key)])
        .send()
        .await
        .map_err(|e| err(format!("FBI CDE request failed: {e}")))?;
    if !resp.status().is_success() {
        return Err(err(format!("FBI CDE returned HTTP {}", resp.status())));
    }
    resp.json()
        .await
        .map_err(|e| err(format!("FBI CDE returned invalid JSON: {e}")))
}

/// A state's agency listing and when it was fetched.
type AgencyCache = Mutex<HashMap<String, (Instant, Vec<Agency>)>>;

/// The by-state agency listings, kept for an hour: they barely change and the
/// demo key allows few calls.
fn agency_cache() -> &'static AgencyCache {
    static CACHE: OnceLock<AgencyCache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

async fn agencies_for(
    client: &reqwest::Client,
    key: &str,
    st: &str,
) -> Result<Vec<Agency>, EnrichmentError> {
    if let Some((at, list)) = agency_cache().lock().ok().and_then(|c| c.get(st).cloned()) {
        if at.elapsed() < Duration::from_secs(3600) {
            return Ok(list);
        }
    }
    let listing = get(client, &format!("{BASE}/agency/byStateAbbr/{st}"), key).await?;
    let agencies = parse_agencies(&listing);
    if let Ok(mut c) = agency_cache().lock() {
        c.insert(st.to_string(), (Instant::now(), agencies.clone()));
    }
    Ok(agencies)
}

/// Fetch crime statistics for a property: `state` is the postal code.
pub async fn fetch(
    key: &str,
    state: &str,
    city: &str,
    county: Option<&str>,
    lat: Option<f64>,
    lng: Option<f64>,
) -> Result<CrimeData, EnrichmentError> {
    let st = state.trim().to_uppercase();
    let st_name = state_name(&st);
    if st_name.is_empty() {
        return Err(err(format!("no FBI data for state \"{state}\"")));
    }
    let client = crate::providers::client::build_http_client()
        .map_err(|e| err(format!("http client: {e}")))?;
    let agencies = agencies_for(&client, key, &st).await?;
    let (agency, dist) = pick_agency(&agencies, city, county, lat, lng)
        .ok_or_else(|| err(format!("no reporting agency found near {city}, {st}")))?;
    let (recent, prior) = periods(Utc::now().date_naive());
    let from = prior.last().cloned().unwrap_or_default();
    let to = recent.first().cloned().unwrap_or_default();
    let mut offenses = vec![];
    let mut name = agency.name.clone();
    let mut population = None;
    for (k, label) in OFFENSES {
        let url = format!(
            "{BASE}/summarized/agency/{}/{k}?from={from}&to={to}",
            agency.ori
        );
        let body = get(&client, &url, key).await?;
        if let Some((o, n, pop)) = parse_offense(&body, k, label, st_name, &recent, &prior) {
            offenses.push(o);
            name = n;
            population = population.or(pop);
        }
    }
    if offenses.is_empty() {
        return Err(err(format!(
            "{} reported no figures for the period",
            agency.name
        )));
    }
    Ok(CrimeData {
        agency_ori: Some(agency.ori.clone()),
        agency_name: name,
        agency_km: dist.map(|d| (d * 10.0).round() / 10.0),
        period_from: recent.last().cloned().unwrap_or_default(),
        period_to: to,
        population,
        offenses,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn months(from: (u32, i32), n: usize) -> Vec<String> {
        let mut out = vec![];
        let (mut m, mut y) = from;
        for _ in 0..n {
            out.push(format!("{m:02}-{y}"));
            m += 1;
            if m > 12 {
                m = 1;
                y += 1;
            }
        }
        out
    }

    #[test]
    fn picks_the_citys_police_then_nearest() {
        let body = json!({
            "MULTNOMAH": [
                {"ori": "OR0260200", "agency_name": "Portland Police Bureau", "latitude": 45.5, "longitude": -122.6, "agency_type_name": "City"},
                {"ori": "OR0260000", "agency_name": "Multnomah County Sheriff's Office", "latitude": 45.6, "longitude": -122.5, "agency_type_name": "County"}
            ],
            "WASHINGTON": [
                {"ori": "OR0340100", "agency_name": "Beaverton Police Department", "latitude": 45.48, "longitude": -122.8, "agency_type_name": "City"}
            ]
        });
        let agencies = parse_agencies(&body);
        assert_eq!(agencies.len(), 3);
        let (a, d) = pick_agency(&agencies, "Beaverton", None, Some(45.49), Some(-122.81)).unwrap();
        assert_eq!(a.ori, "OR0340100");
        assert!(d.unwrap() < 2.0);
        // No name match: nearest police agency wins.
        let (a, _) = pick_agency(&agencies, "Gresham", None, Some(45.5), Some(-122.61)).unwrap();
        assert_eq!(a.ori, "OR0260200");
        // No coordinates: the county's agency.
        let (a, d) =
            pick_agency(&agencies, "Nowhere", Some("Washington County"), None, None).unwrap();
        assert_eq!(a.ori, "OR0340100");
        assert!(d.is_none());
        assert!(pick_agency(&agencies, "Nowhere", None, None, None).is_none());
    }

    #[test]
    fn periods_skip_the_lag_and_cover_two_years() {
        let (recent, prior) = periods(NaiveDate::from_ymd_opt(2026, 10, 4).unwrap());
        assert_eq!(recent.len(), 12);
        assert_eq!(prior.len(), 11);
        assert_eq!(recent[0], "07-2026");
        assert_eq!(recent[11], "08-2025");
        assert_eq!(prior[0], "07-2025");
    }

    #[test]
    fn sums_monthly_rates_into_a_yearly_one() {
        let recent = months((1, 2024), 12);
        let prior = months((1, 2023), 12);
        let mk = |v: f64, ms: &[String]| -> Value {
            json!(ms
                .iter()
                .map(|m| (m.clone(), json!(v)))
                .collect::<serde_json::Map<_, _>>())
        };
        let mut agency = mk(60.0, &recent);
        for (k, v) in mk(50.0, &prior).as_object().unwrap() {
            agency[k] = v.clone();
        }
        let body = json!({
            "offenses": {
                "rates": {
                    "Oregon Offenses": mk(27.0, &recent),
                    "Oregon Clearances": mk(4.0, &recent),
                    "United States Offenses": mk(19.0, &recent),
                    "United States Clearances": mk(3.0, &recent),
                    "Portland Police Department Offenses": agency,
                    "Portland Police Department Clearances": mk(5.0, &recent)
                },
                "actuals": { "Portland Police Department Offenses": mk(370.0, &recent) }
            },
            "populations": { "population": { "Portland Police Department": { "01-2024": 623066 } } }
        });
        let (o, name, pop) =
            parse_offense(&body, "burglary", "Burglary", "Oregon", &recent, &prior).unwrap();
        assert_eq!(name, "Portland Police Department");
        assert_eq!(pop, Some(623066));
        assert_eq!(o.agency_rate, 720.0);
        assert_eq!(o.state_rate, 324.0);
        assert_eq!(o.us_rate, 228.0);
        assert_eq!(o.prior_rate, Some(600.0));
        assert_eq!(o.agency, 4440);
        let data = CrimeData {
            agency_ori: None,
            agency_name: name,
            agency_km: None,
            period_from: "01-2024".into(),
            period_to: "12-2024".into(),
            population: pop,
            offenses: vec![
                CrimeOffense {
                    key: "violent-crime".into(),
                    label: "".into(),
                    agency: 0,
                    agency_rate: 700.0,
                    state_rate: 300.0,
                    us_rate: 0.0,
                    prior_rate: None,
                },
                CrimeOffense {
                    key: "property-crime".into(),
                    label: "".into(),
                    agency: 0,
                    agency_rate: 5000.0,
                    state_rate: 3000.0,
                    us_rate: 0.0,
                    prior_rate: None,
                },
            ],
        };
        assert_eq!(data.verdict(), "well_above");
        assert_eq!(km(45.5, -122.6, 45.5, -122.6), 0.0);
        assert!((km(45.5, -122.6, 45.6, -122.6) - 11.1).abs() < 0.2);
    }
}
