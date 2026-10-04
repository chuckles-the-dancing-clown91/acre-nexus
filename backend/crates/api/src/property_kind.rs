//! **Property types.** What a property is decides how it is rented and what
//! the console offers for it:
//!
//! | kind | rented as | units |
//! |---|---|---|
//! | `single_family`, `townhome`, `condo`, `manufactured` | one home | exactly one (made automatically) |
//! | `multi_family`, `commercial` | apartments, suites | as many as the building has |
//! | `campground`, `rv_park` | sites by the night | none: sites live on the site map |
//! | `land` | nothing | none |
//!
//! Each unit can carry its own appliances (assets) and meters.

use crate::error::{ApiError, ApiResult};
use chrono::Utc;
use entity::prelude::{Property, Unit};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter, Set,
};
use uuid::Uuid;

pub const KINDS: &[&str] = &[
    "single_family",
    "townhome",
    "condo",
    "manufactured",
    "multi_family",
    "commercial",
    "campground",
    "rv_park",
    "land",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitMode {
    /// One home, one unit.
    Single,
    /// A building of units.
    Multi,
    /// Sites on a map, booked by stay.
    Sites,
    /// Nothing to rent.
    None,
}

/// A stored or typed spelling, as one of [`KINDS`].
pub fn normalize(raw: &str) -> Option<&'static str> {
    let k: String = raw
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect();
    Some(match k.as_str() {
        "singlefamily" | "house" | "sfh" | "singlefamilyhome" => "single_family",
        "townhome" | "townhouse" => "townhome",
        "condo" | "condominium" => "condo",
        "manufactured" | "mobilehome" | "manufacturedhome" => "manufactured",
        "multifamily" | "apartment" | "apartments" | "duplex" | "triplex" | "fourplex" => {
            "multi_family"
        }
        "commercial" | "office" | "retail" => "commercial",
        "campground" | "camp" => "campground",
        "rvpark" | "rvresort" => "rv_park",
        "land" | "lot" | "acreage" => "land",
        _ => return None,
    })
}

pub fn unit_mode(kind: &str) -> UnitMode {
    match kind {
        "single_family" | "townhome" | "condo" | "manufactured" => UnitMode::Single,
        "multi_family" | "commercial" => UnitMode::Multi,
        "campground" | "rv_park" => UnitMode::Sites,
        "land" => UnitMode::None,
        _ => UnitMode::Multi,
    }
}

/// The kind of a property whose type may be blank or unfamiliar: its type if
/// it's one we know, else guessed from how many units it has.
pub fn effective_kind(property_type: &str, unit_count: i64) -> &'static str {
    normalize(property_type).unwrap_or(if unit_count > 1 {
        "multi_family"
    } else {
        "single_family"
    })
}

pub fn label(kind: &str) -> &'static str {
    match kind {
        "single_family" => "Single-family home",
        "townhome" => "Townhome",
        "condo" => "Condo",
        "manufactured" => "Manufactured home",
        "multi_family" => "Apartments",
        "commercial" => "Commercial",
        "campground" => "Campground",
        "rv_park" => "RV park",
        "land" => "Land",
        _ => "Property",
    }
}

/// Why a unit can't be added to a property of this kind with this many
/// already, or `None` when it can.
pub fn bar_to_adding_unit(kind: &str, existing: u64) -> Option<String> {
    match unit_mode(kind) {
        UnitMode::Multi => None,
        UnitMode::Single if existing == 0 => None,
        UnitMode::Single => Some(format!(
            "A {} is one unit, and it already has it.",
            label(kind).to_lowercase()
        )),
        UnitMode::Sites => {
            Some("Campground and RV sites are drawn on the site map, not added as units.".into())
        }
        UnitMode::None => Some("Land has no units to rent.".into()),
    }
}

/// Parse a typed property type for saving: blank stays blank, a known
/// spelling is made canonical, anything else is refused.
pub fn parse_for_save(raw: &str) -> ApiResult<String> {
    if raw.trim().is_empty() {
        return Ok(String::new());
    }
    normalize(raw).map(str::to_string).ok_or_else(|| {
        ApiError::BadRequest(format!(
            "property type must be one of: {}",
            KINDS.join(", ")
        ))
    })
}

/// The unit a single-family-style property is rented as, made if missing.
pub async fn ensure_home_unit(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    property_id: Uuid,
) -> ApiResult<entity::unit::Model> {
    if let Some(u) = Unit::find()
        .filter(entity::unit::Column::PropertyId.eq(property_id))
        .one(db)
        .await?
    {
        return Ok(u);
    }
    let occupied = Property::find_by_id(property_id)
        .one(db)
        .await?
        .map(|p| p.occupied_units > 0)
        .unwrap_or(false);
    let now = Utc::now();
    Ok(entity::unit::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        property_id: Set(property_id),
        unit_number: Set("Home".into()),
        beds: Set(None),
        baths: Set(None),
        sqft: Set(None),
        market_rent_cents: Set(None),
        status: Set(if occupied { "occupied" } else { "vacant" }.into()),
        floor: Set(None),
        notes: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?)
}

/// How many units a property has on record.
pub async fn unit_count(db: &impl ConnectionTrait, property_id: Uuid) -> ApiResult<u64> {
    Ok(Unit::find()
        .filter(entity::unit::Column::PropertyId.eq(property_id))
        .count(db)
        .await?)
}

/// Which unit a new lease is on. A house has one, so it's picked for you; in a
/// building you choose, and it must belong to this property.
pub async fn resolve_unit(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    property: &entity::property::Model,
    chosen: Option<Uuid>,
) -> ApiResult<Option<Uuid>> {
    let count = unit_count(db, property.id).await?;
    let kind = effective_kind(
        &property.property_type,
        (count as i64).max(property.units as i64),
    );
    if let Some(uid) = chosen {
        Unit::find_by_id(uid)
            .filter(entity::unit::Column::TenantId.eq(tenant_id))
            .filter(entity::unit::Column::PropertyId.eq(property.id))
            .one(db)
            .await?
            .ok_or_else(|| ApiError::BadRequest("that unit isn't on this property".into()))?;
        return Ok(Some(uid));
    }
    match unit_mode(kind) {
        UnitMode::Single => Ok(Some(ensure_home_unit(db, tenant_id, property.id).await?.id)),
        UnitMode::Multi if count > 0 => Err(ApiError::BadRequest(
            "choose which unit this lease is for".into(),
        )),
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spellings_collapse() {
        assert_eq!(normalize("Multi-Family"), Some("multi_family"));
        assert_eq!(normalize("apartment"), Some("multi_family"));
        assert_eq!(normalize("Single Family"), Some("single_family"));
        assert_eq!(normalize("RV park"), Some("rv_park"));
        assert_eq!(normalize("spaceship"), None);
    }

    #[test]
    fn modes() {
        assert_eq!(unit_mode("single_family"), UnitMode::Single);
        assert_eq!(unit_mode("multi_family"), UnitMode::Multi);
        assert_eq!(unit_mode("campground"), UnitMode::Sites);
        assert_eq!(unit_mode("land"), UnitMode::None);
    }

    #[test]
    fn blank_types_are_guessed() {
        assert_eq!(effective_kind("", 12), "multi_family");
        assert_eq!(effective_kind("", 1), "single_family");
        assert_eq!(effective_kind("condo", 40), "condo");
    }

    #[test]
    fn adding_units() {
        assert!(bar_to_adding_unit("multi_family", 30).is_none());
        assert!(bar_to_adding_unit("single_family", 0).is_none());
        assert!(bar_to_adding_unit("single_family", 1).is_some());
        assert!(bar_to_adding_unit("campground", 0).is_some());
        assert!(bar_to_adding_unit("land", 0).is_some());
    }

    #[test]
    fn saving_types() {
        assert_eq!(parse_for_save("").unwrap(), "");
        assert_eq!(parse_for_save("Apartments").unwrap(), "multi_family");
        assert!(parse_for_save("castle").is_err());
    }
}
