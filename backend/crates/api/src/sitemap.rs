//! Site-map rules, kept pure so they are tested without a database: geometry
//! validation, measurements, campsite attributes, and the OpenStreetMap-style
//! GeoJSON export and import.
//!
//! Campsite vocabulary follows OpenStreetMap: a site is `tourism=camp_pitch`
//! inside a `tourism=camp_site` (or `caravan_site`) area, so an exported map
//! carries OSM-style tags and a park's OSM pitches import as sites.

use serde_json::{json, Map, Value};

pub const MAP_KINDS: &[&str] = &["apartment", "campground", "rv_park", "other"];
pub const BASE_LAYERS: &[&str] = &["satellite", "streets", "plan", "grid"];
pub const FEATURE_KINDS: &[&str] = &[
    "building", "unit", "site", "amenity", "road", "boundary", "parking", "water", "label",
];
pub const SITE_TYPES: &[&str] = &["tent", "rv", "cabin", "glamping", "group", "other"];
pub const AMENITIES: &[&str] = &[
    "bath_house",
    "dump_station",
    "laundry",
    "fire_ring",
    "playground",
    "office",
    "pool",
    "trash",
    "water_spigot",
    "other",
];
pub const MAX_FEATURES: usize = 2000;
const MAX_VERTICES: usize = 1000;

/// Which GeoJSON geometry types a feature kind may use.
fn allowed_geometry(kind: &str) -> &'static [&'static str] {
    match kind {
        "building" | "unit" | "parking" | "water" => &["Polygon", "MultiPolygon"],
        "site" => &["Point", "Polygon"],
        "amenity" | "label" => &["Point", "Polygon"],
        "road" => &["LineString", "MultiLineString"],
        "boundary" => &["Polygon", "MultiPolygon", "LineString"],
        _ => &[],
    }
}

fn position(v: &Value) -> Result<(f64, f64), String> {
    let a = v.as_array().ok_or("a coordinate must be [lng, lat]")?;
    if a.len() < 2 {
        return Err("a coordinate must be [lng, lat]".into());
    }
    let lng = a[0].as_f64().ok_or("coordinates must be numbers")?;
    let lat = a[1].as_f64().ok_or("coordinates must be numbers")?;
    if !(-180.0..=180.0).contains(&lng) || !(-90.0..=90.0).contains(&lat) {
        return Err("coordinate out of range (lng -180..180, lat -90..90)".into());
    }
    Ok((lng, lat))
}

fn line(v: &Value) -> Result<Vec<(f64, f64)>, String> {
    let a = v.as_array().ok_or("a line must be an array of positions")?;
    if a.len() > MAX_VERTICES {
        return Err(format!("too many points (limit {MAX_VERTICES})"));
    }
    a.iter().map(position).collect()
}

fn ring(v: &Value) -> Result<Vec<(f64, f64)>, String> {
    let pts = line(v)?;
    if pts.len() < 4 {
        return Err("a polygon ring needs at least 3 corners and must close".into());
    }
    if pts.first() != pts.last() {
        return Err("a polygon ring must end where it starts".into());
    }
    Ok(pts)
}

/// Check a feature's geometry: a type the kind allows, in-range coordinates,
/// closed rings, and a sane size.
pub fn validate_geometry(kind: &str, g: &Value) -> Result<(), String> {
    let t = g["type"].as_str().ok_or("geometry needs a type")?;
    if !allowed_geometry(kind).contains(&t) {
        return Err(format!(
            "a {kind} cannot be a {t} (use {})",
            allowed_geometry(kind).join(" or ")
        ));
    }
    let c = &g["coordinates"];
    match t {
        "Point" => {
            position(c)?;
        }
        "LineString" => {
            if line(c)?.len() < 2 {
                return Err("a line needs at least 2 points".into());
            }
        }
        "MultiLineString" => {
            for l in c.as_array().ok_or("coordinates must be an array")? {
                if line(l)?.len() < 2 {
                    return Err("a line needs at least 2 points".into());
                }
            }
        }
        "Polygon" => {
            for r in c.as_array().ok_or("coordinates must be an array")? {
                ring(r)?;
            }
            if c.as_array().is_none_or(|a| a.is_empty()) {
                return Err("a polygon needs an outer ring".into());
            }
        }
        "MultiPolygon" => {
            for p in c.as_array().ok_or("coordinates must be an array")? {
                for r in p.as_array().ok_or("polygon must be an array of rings")? {
                    ring(r)?;
                }
            }
        }
        _ => return Err(format!("unsupported geometry {t}")),
    }
    Ok(())
}

/// Ground area of a polygon's outer ring in square metres (equirectangular
/// projection about the ring's centre: accurate to well under 1% at site scale).
pub fn polygon_area_m2(g: &Value) -> Option<f64> {
    if g["type"] != "Polygon" {
        return None;
    }
    let outer = ring(g["coordinates"].as_array()?.first()?).ok()?;
    let lat0 = outer.iter().map(|p| p.1).sum::<f64>() / outer.len() as f64;
    let (kx, ky) = metres_per_degree(lat0);
    let pts: Vec<(f64, f64)> = outer.iter().map(|p| (p.0 * kx, p.1 * ky)).collect();
    let mut s = 0.0;
    for w in pts.windows(2) {
        s += w[0].0 * w[1].1 - w[1].0 * w[0].1;
    }
    Some((s / 2.0).abs())
}

/// Length of a line in metres.
pub fn line_length_m(g: &Value) -> Option<f64> {
    if g["type"] != "LineString" {
        return None;
    }
    let pts = line(&g["coordinates"]).ok()?;
    let lat0 = pts.iter().map(|p| p.1).sum::<f64>() / pts.len().max(1) as f64;
    let (kx, ky) = metres_per_degree(lat0);
    Some(
        pts.windows(2)
            .map(|w| (((w[1].0 - w[0].0) * kx).powi(2) + ((w[1].1 - w[0].1) * ky).powi(2)).sqrt())
            .sum(),
    )
}

fn metres_per_degree(lat: f64) -> (f64, f64) {
    let ky = 111_320.0;
    (ky * lat.to_radians().cos(), ky)
}

/// The four plan-image corners: `[lng, lat]` each, in range.
pub fn validate_corners(v: &Value) -> Result<(), String> {
    let a = v
        .as_array()
        .ok_or("plan corners must be four [lng, lat] pairs")?;
    if a.len() != 4 {
        return Err("plan corners must be four [lng, lat] pairs".into());
    }
    for p in a {
        position(p)?;
    }
    Ok(())
}

fn num(v: &Value, key: &str, lo: f64, hi: f64) -> Result<(), String> {
    match v.get(key) {
        None | Some(Value::Null) => Ok(()),
        Some(x) => match x.as_f64() {
            Some(n) if (lo..=hi).contains(&n) => Ok(()),
            _ => Err(format!("{key} must be a number from {lo} to {hi}")),
        },
    }
}

fn boolean(v: &Value, key: &str) -> Result<(), String> {
    match v.get(key) {
        None | Some(Value::Null) | Some(Value::Bool(_)) => Ok(()),
        _ => Err(format!("{key} must be true or false")),
    }
}

/// Check kind-specific attributes. Unknown keys are kept (a park may track
/// its own), but known ones are range-checked.
pub fn validate_attrs(kind: &str, a: &Value) -> Result<(), String> {
    if !a.is_object() {
        return Err("attrs must be an object".into());
    }
    if a.to_string().len() > 8000 {
        return Err("attrs are too large".into());
    }
    match kind {
        "site" => {
            if let Some(t) = a["site_type"].as_str() {
                if !SITE_TYPES.contains(&t) {
                    return Err(format!(
                        "site_type must be one of {}",
                        SITE_TYPES.join(", ")
                    ));
                }
            }
            num(a, "max_length_ft", 0.0, 200.0)?;
            num(a, "max_guests", 0.0, 100.0)?;
            num(a, "power_amps", 0.0, 100.0)?;
            num(a, "rate_cents_night", 0.0, 10_000_000.0)?;
            num(a, "rate_cents_week", 0.0, 10_000_000.0)?;
            num(a, "rate_cents_month", 0.0, 10_000_000.0)?;
            for k in ["pull_through", "water", "sewer", "shade", "ada", "pets"] {
                boolean(a, k)?;
            }
            if let Some(s) = a["site_status"].as_str() {
                if !["available", "booked", "blocked", "out_of_service"].contains(&s) {
                    return Err(
                        "site_status must be available, booked, blocked or out_of_service".into(),
                    );
                }
            }
        }
        "amenity" => {
            if let Some(t) = a["amenity"].as_str() {
                if !AMENITIES.contains(&t) {
                    return Err(format!("amenity must be one of {}", AMENITIES.join(", ")));
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// OSM-style tags for a feature, as `properties` of an exported GeoJSON feature.
pub fn osm_tags(kind: &str, name: Option<&str>, attrs: &Value) -> Map<String, Value> {
    let mut t = Map::new();
    let mut put = |k: &str, v: Value| {
        t.insert(k.to_string(), v);
    };
    if let Some(n) = name.filter(|n| !n.is_empty()) {
        put("name", json!(n));
    }
    let yn = |b: bool| json!(if b { "yes" } else { "no" });
    match kind {
        "site" => {
            put("tourism", json!("camp_pitch"));
            match attrs["site_type"].as_str() {
                Some("rv") => put("caravans", json!("yes")),
                Some("tent") => put("tents", json!("yes")),
                Some("cabin") => put("tourism", json!("chalet")),
                _ => {}
            }
            if let Some(n) = attrs["max_guests"].as_f64() {
                put("capacity", json!(n as i64));
            }
            if let Some(n) = attrs["max_length_ft"].as_f64() {
                put("maxlength", json!(format!("{} ft", n as i64)));
            }
            if let Some(b) = attrs["power_amps"].as_f64() {
                put("power_supply", yn(b > 0.0));
                if b > 0.0 {
                    put("power_supply:amperage", json!(b as i64));
                }
            }
            if let Some(b) = attrs["water"].as_bool() {
                put("drinking_water", yn(b));
            }
            if let Some(b) = attrs["sewer"].as_bool() {
                put("sanitary_dump_station", yn(b));
            }
            if let Some(b) = attrs["pets"].as_bool() {
                put("dog", yn(b));
            }
            if let Some(b) = attrs["ada"].as_bool() {
                put("wheelchair", yn(b));
            }
            if let Some(b) = attrs["pull_through"].as_bool() {
                put("drive_through", yn(b));
            }
        }
        "amenity" => match attrs["amenity"].as_str() {
            Some("bath_house") => put("amenity", json!("shower")),
            Some("dump_station") => put("amenity", json!("sanitary_dump_station")),
            Some("laundry") => put("shop", json!("laundry")),
            Some("fire_ring") => put("leisure", json!("firepit")),
            Some("playground") => put("leisure", json!("playground")),
            Some("pool") => put("leisure", json!("swimming_pool")),
            Some("trash") => put("amenity", json!("waste_basket")),
            Some("water_spigot") => put("amenity", json!("drinking_water")),
            Some("office") => put("office", json!("yes")),
            _ => {}
        },
        "building" => put("building", json!("yes")),
        "unit" => {
            put("building", json!("apartments"));
            put("indoor", json!("room"));
        }
        "road" => put("highway", json!("service")),
        "boundary" => put("tourism", json!("camp_site")),
        "parking" => put("amenity", json!("parking")),
        "water" => put("natural", json!("water")),
        _ => {}
    }
    t
}

/// The inverse: pick a feature kind (and attributes) out of OSM-style tags.
/// `None` for anything that is not a part of a site map.
pub fn kind_from_tags(tags: &Value) -> Option<(&'static str, Value)> {
    let s = |k: &str| tags[k].as_str();
    let mut attrs = Map::new();
    let kind = if s("tourism") == Some("camp_pitch") || s("tourism") == Some("caravan_pitch") {
        if let Some(c) = tags["capacity"]
            .as_str()
            .and_then(|c| c.parse::<f64>().ok())
        {
            attrs.insert("max_guests".into(), json!(c));
        }
        if s("caravans") == Some("yes") {
            attrs.insert("site_type".into(), json!("rv"));
        } else if s("tents") == Some("yes") {
            attrs.insert("site_type".into(), json!("tent"));
        }
        if let Some(a) = tags["power_supply:amperage"]
            .as_str()
            .and_then(|a| a.parse::<f64>().ok())
        {
            attrs.insert("power_amps".into(), json!(a));
        }
        if s("drinking_water") == Some("yes") {
            attrs.insert("water".into(), json!(true));
        }
        "site"
    } else if s("tourism") == Some("camp_site") || s("tourism") == Some("caravan_site") {
        "boundary"
    } else if s("highway").is_some() {
        "road"
    } else if s("amenity") == Some("parking") {
        "parking"
    } else if s("natural") == Some("water") {
        "water"
    } else if s("building").is_some() {
        "building"
    } else if s("amenity") == Some("shower") {
        attrs.insert("amenity".into(), json!("bath_house"));
        "amenity"
    } else if s("amenity") == Some("sanitary_dump_station") {
        attrs.insert("amenity".into(), json!("dump_station"));
        "amenity"
    } else if s("leisure") == Some("playground") {
        attrs.insert("amenity".into(), json!("playground"));
        "amenity"
    } else if s("leisure") == Some("firepit") {
        attrs.insert("amenity".into(), json!("fire_ring"));
        "amenity"
    } else {
        return None;
    };
    Some((kind, Value::Object(attrs)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(lng: f64, lat: f64, d: f64) -> Value {
        json!({ "type": "Polygon", "coordinates": [[
            [lng, lat], [lng + d, lat], [lng + d, lat + d], [lng, lat + d], [lng, lat]
        ]]})
    }

    #[test]
    fn geometry_must_fit_the_kind() {
        assert!(validate_geometry("unit", &square(-117.0, 34.0, 0.0001)).is_ok());
        assert!(validate_geometry(
            "site",
            &json!({ "type": "Point", "coordinates": [-117.0, 34.0] })
        )
        .is_ok());
        assert!(validate_geometry(
            "unit",
            &json!({ "type": "Point", "coordinates": [-117.0, 34.0] })
        )
        .is_err());
        assert!(validate_geometry("road", &square(0.0, 0.0, 1.0)).is_err());
        assert!(validate_geometry(
            "road",
            &json!({ "type": "LineString", "coordinates": [[0, 0], [1, 1]] })
        )
        .is_ok());
    }

    #[test]
    fn bad_coordinates_and_open_rings_are_refused() {
        assert!(validate_geometry(
            "site",
            &json!({ "type": "Point", "coordinates": [200.0, 0.0] })
        )
        .is_err());
        assert!(validate_geometry(
            "site",
            &json!({ "type": "Point", "coordinates": ["a", 0.0] })
        )
        .is_err());
        let open = json!({ "type": "Polygon", "coordinates": [[[0, 0], [1, 0], [1, 1], [0, 1]]] });
        assert!(validate_geometry("building", &open).is_err());
        let tiny = json!({ "type": "Polygon", "coordinates": [[[0, 0], [1, 0], [0, 0]]] });
        assert!(validate_geometry("building", &tiny).is_err());
        assert!(validate_geometry("building", &json!({ "coordinates": [] })).is_err());
    }

    #[test]
    fn measurements_are_close() {
        // ~0.0001 degrees is about 11.1 m on a side at the equator.
        let a = polygon_area_m2(&square(0.0, 0.0, 0.0001)).unwrap();
        assert!((a - 123.9).abs() < 3.0, "{a}");
        let l = line_length_m(
            &json!({ "type": "LineString", "coordinates": [[0.0, 0.0], [0.0, 0.001]] }),
        )
        .unwrap();
        assert!((l - 111.3).abs() < 1.0, "{l}");
        assert!(polygon_area_m2(&json!({ "type": "Point", "coordinates": [0, 0] })).is_none());
    }

    #[test]
    fn campsite_attributes_are_range_checked() {
        let ok = json!({ "site_type": "rv", "max_length_ft": 40, "power_amps": 50, "pull_through": true, "rate_cents_night": 4500 });
        assert!(validate_attrs("site", &ok).is_ok());
        assert!(validate_attrs("site", &json!({ "site_type": "yurt" })).is_err());
        assert!(validate_attrs("site", &json!({ "power_amps": 9999 })).is_err());
        assert!(validate_attrs("site", &json!({ "pets": "yes" })).is_err());
        assert!(validate_attrs("site", &json!({ "site_status": "closed" })).is_err());
        assert!(validate_attrs("site", &json!([1])).is_err());
        assert!(validate_attrs("amenity", &json!({ "amenity": "spaceport" })).is_err());
        assert!(validate_attrs("unit", &json!({ "anything": 1 })).is_ok());
    }

    #[test]
    fn osm_tags_use_camp_pitch() {
        let t = osm_tags(
            "site",
            Some("A12"),
            &json!({ "site_type": "rv", "power_amps": 50, "water": true, "max_length_ft": 40 }),
        );
        assert_eq!(t["tourism"], "camp_pitch");
        assert_eq!(t["name"], "A12");
        assert_eq!(t["caravans"], "yes");
        assert_eq!(t["power_supply:amperage"], 50);
        assert_eq!(t["drinking_water"], "yes");
        assert_eq!(t["maxlength"], "40 ft");
        assert_eq!(
            osm_tags("boundary", None, &json!({}))["tourism"],
            "camp_site"
        );
        assert_eq!(
            osm_tags("amenity", None, &json!({ "amenity": "bath_house" }))["amenity"],
            "shower"
        );
    }

    #[test]
    fn osm_import_round_trips_the_basics() {
        let (k, a) = kind_from_tags(&json!({ "tourism": "camp_pitch", "caravans": "yes", "capacity": "6", "power_supply:amperage": "30" })).unwrap();
        assert_eq!(k, "site");
        assert_eq!(a["site_type"], "rv");
        assert_eq!(a["max_guests"], 6.0);
        assert_eq!(a["power_amps"], 30.0);
        assert_eq!(
            kind_from_tags(&json!({ "tourism": "camp_site" }))
                .unwrap()
                .0,
            "boundary"
        );
        assert_eq!(
            kind_from_tags(&json!({ "amenity": "shower" })).unwrap().1["amenity"],
            "bath_house"
        );
        assert!(kind_from_tags(&json!({ "shop": "bakery" })).is_none());
    }

    #[test]
    fn plan_corners() {
        assert!(validate_corners(&json!([[0, 1], [1, 1], [1, 0], [0, 0]])).is_ok());
        assert!(validate_corners(&json!([[0, 1], [1, 1]])).is_err());
        assert!(validate_corners(&json!([[0, 1], [1, 1], [1, 0], [0, 999]])).is_err());
    }
}
