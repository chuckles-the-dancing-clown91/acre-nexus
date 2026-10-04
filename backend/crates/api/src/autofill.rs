//! **Autofill proposals**: after enrichment fetches a property's public
//! record (type, beds, baths, square footage), propose those values for the
//! property and its unit instead of leaving them in a side table. A person
//! reviews and applies; nothing is overwritten silently.
//!
//! The rules are pure so they are tested without a database.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct Proposal {
    /// `property_type` | `year_built` | `unit_beds` | `unit_baths` |
    /// `unit_sqft` | `unit_market_rent`.
    pub field: String,
    pub label: String,
    pub current: Option<String>,
    pub proposed: String,
    /// Where the value came from.
    pub source: String,
}

/// The enrichment's free-text type, as the property's type code.
pub fn normalize_type(raw: &str) -> Option<&'static str> {
    let t = raw.trim().to_lowercase();
    if t.contains("single") || t == "sfr" || t == "house" {
        Some("single_family")
    } else if t.contains("multi")
        || t.contains("duplex")
        || t.contains("triplex")
        || t.contains("fourplex")
    {
        Some("multi_family")
    } else if t.contains("condo") {
        Some("condo")
    } else if t.contains("town") {
        Some("townhome")
    } else if t.contains("commercial") || t.contains("retail") || t.contains("office") {
        Some("commercial")
    } else if t.contains("land") || t.contains("lot") || t.contains("vacant") {
        Some("land")
    } else {
        None
    }
}

/// The record's numbers, and the current values they would replace.
pub struct Facts<'a> {
    pub property_type: &'a str,
    pub detail_type: Option<&'a str>,
    pub detail_beds: Option<i32>,
    pub detail_baths: Option<f64>,
    pub detail_sqft: Option<i32>,
    /// The property's only unit, when it has exactly one.
    pub unit_beds: Option<i32>,
    pub unit_baths: Option<f64>,
    pub unit_sqft: Option<i32>,
    pub single_unit: bool,
    /// The property's year built (0 when unknown) and the record's.
    pub property_year_built: i32,
    pub detail_year_built: Option<i32>,
}

fn baths(b: f64) -> String {
    if b.fract() == 0.0 {
        format!("{}", b as i64)
    } else {
        format!("{b:.1}")
    }
}

/// What to propose: only values that are known and differ from what is set.
pub fn proposals(f: &Facts, source: &str) -> Vec<Proposal> {
    let mut out = Vec::new();
    let mut add = |field: &str, label: &str, current: Option<String>, proposed: String| {
        if current.as_deref() != Some(proposed.as_str()) {
            out.push(Proposal {
                field: field.into(),
                label: label.into(),
                current,
                proposed,
                source: source.into(),
            });
        }
    };
    if let Some(t) = f.detail_type.and_then(normalize_type) {
        add(
            "property_type",
            "Property type",
            (!f.property_type.is_empty()).then(|| f.property_type.to_string()),
            t.to_string(),
        );
    }
    if let Some(y) = f.detail_year_built.filter(|y| (1700..=2100).contains(y)) {
        add(
            "year_built",
            "Year built",
            (f.property_year_built > 0).then(|| f.property_year_built.to_string()),
            y.to_string(),
        );
    }
    if f.single_unit {
        if let Some(b) = f.detail_beds.filter(|b| *b >= 0) {
            add(
                "unit_beds",
                "Bedrooms",
                f.unit_beds.map(|v| v.to_string()),
                b.to_string(),
            );
        }
        if let Some(b) = f.detail_baths.filter(|b| *b > 0.0) {
            add("unit_baths", "Bathrooms", f.unit_baths.map(baths), baths(b));
        }
        if let Some(s) = f.detail_sqft.filter(|s| *s > 0) {
            add(
                "unit_sqft",
                "Square feet",
                f.unit_sqft.map(|v| v.to_string()),
                s.to_string(),
            );
        }
    }
    out
}

/// A market rent for a single-unit property from the latest rent estimate,
/// in whole dollars, when it differs from what's set.
pub fn rent_proposal(
    single_unit: bool,
    unit_market_rent_cents: Option<i64>,
    estimated_rent_cents: Option<i64>,
    source: &str,
) -> Option<Proposal> {
    if !single_unit {
        return None;
    }
    let est = estimated_rent_cents.filter(|c| *c > 0)?;
    let dollars = (est + 50) / 100;
    let current = unit_market_rent_cents.map(|c| ((c + 50) / 100).to_string());
    (current.as_deref() != Some(dollars.to_string().as_str())).then(|| Proposal {
        field: "unit_market_rent".into(),
        label: "Market rent (a month)".into(),
        current,
        proposed: dollars.to_string(),
        source: source.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> Facts<'static> {
        Facts {
            property_type: "multi_family",
            detail_type: Some("Single Family Residence"),
            detail_beds: Some(3),
            detail_baths: Some(2.5),
            detail_sqft: Some(1400),
            unit_beds: Some(3),
            unit_baths: None,
            unit_sqft: Some(1200),
            single_unit: true,
            property_year_built: 0,
            detail_year_built: None,
        }
    }

    #[test]
    fn types_normalize() {
        assert_eq!(
            normalize_type("Single Family Residence"),
            Some("single_family")
        );
        assert_eq!(normalize_type("Duplex"), Some("multi_family"));
        assert_eq!(normalize_type("Condominium"), Some("condo"));
        assert_eq!(normalize_type("Townhouse"), Some("townhome"));
        assert_eq!(normalize_type("Retail"), Some("commercial"));
        assert_eq!(normalize_type("Vacant lot"), Some("land"));
        assert_eq!(normalize_type("Mystery"), None);
    }

    #[test]
    fn only_differences_are_proposed() {
        let p = proposals(&facts(), "county record");
        let fields: Vec<_> = p.iter().map(|x| x.field.as_str()).collect();
        // beds already match; type, baths (unset) and sqft differ.
        assert_eq!(fields, vec!["property_type", "unit_baths", "unit_sqft"]);
        assert_eq!(p[0].proposed, "single_family");
        assert_eq!(p[0].current.as_deref(), Some("multi_family"));
        assert_eq!(p[1].proposed, "2.5");
        assert_eq!(p[1].current, None);
    }

    #[test]
    fn unit_fields_only_for_a_single_unit_property() {
        let mut f = facts();
        f.single_unit = false;
        let p = proposals(&f, "x");
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].field, "property_type");
    }

    #[test]
    fn nothing_to_propose_when_it_all_matches() {
        let mut f = facts();
        f.property_type = "single_family";
        f.unit_baths = Some(2.5);
        f.unit_sqft = Some(1400);
        assert!(proposals(&f, "x").is_empty());
    }

    #[test]
    fn year_built_when_the_record_has_it() {
        let mut f = facts();
        f.detail_year_built = Some(1962);
        let p = proposals(&f, "x");
        let y = p.iter().find(|x| x.field == "year_built").unwrap();
        assert_eq!((y.current.as_deref(), y.proposed.as_str()), (None, "1962"));
        f.property_year_built = 1962;
        assert!(proposals(&f, "x").iter().all(|x| x.field != "year_built"));
        f.detail_year_built = Some(12);
        assert!(proposals(&f, "x").iter().all(|x| x.field != "year_built"));
    }

    #[test]
    fn rent_from_the_estimate() {
        let r = rent_proposal(true, Some(170_000), Some(184_960), "rent estimate").unwrap();
        assert_eq!(
            (r.current.as_deref(), r.proposed.as_str()),
            (Some("1700"), "1850")
        );
        assert!(rent_proposal(true, Some(185_000), Some(184_960), "x").is_none());
        assert!(rent_proposal(false, None, Some(184_960), "x").is_none());
        assert!(rent_proposal(true, None, None, "x").is_none());
    }
}
