//! **Utilities**: who pays for what at a property or unit, worked out from its
//! meters (and, for a provider name, the property's utility record). The
//! lease's utility agreement is built from this, so what the lease says is
//! what the meters say.

use crate::error::ApiResult;
use entity::prelude::{Meter, PropertyUtility};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use serde::Serialize;
use uuid::Uuid;

pub const KINDS: &[&str] = &[
    "electric", "gas", "water", "sewer", "trash", "internet", "other",
];

pub fn kind_label(kind: &str) -> &'static str {
    match kind {
        "electric" => "Electricity",
        "gas" => "Gas",
        "water" => "Water",
        "sewer" => "Sewer",
        "trash" => "Trash and recycling",
        "internet" => "Internet and cable",
        _ => "Other utilities",
    }
}

/// Who pays, in words a lease can use.
pub fn payer_words(paid_by: &str) -> &'static str {
    match paid_by {
        "landlord" => "Landlord",
        "shared" => "Shared between landlord and tenant",
        _ => "Tenant",
    }
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema, PartialEq)]
pub struct UtilityTerm {
    pub kind: String,
    pub label: String,
    /// `tenant` | `landlord` | `shared`.
    pub paid_by: String,
    pub paid_by_label: String,
    pub provider: Option<String>,
    /// The meters this covers (`Unit 4 electric #123`).
    pub meters: Vec<String>,
    pub note: Option<String>,
}

/// Fold one kind's meters into one term: a unit's own meters win over the
/// building's; mixed payers mean shared.
pub fn fold(
    kind: &str,
    unit_meters: &[&entity::meter::Model],
    house_meters: &[&entity::meter::Model],
    fallback_provider: Option<String>,
) -> Option<UtilityTerm> {
    let used = if unit_meters.is_empty() {
        house_meters
    } else {
        unit_meters
    };
    if used.is_empty() {
        return None;
    }
    let first = used[0].paid_by.as_str();
    let paid_by = if used.iter().all(|m| m.paid_by == first) {
        first.to_string()
    } else {
        "shared".to_string()
    };
    let provider = used
        .iter()
        .find_map(|m| m.provider.clone())
        .or(fallback_provider);
    Some(UtilityTerm {
        kind: kind.to_string(),
        label: kind_label(kind).to_string(),
        paid_by_label: payer_words(&paid_by).to_string(),
        paid_by,
        provider,
        meters: used
            .iter()
            .map(|m| match &m.meter_number {
                Some(n) => format!("{} #{n}", m.label),
                None => m.label.clone(),
            })
            .collect(),
        note: used.iter().find_map(|m| m.billing_note.clone()),
    })
}

/// The utility terms for a unit (or the whole property): one per kind that has
/// an active meter.
pub async fn terms(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    property_id: Uuid,
    unit_id: Option<Uuid>,
) -> ApiResult<Vec<UtilityTerm>> {
    let meters: Vec<entity::meter::Model> = Meter::find()
        .filter(entity::meter::Column::TenantId.eq(tenant_id))
        .filter(entity::meter::Column::PropertyId.eq(property_id))
        .filter(entity::meter::Column::Status.eq("active"))
        .all(db)
        .await?;
    let providers: Vec<entity::property_utility::Model> = PropertyUtility::find()
        .filter(entity::property_utility::Column::TenantId.eq(tenant_id))
        .filter(entity::property_utility::Column::PropertyId.eq(property_id))
        .all(db)
        .await?;
    let mut out = vec![];
    for kind in KINDS {
        let unit: Vec<&entity::meter::Model> = meters
            .iter()
            .filter(|m| m.kind == *kind && unit_id.is_some() && m.unit_id == unit_id)
            .collect();
        let house: Vec<&entity::meter::Model> = meters
            .iter()
            .filter(|m| m.kind == *kind && m.unit_id.is_none())
            .collect();
        let fallback = providers
            .iter()
            .find(|u| u.utility_type.to_lowercase().contains(kind))
            .map(|u| u.provider.clone());
        if let Some(t) = fold(kind, &unit, &house, fallback) {
            out.push(t);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn meter(kind: &str, unit: Option<Uuid>, paid_by: &str) -> entity::meter::Model {
        entity::meter::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::nil(),
            property_id: Uuid::nil(),
            unit_id: unit,
            kind: kind.into(),
            label: format!("{kind} meter"),
            meter_number: Some("77".into()),
            location: None,
            provider: None,
            unit_of_measure: None,
            paid_by: paid_by.into(),
            billing_note: None,
            status: "active".into(),
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
        }
    }

    #[test]
    fn unit_meters_win_over_the_buildings() {
        let u = Uuid::new_v4();
        let own = meter("electric", Some(u), "tenant");
        let house = meter("electric", None, "landlord");
        let t = fold("electric", &[&own], &[&house], None).unwrap();
        assert_eq!(t.paid_by, "tenant");
        let t = fold("electric", &[], &[&house], None).unwrap();
        assert_eq!(t.paid_by, "landlord");
    }

    #[test]
    fn mixed_payers_are_shared() {
        let a = meter("water", None, "tenant");
        let b = meter("water", None, "landlord");
        let t = fold("water", &[], &[&a, &b], None).unwrap();
        assert_eq!(t.paid_by, "shared");
        assert_eq!(t.meters, vec!["water meter #77", "water meter #77"]);
    }

    #[test]
    fn nothing_means_no_term_and_provider_falls_back() {
        assert!(fold("gas", &[], &[], None).is_none());
        let m = meter("gas", None, "tenant");
        let t = fold("gas", &[], &[&m], Some("NW Natural".into())).unwrap();
        assert_eq!(t.provider.as_deref(), Some("NW Natural"));
    }
}
