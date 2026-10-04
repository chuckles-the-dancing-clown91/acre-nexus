//! **Needs attention**: what's on file about a property, turned into to-dos.
//! A permit about to lapse, a policy coming up for renewal, a flood zone with
//! no flood cover, a warranty running out, a water heater at the end of its
//! life, a school zone nobody has confirmed. Each suggestion carries a key;
//! once someone adds it to the action items (or dismisses it) it stops being
//! suggested. Keys that include a date come back next cycle.

use super::permits::is_open;
use super::property_in;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::{Datelike, Duration, NaiveDate, Utc};
use entity::prelude::{
    ActionItem, Asset, InsurancePolicy, PropertyDetail, PropertyPermit, PropertySchool,
};
use rocket::serde::json::Json;
use rocket::{get, State};
use schemars::JsonSchema;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::Serialize;
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Serialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct Suggestion {
    /// Stable id for this suggestion; pass it back as `suggestion_key`.
    pub key: String,
    pub subject_type: String,
    pub subject_id: Option<Uuid>,
    pub title: String,
    pub detail: String,
    pub due_on: Option<String>,
    /// high | normal | low
    pub priority: String,
}

/// Everything the rules look at.
pub struct Facts<'a> {
    pub today: NaiveDate,
    pub detail: Option<&'a entity::property_detail::Model>,
    pub permits: &'a [entity::property_permit::Model],
    pub policies: &'a [entity::insurance_policy::Model],
    pub assets: &'a [entity::asset::Model],
    pub schools: &'a [entity::property_school::Model],
    pub crime: Option<&'a entity::property_crime::Model>,
}

fn day(s: &Option<String>) -> Option<NaiveDate> {
    s.as_deref()
        .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
}

fn nice(d: NaiveDate) -> String {
    d.format("%b %-d, %Y").to_string()
}

/// "Permit B-1" → "permit B-1"; "Water heater" → "water heater". Only the
/// first letter, so numbers and names keep their case.
fn lower_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_lowercase().chain(c).collect(),
        None => String::new(),
    }
}

fn permit_name(p: &entity::property_permit::Model) -> String {
    match &p.permit_number {
        Some(n) => format!("Permit {n}"),
        None => format!("The {} permit", p.kind),
    }
}

fn policy_name(p: &entity::insurance_policy::Model) -> String {
    format!("{} {} policy", p.carrier, p.kind.replace('_', " "))
}

/// Special flood hazard areas: FEMA zones starting with A or V.
pub fn in_flood_zone(zone: &str) -> bool {
    let z = zone.trim().to_uppercase();
    z.starts_with('A') || z.starts_with('V')
}

#[allow(clippy::too_many_arguments)]
fn push(
    out: &mut Vec<Suggestion>,
    key: String,
    subject_type: &str,
    subject_id: Option<Uuid>,
    title: String,
    detail: String,
    due_on: Option<NaiveDate>,
    priority: &str,
) {
    out.push(Suggestion {
        key,
        subject_type: subject_type.into(),
        subject_id,
        title,
        detail,
        due_on: due_on.map(|d| d.to_string()),
        priority: priority.into(),
    });
}

/// The rules. Pure, so they're tested without a database.
pub fn suggest(f: &Facts) -> Vec<Suggestion> {
    let mut out = Vec::new();
    let today = f.today;

    // Crime well above the state's: worth a look at lighting, locks and
    // cameras, and worth knowing before a showing.
    if let Some(c) = f.crime.filter(|c| c.verdict == "well_above") {
        let offenses: Vec<crate::enrichment::data::CrimeOffense> =
            serde_json::from_value(c.offenses.clone()).unwrap_or_default();
        let worst = offenses
            .iter()
            .filter(|o| o.state_rate > 0.0)
            .max_by(|a, b| {
                (a.agency_rate / a.state_rate)
                    .partial_cmp(&(b.agency_rate / b.state_rate))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        push(
            &mut out,
            "crime-high".into(),
            "property",
            None,
            "Crime around here runs well above the state".into(),
            match worst {
                Some(o) => format!(
                    "{} reports {} at {} per 100,000 a year against {} statewide. Walk the \
                     exterior for lighting, locks and sightlines, and mention it on showings.",
                    c.agency_name,
                    o.label.to_lowercase(),
                    o.agency_rate.round(),
                    o.state_rate.round()
                ),
                None => format!(
                    "{} reports violent and property crime well above the state. Walk the \
                     exterior for lighting, locks and sightlines.",
                    c.agency_name
                ),
            },
            None,
            "normal",
        );
    }

    for p in f.permits.iter().filter(|p| is_open(&p.status)) {
        let name = permit_name(p);
        if let Some(exp) = day(&p.expires_on) {
            if exp < today {
                push(
                    &mut out,
                    format!("permit-expired:{}", p.id),
                    "permit",
                    Some(p.id),
                    format!("{name} expired without a final"),
                    format!(
                        "It lapsed {}. Renew it or close it out with {} so the work isn't unpermitted.",
                        nice(exp),
                        p.jurisdiction.as_deref().unwrap_or("the building department")
                    ),
                    Some(today),
                    "high",
                );
            } else if exp <= today + Duration::days(30) {
                push(
                    &mut out,
                    format!("permit-expiring:{}", p.id),
                    "permit",
                    Some(p.id),
                    format!("Get the final inspection on {}", lower_first(&name)),
                    format!("It expires {}.", nice(exp)),
                    Some(exp),
                    "high",
                );
            }
        }
        if let Some(insp) = day(&p.inspection_on) {
            if insp >= today && insp <= today + Duration::days(14) {
                push(
                    &mut out,
                    format!("permit-inspection:{}:{}", p.id, insp),
                    "permit",
                    Some(p.id),
                    format!("Be ready for the inspection on {}", nice(insp)),
                    format!("{name}: {}.", p.description),
                    Some(insp),
                    "normal",
                );
            }
        }
    }

    let active: Vec<_> = f.policies.iter().filter(|p| p.status == "active").collect();
    for p in &active {
        if let Some(exp) = day(&p.expires_on) {
            if exp < today {
                push(
                    &mut out,
                    format!("policy-expired:{}:{}", p.id, exp),
                    "insurance",
                    Some(p.id),
                    format!("The {} has lapsed", policy_name(p)),
                    format!(
                        "It ended {}. Confirm the renewal and upload the new declarations page.",
                        nice(exp)
                    ),
                    Some(today),
                    "high",
                );
            } else if exp <= today + Duration::days(45) {
                push(
                    &mut out,
                    format!("policy-renew:{}:{}", p.id, exp),
                    "insurance",
                    Some(p.id),
                    format!("Renew the {}", policy_name(p)),
                    format!(
                        "It renews {}.{}",
                        nice(exp),
                        p.agent_name
                            .as_deref()
                            .map(|a| format!(" Agent: {a}."))
                            .unwrap_or_default()
                    ),
                    Some(exp - Duration::days(14)),
                    "normal",
                );
            }
        }
    }
    if !active.iter().any(|p| p.kind == "property") {
        push(
            &mut out,
            "policy-none".into(),
            "insurance",
            None,
            "Add the property insurance policy".into(),
            "There's no active property policy on file.".into(),
            None,
            "high",
        );
    }
    if let Some(zone) = f.detail.and_then(|d| d.flood_zone.as_deref()) {
        if in_flood_zone(zone) && !active.iter().any(|p| p.kind == "flood") {
            push(
                &mut out,
                "flood-policy".into(),
                "insurance",
                None,
                "Get a flood policy quote".into(),
                format!(
                    "The parcel is in FEMA flood zone {zone}, and there's no flood policy on file."
                ),
                None,
                "high",
            );
        }
    }

    for a in f.assets.iter().filter(|a| a.status == "active") {
        if let Some(w) = day(&a.warranty_expires) {
            if w >= today && w <= today + Duration::days(60) {
                push(
                    &mut out,
                    format!("warranty:{}:{}", a.id, w),
                    "asset",
                    Some(a.id),
                    format!(
                        "Check the {} before its warranty ends",
                        lower_first(&a.name)
                    ),
                    format!(
                        "Coverage{} ends {}. Anything wrong with it is covered until then.",
                        a.warranty_provider
                            .as_deref()
                            .map(|p| format!(" from {p}"))
                            .unwrap_or_default(),
                        nice(w)
                    ),
                    Some(w - Duration::days(14)),
                    "normal",
                );
            }
        }
        if let (Some(installed), Some(life)) = (day(&a.install_date), a.expected_life_years) {
            let end_year = installed.year() + life;
            if end_year <= today.year() + 1 {
                push(
                    &mut out,
                    format!("replace:{}", a.id),
                    "asset",
                    Some(a.id),
                    format!("Plan to replace the {}", lower_first(&a.name)),
                    format!(
                        "Installed {}; these last about {life} years. Budget for it before it fails.",
                        installed.year()
                    ),
                    None,
                    "low",
                );
            }
        }
    }

    if f.schools.is_empty() {
        push(
            &mut out,
            "schools-none".into(),
            "school",
            None,
            "Add the schools this property is zoned for".into(),
            "Families ask first. The district's boundary lookup has the answer.".into(),
            None,
            "low",
        );
    }
    for s in f
        .schools
        .iter()
        .filter(|s| s.assigned && s.zone_verified_on.is_none())
    {
        push(
            &mut out,
            format!("school-zone:{}", s.id),
            "school",
            Some(s.id),
            format!("Confirm the {} zone", s.name),
            format!(
                "Check the address with {} before a listing names the school.",
                s.district.as_deref().unwrap_or("the district")
            ),
            None,
            "low",
        );
    }

    if f.detail.is_none_or(|d| d.apn.is_none()) {
        push(
            &mut out,
            "parcel-apn".into(),
            "parcel",
            None,
            "Add the parcel number (APN)".into(),
            "Permits, taxes and title all key off it.".into(),
            None,
            "low",
        );
    }

    let rank = |p: &str| match p {
        "high" => 0,
        "normal" => 1,
        _ => 2,
    };
    out.sort_by(|a, b| {
        rank(&a.priority)
            .cmp(&rank(&b.priority))
            .then_with(|| a.due_on.cmp(&b.due_on))
    });
    out
}

/// `GET /properties/<id>/attention` — suggestions not already on the list.
#[rocket_okapi::openapi(tag = "Property Profile")]
#[get("/properties/<id>/attention")]
pub async fn attention(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Vec<Suggestion>>> {
    user.require(Permission::PropertyRead)?;
    let p = property_in(&db, scope.tenant_id, id).await?;
    let t = scope.tenant_id;
    let detail = PropertyDetail::find_by_id(p.id)
        .filter(entity::property_detail::Column::TenantId.eq(t))
        .one(&db)
        .await?;
    let permits = PropertyPermit::find()
        .filter(entity::property_permit::Column::TenantId.eq(t))
        .filter(entity::property_permit::Column::PropertyId.eq(p.id))
        .all(&db)
        .await?;
    let policies = InsurancePolicy::find()
        .filter(entity::insurance_policy::Column::TenantId.eq(t))
        .filter(entity::insurance_policy::Column::PropertyId.eq(p.id))
        .all(&db)
        .await?;
    let assets = Asset::find()
        .filter(entity::asset::Column::TenantId.eq(t))
        .filter(entity::asset::Column::PropertyId.eq(p.id))
        .all(&db)
        .await?;
    let schools = PropertySchool::find()
        .filter(entity::property_school::Column::TenantId.eq(t))
        .filter(entity::property_school::Column::PropertyId.eq(p.id))
        .all(&db)
        .await?;
    let crime = entity::prelude::PropertyCrime::find_by_id(p.id)
        .filter(entity::property_crime::Column::TenantId.eq(t))
        .one(&db)
        .await?;
    let taken: HashSet<String> = ActionItem::find()
        .filter(entity::action_item::Column::TenantId.eq(t))
        .filter(entity::action_item::Column::PropertyId.eq(p.id))
        .filter(entity::action_item::Column::SuggestionKey.is_not_null())
        .all(&db)
        .await?
        .into_iter()
        .filter_map(|a| a.suggestion_key)
        .collect();
    let all = suggest(&Facts {
        today: Utc::now().date_naive(),
        detail: detail.as_ref(),
        permits: &permits,
        policies: &policies,
        assets: &assets,
        schools: &schools,
        crime: crime.as_ref(),
    });
    Ok(Json(
        all.into_iter()
            .filter(|s| !taken.contains(&s.key))
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ts() -> sea_orm::prelude::DateTimeWithTimeZone {
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap().into()
    }
    fn d(s: &str) -> Option<String> {
        Some(s.into())
    }
    fn permit(
        status: &str,
        expires: Option<String>,
        insp: Option<String>,
    ) -> entity::property_permit::Model {
        entity::property_permit::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::nil(),
            property_id: Uuid::nil(),
            unit_id: None,
            permit_number: Some("B-1".into()),
            kind: "building".into(),
            description: "Deck rebuild".into(),
            status: status.into(),
            jurisdiction: Some("City of Portland".into()),
            applied_on: None,
            issued_on: None,
            expires_on: expires,
            inspection_on: insp,
            finaled_on: None,
            contractor_entity_id: None,
            contractor_name: None,
            valuation_cents: None,
            fee_cents: None,
            ticket_id: None,
            document_ids: serde_json::json!([]),
            notes: None,
            created_by: None,
            created_at: ts(),
            updated_at: ts(),
        }
    }
    fn policy(kind: &str, expires: Option<String>) -> entity::insurance_policy::Model {
        entity::insurance_policy::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::nil(),
            property_id: Uuid::nil(),
            kind: kind.into(),
            carrier: "Acme".into(),
            policy_number: None,
            status: "active".into(),
            effective_on: None,
            expires_on: expires,
            premium_cents: None,
            coverage_cents: None,
            deductible_cents: None,
            agent_name: None,
            agent_phone: None,
            agent_email: None,
            document_ids: serde_json::json!([]),
            notes: None,
            created_by: None,
            created_at: ts(),
            updated_at: ts(),
        }
    }
    fn keys(v: &[Suggestion]) -> Vec<String> {
        v.iter()
            .map(|s| s.key.split(':').next().unwrap().to_string())
            .collect()
    }

    #[test]
    fn permits_and_policies() {
        let today = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
        let permits = vec![
            permit("issued", d("2026-02-01"), None),
            permit("issued", d("2026-03-20"), d("2026-03-05")),
            permit("finaled", d("2026-02-01"), None),
        ];
        let policies = vec![policy("property", d("2026-04-01"))];
        let out = suggest(&Facts {
            today,
            detail: None,
            permits: &permits,
            policies: &policies,
            assets: &[],
            schools: &[],
            crime: None,
        });
        let k = keys(&out);
        assert!(k.contains(&"permit-expired".to_string()));
        assert!(k.contains(&"permit-expiring".to_string()));
        assert!(k.contains(&"permit-inspection".to_string()));
        assert!(k.contains(&"policy-renew".to_string()));
        assert!(!k.contains(&"policy-none".to_string()));
        assert!(k.contains(&"parcel-apn".to_string()));
        assert!(k.contains(&"schools-none".to_string()));
        // The finaled permit is left alone.
        assert_eq!(k.iter().filter(|x| *x == "permit-expired").count(), 1);
        // High first.
        assert_eq!(out[0].priority, "high");
        let expiring = out
            .iter()
            .find(|s| s.key.starts_with("permit-expiring"))
            .unwrap();
        assert_eq!(expiring.title, "Get the final inspection on permit B-1");
        // Renewal is due two weeks before the policy ends.
        let renew = out
            .iter()
            .find(|s| s.key.starts_with("policy-renew"))
            .unwrap();
        assert_eq!(renew.due_on.as_deref(), Some("2026-03-18"));
    }

    #[test]
    fn flood_zone_needs_flood_cover() {
        assert!(in_flood_zone("AE"));
        assert!(in_flood_zone(" ve "));
        assert!(!in_flood_zone("X"));
        let today = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
        let detail = entity::property_detail::Model {
            property_id: Uuid::nil(),
            tenant_id: Uuid::nil(),
            beds: None,
            baths: None,
            sqft: None,
            lot_size_sqft: None,
            property_type: None,
            stories: None,
            parking_spaces: None,
            heating: None,
            cooling: None,
            latitude: None,
            longitude: None,
            geocode_accuracy: None,
            matched_address: None,
            apn: Some("R123".into()),
            legal_description: None,
            zoning: None,
            subdivision: None,
            county: None,
            fips: None,
            owner_of_record: None,
            last_sale_date: None,
            last_sale_price_cents: None,
            flood_zone: Some("AE".into()),
            walk_score: None,
            last_enriched_at: None,
            description: None,
            features: serde_json::json!({}),
            created_at: ts(),
            updated_at: ts(),
        };
        let none = suggest(&Facts {
            today,
            detail: Some(&detail),
            permits: &[],
            policies: &[policy("property", None)],
            assets: &[],
            schools: &[],
            crime: None,
        });
        assert!(keys(&none).contains(&"flood-policy".to_string()));
        assert!(!keys(&none).contains(&"parcel-apn".to_string()));
        let covered = suggest(&Facts {
            today,
            detail: Some(&detail),
            permits: &[],
            policies: &[policy("property", None), policy("flood", None)],
            assets: &[],
            schools: &[],
            crime: None,
        });
        assert!(!keys(&covered).contains(&"flood-policy".to_string()));
    }
}
