//! **Code-required items**: the recurring checks landlords owe by law or by
//! common code (smoke and CO alarms, fire equipment, water heater straps,
//! lead paint, and so on), as a catalog that seeds maintenance plans per
//! property. Each item says who it applies to and names the rule it comes
//! from; local code is the final word, so the notes say "where required".

use crate::error::ApiResult;
use chrono::{Duration, NaiveDate};
use entity::prelude::{IssueTemplate, MaintenancePlan};
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use serde::Serialize;
use uuid::Uuid;

/// Who an item applies to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applies {
    All,
    /// Buildings with more than one unit.
    Multifamily,
    /// Built before this year.
    BuiltBefore(i32),
    /// Only in these states (postal codes).
    States(&'static [&'static str]),
    /// Multifamily in these states.
    MultifamilyIn(&'static [&'static str]),
}

#[derive(Clone, Copy, Debug)]
pub struct Mandate {
    pub key: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    /// The rule, in a line.
    pub basis: &'static str,
    pub cadence_days: i32,
    pub category: &'static str,
    pub priority: &'static str,
    /// A job kit to start each work order from.
    pub kit_key: Option<&'static str>,
    pub applies: Applies,
    /// `true` when it only matters if the property has the thing (a pool, a
    /// septic tank); shown as optional.
    pub conditional: bool,
}

const SEISMIC: &[&str] = &["CA", "OR", "WA", "AK", "HI", "NV", "UT"];
const RADON: &[&str] = &["IL", "ME", "MN", "MT", "NJ", "FL", "IA", "CO"];
const NY: &[&str] = &["NY"];

pub const MANDATES: &[Mandate] = &[
    Mandate {
        key: "smoke-co-test",
        title: "Test smoke and CO alarms, replace batteries",
        description: "Press-test every smoke and carbon monoxide alarm, swap batteries, \
                      note the manufacture dates, and log it.",
        basis: "Required at every tenancy and at least yearly in most states; many \
                require a working alarm on every level and in every bedroom.",
        cadence_days: 182,
        category: "safety",
        priority: "high",
        kit_key: None,
        applies: Applies::All,
        conditional: false,
    },
    Mandate {
        key: "smoke-alarm-replace",
        title: "Replace smoke alarms past ten years",
        description: "Smoke alarms expire ten years after manufacture; replace any past \
                      date with sealed ten-year units where the code asks for them.",
        basis: "NFPA 72 and state fire codes: ten-year service life.",
        cadence_days: 3650,
        category: "safety",
        priority: "normal",
        kit_key: Some("replace-smoke-co-detectors"),
        applies: Applies::All,
        conditional: false,
    },
    Mandate {
        key: "co-alarm-replace",
        title: "Replace carbon monoxide alarms",
        description: "CO sensors last five to seven years; replace by the date printed \
                      on the unit.",
        basis: "Manufacturer end-of-life, required where fuel-burning appliances or \
                attached garages exist in most states.",
        cadence_days: 2190,
        category: "safety",
        priority: "normal",
        kit_key: Some("replace-smoke-co-detectors"),
        applies: Applies::All,
        conditional: false,
    },
    Mandate {
        key: "fire-extinguishers",
        title: "Inspect and tag fire extinguishers",
        description: "Check gauge, seal and tag on every extinguisher in common areas \
                      and mechanical rooms; service any past its annual date.",
        basis: "NFPA 10 monthly checks and annual service, enforced by local fire marshals \
                for multifamily buildings.",
        cadence_days: 365,
        category: "safety",
        priority: "normal",
        kit_key: None,
        applies: Applies::Multifamily,
        conditional: false,
    },
    Mandate {
        key: "fire-alarm-sprinkler",
        title: "Annual fire alarm and sprinkler inspection",
        description: "A licensed company inspects and tests the fire alarm panel, \
                      devices and sprinkler system and files the report.",
        basis: "NFPA 72 and NFPA 25 annual inspection, required by fire code for \
                buildings with systems.",
        cadence_days: 365,
        category: "safety",
        priority: "high",
        kit_key: None,
        applies: Applies::Multifamily,
        conditional: true,
    },
    Mandate {
        key: "emergency-lighting",
        title: "Test emergency and exit lighting",
        description: "Push-test every emergency light and exit sign; a 90-minute test \
                      once a year.",
        basis: "NFPA 101: monthly function test, annual 90-minute test.",
        cadence_days: 30,
        category: "safety",
        priority: "normal",
        kit_key: None,
        applies: Applies::Multifamily,
        conditional: false,
    },
    Mandate {
        key: "water-heater-straps",
        title: "Check water heater straps and relief valve",
        description: "Two seismic straps in the upper and lower thirds, a discharge pipe \
                      on the temperature-pressure relief valve, and no corrosion.",
        basis: "Required by state law in seismic states (California Health and Safety \
                Code 19211, Oregon and Washington plumbing codes).",
        cadence_days: 365,
        category: "plumbing",
        priority: "normal",
        kit_key: None,
        applies: Applies::States(SEISMIC),
        conditional: false,
    },
    Mandate {
        key: "dryer-vents",
        title: "Clean dryer vents and ducts",
        description: "Clear lint from the dryer duct to the exterior hood; check the \
                      hood flap.",
        basis: "Fire prevention; required yearly in many multifamily fire codes and by \
                most insurers.",
        cadence_days: 365,
        category: "safety",
        priority: "normal",
        kit_key: None,
        applies: Applies::All,
        conditional: false,
    },
    Mandate {
        key: "hvac-service",
        title: "Service heating and cooling",
        description: "Filter, coil, condensate and burner checks before the season.",
        basis: "Habitability codes require working heat; a yearly service keeps the \
                warranty and the furnace safe.",
        cadence_days: 182,
        category: "hvac",
        priority: "normal",
        kit_key: Some("service-hvac"),
        applies: Applies::All,
        conditional: false,
    },
    Mandate {
        key: "gfci-test",
        title: "Test GFCI and AFCI protection",
        description: "Press test and reset on every GFCI outlet and breaker in kitchens, \
                      baths, garages and outdoors.",
        basis: "Electrical code; many habitability inspections check it.",
        cadence_days: 365,
        category: "electrical",
        priority: "low",
        kit_key: None,
        applies: Applies::All,
        conditional: false,
    },
    Mandate {
        key: "locks-and-windows",
        title: "Check deadbolts, window locks and peepholes",
        description: "Keyed deadbolt and viewer on each entry door, a lock on every \
                      operable window, and sliding-door bars where fitted.",
        basis: "Security device statutes (Texas Property Code 92.153, California Civil \
                Code 1941.3) and re-keying between tenants.",
        cadence_days: 365,
        category: "safety",
        priority: "normal",
        kit_key: None,
        applies: Applies::All,
        conditional: false,
    },
    Mandate {
        key: "lead-paint-visual",
        title: "Lead paint visual check (built before 1978)",
        description: "Walk every unit and common area for chipping, peeling or chalking \
                      paint; fix with lead-safe practices and keep the record.",
        basis: "HUD Lead Safe Housing Rule and the EPA RRP rule for pre-1978 housing.",
        cadence_days: 365,
        category: "safety",
        priority: "high",
        kit_key: None,
        applies: Applies::BuiltBefore(1978),
        conditional: false,
    },
    Mandate {
        key: "carpet-assessment",
        title: "Carpet condition and replacement review",
        description: "Rate the carpet in each unit; plan replacement where it's worn past \
                      cleaning.",
        basis: "HUD's seven-year useful-life guideline for carpet, used in deposit \
                disputes; some cities set replacement cycles.",
        cadence_days: 2555,
        category: "flooring",
        priority: "low",
        kit_key: Some("replace-flooring-lvp"),
        applies: Applies::All,
        conditional: false,
    },
    Mandate {
        key: "repaint-cycle",
        title: "Repaint units on the required cycle",
        description: "Repaint apartment interiors every three years, or at turnover.",
        basis: "New York City Housing Maintenance Code 27-2013: paint every three years.",
        cadence_days: 1095,
        category: "paint",
        priority: "low",
        kit_key: Some("repaint-unit"),
        applies: Applies::MultifamilyIn(NY),
        conditional: false,
    },
    Mandate {
        key: "window-guards",
        title: "Window guard notices and checks",
        description: "Send the yearly window-guard notice and check guards where a child \
                      under eleven lives.",
        basis: "New York City Health Code 24 RCNY 12.",
        cadence_days: 365,
        category: "safety",
        priority: "normal",
        kit_key: None,
        applies: Applies::MultifamilyIn(NY),
        conditional: false,
    },
    Mandate {
        key: "bed-bug-report",
        title: "Annual bed bug report",
        description: "Collect the year's infestation history and file the report; give \
                      new tenants the disclosure.",
        basis: "New York City Local Law 69 of 2017.",
        cadence_days: 365,
        category: "pest",
        priority: "low",
        kit_key: None,
        applies: Applies::MultifamilyIn(NY),
        conditional: false,
    },
    Mandate {
        key: "boiler-inspection",
        title: "Boiler inspection",
        description: "A licensed inspector checks the boiler and files with the city.",
        basis: "State and city boiler codes for buildings with central boilers.",
        cadence_days: 365,
        category: "hvac",
        priority: "high",
        kit_key: None,
        applies: Applies::Multifamily,
        conditional: true,
    },
    Mandate {
        key: "elevator-inspection",
        title: "Elevator inspection",
        description: "A licensed inspector tests the elevator; keep the certificate posted.",
        basis: "State elevator safety codes.",
        cadence_days: 365,
        category: "general",
        priority: "high",
        kit_key: None,
        applies: Applies::Multifamily,
        conditional: true,
    },
    Mandate {
        key: "backflow-test",
        title: "Backflow preventer test",
        description: "A certified tester checks the backflow device on irrigation or the \
                      building service and files with the water utility.",
        basis: "Water utility cross-connection rules, yearly in most cities.",
        cadence_days: 365,
        category: "plumbing",
        priority: "normal",
        kit_key: None,
        applies: Applies::All,
        conditional: true,
    },
    Mandate {
        key: "radon-test",
        title: "Radon test",
        description: "A short-term test in the lowest lived-in level; mitigate over \
                      4 pCi/L.",
        basis: "State radon disclosure and testing laws (Illinois, Maine, Minnesota, \
                Montana, New Jersey, Florida, Iowa, Colorado).",
        cadence_days: 730,
        category: "safety",
        priority: "normal",
        kit_key: None,
        applies: Applies::States(RADON),
        conditional: false,
    },
    Mandate {
        key: "gutters-roof",
        title: "Clean gutters and check the roof",
        description: "Clear gutters and downspouts, look for lifted shingles and flashing \
                      gaps before the wet season.",
        basis: "Habitability: weatherproofing and drainage.",
        cadence_days: 365,
        category: "exterior",
        priority: "normal",
        kit_key: None,
        applies: Applies::All,
        conditional: false,
    },
    Mandate {
        key: "moisture-mold",
        title: "Moisture and mold walk-through",
        description: "Check under sinks, around tubs, windows and the water heater for \
                      leaks and growth; fix the source.",
        basis: "Mold is a habitability defect in California (Civil Code 1941.7) and \
                most states' warranty of habitability.",
        cadence_days: 365,
        category: "general",
        priority: "normal",
        kit_key: None,
        applies: Applies::All,
        conditional: false,
    },
    Mandate {
        key: "pool-barrier",
        title: "Pool barrier, gate and drain cover check",
        description: "Self-closing, self-latching gate, barrier height, and compliant \
                      drain covers.",
        basis: "Virginia Graeme Baker Act and state pool safety acts.",
        cadence_days: 365,
        category: "safety",
        priority: "high",
        kit_key: None,
        applies: Applies::All,
        conditional: true,
    },
    Mandate {
        key: "septic-pump",
        title: "Pump and inspect the septic tank",
        description: "Pump the tank and check baffles and the drain field.",
        basis: "County health codes, typically every three to five years.",
        cadence_days: 1095,
        category: "plumbing",
        priority: "normal",
        kit_key: None,
        applies: Applies::All,
        conditional: true,
    },
    Mandate {
        key: "chimney-sweep",
        title: "Chimney and fireplace inspection",
        description: "Sweep and inspect flues, dampers and caps before heating season.",
        basis: "NFPA 211 yearly inspection.",
        cadence_days: 365,
        category: "safety",
        priority: "normal",
        kit_key: None,
        applies: Applies::All,
        conditional: true,
    },
];

/// Whether an item applies to a property.
pub fn applies(m: &Mandate, state: &str, units: i32, year_built: i32) -> bool {
    let st = state.trim().to_uppercase();
    let multifamily = units > 1;
    match m.applies {
        Applies::All => true,
        Applies::Multifamily => multifamily,
        Applies::BuiltBefore(y) => year_built > 0 && year_built < y,
        Applies::States(list) => list.contains(&st.as_str()),
        Applies::MultifamilyIn(list) => multifamily && list.contains(&st.as_str()),
    }
}

/// The items that apply to a property, in catalog order.
pub fn for_property(p: &entity::property::Model) -> Vec<&'static Mandate> {
    let (_, state) = crate::enrichment::runner::city_state(&p.city, &p.state, None);
    MANDATES
        .iter()
        .filter(|m| applies(m, &state, p.units, p.year_built))
        .collect()
}

pub fn by_key(key: &str) -> Option<&'static Mandate> {
    MANDATES.iter().find(|m| m.key == key)
}

/// One item on a property: the catalog entry plus the plan it has, if any.
#[derive(Serialize, schemars::JsonSchema)]
pub struct MandateStatus {
    pub key: String,
    pub title: String,
    pub description: String,
    pub basis: String,
    pub cadence_days: i32,
    pub category: String,
    pub priority: String,
    pub kit_key: Option<String>,
    pub conditional: bool,
    pub plan_id: Option<Uuid>,
    pub next_due_date: Option<String>,
    pub active: Option<bool>,
}

/// The items that apply to a property with where each stands.
pub async fn status(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    p: &entity::property::Model,
) -> ApiResult<Vec<MandateStatus>> {
    let plans = MaintenancePlan::find()
        .filter(entity::maintenance_plan::Column::TenantId.eq(tenant_id))
        .filter(entity::maintenance_plan::Column::PropertyId.eq(p.id))
        .filter(entity::maintenance_plan::Column::MandateKey.is_not_null())
        .all(db)
        .await?;
    Ok(for_property(p)
        .into_iter()
        .map(|m| {
            let plan = plans
                .iter()
                .find(|x| x.mandate_key.as_deref() == Some(m.key));
            MandateStatus {
                key: m.key.into(),
                title: m.title.into(),
                description: m.description.into(),
                basis: m.basis.into(),
                cadence_days: m.cadence_days,
                category: m.category.into(),
                priority: m.priority.into(),
                kit_key: m.kit_key.map(str::to_string),
                conditional: m.conditional,
                plan_id: plan.map(|x| x.id),
                next_due_date: plan.map(|x| x.next_due_date.clone()),
                active: plan.map(|x| x.active),
            }
        })
        .collect())
}

/// Create plans for the items a property is missing: all that apply (the
/// conditional ones aside) when `keys` is empty, else those keys. The first
/// due date is 30 days out, so they show under "To schedule" soon without
/// opening work orders at once. Returns how many were created.
pub async fn apply(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    p: &entity::property::Model,
    keys: &[String],
    today: NaiveDate,
    by: Option<Uuid>,
) -> ApiResult<usize> {
    crate::servicedesk::ensure_kits(db, tenant_id).await?;
    let kits: Vec<entity::issue_template::Model> = IssueTemplate::find()
        .filter(entity::issue_template::Column::TenantId.eq(tenant_id))
        .filter(entity::issue_template::Column::Active.eq(true))
        .all(db)
        .await?;
    let existing: Vec<String> = MaintenancePlan::find()
        .filter(entity::maintenance_plan::Column::TenantId.eq(tenant_id))
        .filter(entity::maintenance_plan::Column::PropertyId.eq(p.id))
        .filter(entity::maintenance_plan::Column::MandateKey.is_not_null())
        .all(db)
        .await?
        .into_iter()
        .filter_map(|x| x.mandate_key)
        .collect();
    let wanted: Vec<&Mandate> = for_property(p)
        .into_iter()
        .filter(|m| {
            if keys.is_empty() {
                !m.conditional
            } else {
                keys.iter().any(|k| k == m.key)
            }
        })
        .filter(|m| !existing.iter().any(|k| k == m.key))
        .collect();
    let now = chrono::Utc::now();
    let mut made = 0;
    for m in wanted {
        let kit_id = m
            .kit_key
            .and_then(|k| kits.iter().find(|x| x.kit_key.as_deref() == Some(k)))
            .map(|x| x.id);
        entity::maintenance_plan::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            property_id: Set(p.id),
            unit_id: Set(None),
            asset_id: Set(None),
            title: Set(m.title.into()),
            description: Set(Some(format!("{}\n\nWhy: {}", m.description, m.basis))),
            category: Set(m.category.into()),
            priority: Set(m.priority.into()),
            cadence_days: Set(m.cadence_days),
            next_due_date: Set((today + Duration::days(30)).to_string()),
            active: Set(true),
            last_ticket_id: Set(None),
            issue_template_id: Set(kit_id),
            created_by: Set(by),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            mandate_key: Set(Some(m.key.into())),
            lead_days: Set(None),
        }
        .insert(db)
        .await?;
        made += 1;
    }
    Ok(made)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_and_rules_apply() {
        let mut keys: Vec<&str> = MANDATES.iter().map(|m| m.key).collect();
        keys.sort_unstable();
        let n = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), n);
        let straps = by_key("water-heater-straps").unwrap();
        assert!(applies(straps, "OR", 1, 1990));
        assert!(!applies(straps, "TX", 1, 1990));
        let lead = by_key("lead-paint-visual").unwrap();
        assert!(applies(lead, "", 1, 1965));
        assert!(!applies(lead, "", 1, 1990));
        assert!(!applies(lead, "", 1, 0));
        let paint = by_key("repaint-cycle").unwrap();
        assert!(applies(paint, "ny", 12, 2000));
        assert!(!applies(paint, "NY", 1, 2000));
        let ext = by_key("fire-extinguishers").unwrap();
        assert!(applies(ext, "OR", 4, 2000));
        assert!(!applies(ext, "OR", 1, 2000));
    }
}
