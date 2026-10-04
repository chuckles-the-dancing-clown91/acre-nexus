//! **The house onboarding checklist** (roadmap area 7): everything a new
//! property needs before it runs itself, each step ticked from the data
//! rather than by hand, in order, with where to go to do it.
//!
//! `GET /properties/<id>/checklist`. Steps: address placed on the map,
//! property record filled in, a photo, units, owner LLC, financing
//! (optional), insurance, utilities, a manager assigned, market rents set,
//! and every unit leased or listed.

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::tenancy::{Access, TenantScope};
use chrono::Utc;
use entity::prelude::{
    Assignment, InsurancePolicy, Lease, Listing, Mortgage, Property, PropertyDetail,
    PropertyUtility, Unit,
};
use rocket::get;
use rocket::serde::json::Json;
use schemars::JsonSchema;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct Step {
    pub key: String,
    pub title: String,
    pub done: bool,
    pub optional: bool,
    /// What's left, or what was found.
    pub detail: String,
    /// Where it gets done (a console path).
    pub href: String,
}

#[derive(Serialize, JsonSchema)]
pub struct Checklist {
    pub property_id: Uuid,
    pub steps: Vec<Step>,
    pub done: usize,
    /// Required steps only.
    pub required: usize,
    pub required_done: usize,
    /// The first required step not done.
    pub next: Option<String>,
}

/// What the checklist reads.
#[derive(Default, Debug, Clone)]
pub struct Facts {
    pub placed: bool,
    pub enriched: bool,
    pub proposals_open: usize,
    pub has_photo: bool,
    pub planned_units: i32,
    pub units: usize,
    pub has_llc: bool,
    pub loans: usize,
    pub insured: bool,
    pub insurance_expired: bool,
    pub utilities: usize,
    pub managers: usize,
    pub units_without_rent: usize,
    pub units_unlet: usize,
    pub listings_public: usize,
}

pub fn steps(id: Uuid, f: &Facts) -> Vec<Step> {
    let p = |tab: &str| format!("/console/properties/{id}{tab}");
    let s =
        |key: &str, title: &str, done: bool, optional: bool, detail: String, href: String| Step {
            key: key.into(),
            title: title.into(),
            done,
            optional,
            detail,
            href,
        };
    vec![
        s(
            "address",
            "Address placed on the map",
            f.placed,
            false,
            if f.placed {
                "Found and mapped.".into()
            } else {
                "Refresh the property data to look the address up.".into()
            },
            p(""),
        ),
        s(
            "record",
            "Property record filled in",
            f.enriched && f.proposals_open == 0,
            false,
            if !f.enriched {
                "The public record hasn't been fetched yet.".into()
            } else if f.proposals_open > 0 {
                format!(
                    "{} suggested value{} to review.",
                    f.proposals_open,
                    if f.proposals_open == 1 { "" } else { "s" }
                )
            } else {
                "Type, year built and size match the record.".into()
            },
            p(""),
        ),
        s(
            "photo",
            "A photo",
            f.has_photo,
            false,
            if f.has_photo {
                "Has a photo.".into()
            } else {
                "Add one, or let the street photo fetch run.".into()
            },
            p(""),
        ),
        s(
            "units",
            "Units set up",
            f.units > 0 && (f.planned_units <= 0 || f.units as i32 >= f.planned_units),
            false,
            if f.units == 0 {
                "No units yet.".into()
            } else if f.planned_units > 0 && (f.units as i32) < f.planned_units {
                format!("{} of {} units added.", f.units, f.planned_units)
            } else {
                format!("{} unit{}.", f.units, if f.units == 1 { "" } else { "s" })
            },
            p(""),
        ),
        s(
            "owner",
            "Owner LLC",
            f.has_llc,
            false,
            if f.has_llc {
                "Owned by an LLC on file.".into()
            } else {
                "Say which LLC owns it.".into()
            },
            p(""),
        ),
        s(
            "financing",
            "Financing",
            f.loans > 0,
            true,
            if f.loans > 0 {
                format!(
                    "{} loan{} on file.",
                    f.loans,
                    if f.loans == 1 { "" } else { "s" }
                )
            } else {
                "Add the mortgage, or skip if it's owned outright.".into()
            },
            p(""),
        ),
        s(
            "insurance",
            "Insurance",
            f.insured && !f.insurance_expired,
            false,
            if !f.insured {
                "No policy on file.".into()
            } else if f.insurance_expired {
                "The policy on file has expired.".into()
            } else {
                "Current policy on file.".into()
            },
            p("?tab=records"),
        ),
        s(
            "utilities",
            "Utilities",
            f.utilities > 0,
            false,
            if f.utilities > 0 {
                format!(
                    "{} provider{} on file.",
                    f.utilities,
                    if f.utilities == 1 { "" } else { "s" }
                )
            } else {
                "Add who provides power, water and the rest.".into()
            },
            p("?tab=records"),
        ),
        s(
            "manager",
            "A manager assigned",
            f.managers > 0,
            false,
            if f.managers > 0 {
                "Someone is responsible for it.".into()
            } else {
                "Assign a property manager.".into()
            },
            p(""),
        ),
        s(
            "rent",
            "Market rent set",
            f.units > 0 && f.units_without_rent == 0,
            false,
            if f.units == 0 {
                "Add units first.".into()
            } else if f.units_without_rent > 0 {
                format!(
                    "{} unit{} without a market rent.",
                    f.units_without_rent,
                    if f.units_without_rent == 1 { "" } else { "s" }
                )
            } else {
                "Every unit has a market rent.".into()
            },
            p(""),
        ),
        s(
            "leasing",
            "Leased or listed",
            f.units > 0 && (f.units_unlet == 0 || f.listings_public > 0),
            false,
            if f.units == 0 {
                "Add units first.".into()
            } else if f.units_unlet == 0 {
                "Every unit is leased.".into()
            } else if f.listings_public > 0 {
                format!("{} empty, listed publicly.", f.units_unlet)
            } else {
                format!(
                    "{} empty unit{} not listed.",
                    f.units_unlet,
                    if f.units_unlet == 1 { "" } else { "s" }
                )
            },
            "/console/listings".into(),
        ),
    ]
}

/// `GET /properties/<id>/checklist` — what this property still needs.
#[rocket_okapi::openapi(tag = "Properties")]
#[get("/properties/<id>/checklist")]
pub async fn checklist(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    id: &str,
) -> ApiResult<Json<Checklist>> {
    user.require(Permission::PropertyRead)?;
    let t = scope.tenant_id;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::NotFound("property not found".into()))?;
    if !access.sees(pid) {
        return Err(ApiError::NotFound("property not found".into()));
    }
    let p = Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(t))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    let detail = PropertyDetail::find()
        .filter(entity::property_detail::Column::TenantId.eq(t))
        .filter(entity::property_detail::Column::PropertyId.eq(pid))
        .one(&db)
        .await?;
    let units = Unit::find()
        .filter(entity::unit::Column::TenantId.eq(t))
        .filter(entity::unit::Column::PropertyId.eq(pid))
        .all(&db)
        .await?;
    let active_leases = Lease::find()
        .filter(entity::lease::Column::TenantId.eq(t))
        .filter(entity::lease::Column::PropertyId.eq(pid))
        .filter(entity::lease::Column::Status.is_in(["active", "notice", "upcoming"]))
        .all(&db)
        .await?;
    let leased_units: std::collections::HashSet<Uuid> =
        active_leases.iter().filter_map(|l| l.unit_id).collect();
    let policies = InsurancePolicy::find()
        .filter(entity::insurance_policy::Column::TenantId.eq(t))
        .filter(entity::insurance_policy::Column::PropertyId.eq(pid))
        .all(&db)
        .await?;
    let today = Utc::now().date_naive().to_string();
    let live_policy = policies.iter().any(|x| {
        x.status != "cancelled" && x.expires_on.as_deref().is_none_or(|e| e >= today.as_str())
    });
    let proposals_open = {
        // The same suggestions the autofill review shows.
        let resp = crate::routes::properties::autofill::proposals_for(&db, t, pid).await?;
        resp.len()
    };
    let facts = Facts {
        placed: detail
            .as_ref()
            .is_some_and(|d| d.latitude.is_some() && d.longitude.is_some()),
        enriched: detail
            .as_ref()
            .is_some_and(|d| d.last_enriched_at.is_some()),
        proposals_open,
        has_photo: p.image_url.as_deref().is_some_and(|u| !u.is_empty()),
        planned_units: p.units,
        units: units.len(),
        has_llc: p.llc_id.is_some(),
        loans: Mortgage::find()
            .filter(entity::mortgage::Column::TenantId.eq(t))
            .filter(entity::mortgage::Column::PropertyId.eq(pid))
            .count(&db)
            .await? as usize,
        insured: !policies.is_empty(),
        insurance_expired: !policies.is_empty() && !live_policy,
        utilities: PropertyUtility::find()
            .filter(entity::property_utility::Column::TenantId.eq(t))
            .filter(entity::property_utility::Column::PropertyId.eq(pid))
            .count(&db)
            .await? as usize,
        managers: Assignment::find()
            .filter(entity::assignment::Column::TenantId.eq(t))
            .filter(entity::assignment::Column::SubjectType.eq("property"))
            .filter(entity::assignment::Column::SubjectId.eq(pid))
            .count(&db)
            .await? as usize,
        units_without_rent: units
            .iter()
            .filter(|u| u.market_rent_cents.is_none_or(|c| c <= 0))
            .count(),
        units_unlet: units
            .iter()
            .filter(|u| !leased_units.contains(&u.id))
            .count(),
        listings_public: Listing::find()
            .filter(entity::listing::Column::TenantId.eq(t))
            .filter(entity::listing::Column::PropertyId.eq(pid))
            .filter(entity::listing::Column::IsPublic.eq(true))
            .count(&db)
            .await? as usize,
    };
    let steps = steps(pid, &facts);
    let required: Vec<&Step> = steps.iter().filter(|s| !s.optional).collect();
    Ok(Json(Checklist {
        property_id: pid,
        done: steps.iter().filter(|s| s.done).count(),
        required: required.len(),
        required_done: required.iter().filter(|s| s.done).count(),
        next: required.iter().find(|s| !s.done).map(|s| s.key.clone()),
        steps,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn by_key<'a>(s: &'a [Step], k: &str) -> &'a Step {
        s.iter().find(|x| x.key == k).unwrap()
    }

    #[test]
    fn a_brand_new_house() {
        let s = steps(
            Uuid::nil(),
            &Facts {
                planned_units: 1,
                ..Default::default()
            },
        );
        assert_eq!(s.len(), 11);
        assert!(s.iter().all(|x| !x.done));
        assert!(by_key(&s, "financing").optional);
        assert_eq!(by_key(&s, "units").detail, "No units yet.");
    }

    #[test]
    fn a_running_fourplex() {
        let f = Facts {
            placed: true,
            enriched: true,
            proposals_open: 0,
            has_photo: true,
            planned_units: 4,
            units: 4,
            has_llc: true,
            loans: 0,
            insured: true,
            insurance_expired: false,
            utilities: 3,
            managers: 1,
            units_without_rent: 0,
            units_unlet: 1,
            listings_public: 1,
        };
        let s = steps(Uuid::nil(), &f);
        assert!(s.iter().filter(|x| !x.optional).all(|x| x.done), "{s:?}");
        assert!(!by_key(&s, "financing").done);
        assert_eq!(by_key(&s, "leasing").detail, "1 empty, listed publicly.");
    }

    #[test]
    fn partial_states_say_whats_left() {
        let f = Facts {
            enriched: true,
            proposals_open: 2,
            planned_units: 4,
            units: 2,
            insured: true,
            insurance_expired: true,
            units_without_rent: 1,
            units_unlet: 2,
            ..Default::default()
        };
        let s = steps(Uuid::nil(), &f);
        assert_eq!(by_key(&s, "record").detail, "2 suggested values to review.");
        assert_eq!(by_key(&s, "units").detail, "2 of 4 units added.");
        assert_eq!(
            by_key(&s, "insurance").detail,
            "The policy on file has expired."
        );
        assert_eq!(by_key(&s, "rent").detail, "1 unit without a market rent.");
        assert_eq!(by_key(&s, "leasing").detail, "2 empty units not listed.");
    }
}
