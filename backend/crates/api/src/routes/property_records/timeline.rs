//! The property's history in one list, newest first: when it was built and
//! bought, every estimate and tax assessment, leases and listings, loans,
//! deeds and liens, and permits.

use super::property_in;
use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{
    InsurancePolicy, Lease, Lien, Listing, Mortgage, Ownership, PropertyDetail, PropertyPermit,
    PropertyTax, PropertyValuation,
};
use rocket::serde::json::Json;
use rocket::{get, State};
use schemars::JsonSchema;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::Serialize;

#[derive(Serialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct Event {
    /// `YYYY-MM-DD`.
    pub date: String,
    /// built | sale | estimate | tax | lease | listing | loan | deed | lien |
    /// permit | insurance
    pub kind: String,
    pub title: String,
    pub detail: Option<String>,
    pub amount_cents: Option<i64>,
    pub amount_label: Option<String>,
    /// The amount is a monthly rent.
    pub monthly: bool,
}

fn ev(
    date: &str,
    kind: &str,
    title: impl Into<String>,
    detail: Option<String>,
    amount: Option<i64>,
    monthly: bool,
) -> Event {
    // Keep only the date part of a timestamp; anything else isn't a date.
    let d: String = date.chars().take(10).collect();
    Event {
        date: d,
        kind: kind.into(),
        title: title.into(),
        detail,
        amount_cents: amount,
        amount_label: amount.map(|c| {
            if monthly {
                format!("{}/mo", usd(c))
            } else {
                usd(c)
            }
        }),
        monthly,
    }
}

fn is_date(s: &str) -> bool {
    chrono::NaiveDate::parse_from_str(&s.chars().take(10).collect::<String>(), "%Y-%m-%d").is_ok()
}

pub struct Sources<'a> {
    pub property: &'a entity::property::Model,
    pub detail: Option<&'a entity::property_detail::Model>,
    pub valuations: &'a [entity::property_valuation::Model],
    pub taxes: &'a [entity::property_tax::Model],
    pub leases: &'a [entity::lease::Model],
    pub listings: &'a [entity::listing::Model],
    pub mortgages: &'a [entity::mortgage::Model],
    pub ownership: &'a [entity::ownership::Model],
    pub liens: &'a [entity::lien::Model],
    pub permits: &'a [entity::property_permit::Model],
    pub policies: &'a [entity::insurance_policy::Model],
}

/// Pure: merge everything into one list, newest first. Entries without a
/// usable date are left out.
pub fn build(s: &Sources) -> Vec<Event> {
    let mut out = Vec::new();
    if s.property.year_built > 0 {
        out.push(ev(
            &format!("{}-01-01", s.property.year_built),
            "built",
            "Built",
            None,
            None,
            false,
        ));
    }
    if let (Some(d), true) = (
        s.property.acquired_on.as_deref(),
        s.property.acquired_on.is_some(),
    ) {
        out.push(ev(
            d,
            "sale",
            "Acquired",
            None,
            s.property.purchase_price_cents,
            false,
        ));
    }
    if let Some(d) = s.detail.and_then(|d| d.last_sale_date.as_deref()) {
        out.push(ev(
            d,
            "sale",
            "Sold",
            Some("Public record".into()),
            s.detail.and_then(|d| d.last_sale_price_cents),
            false,
        ));
    }
    for v in s.valuations {
        out.push(ev(
            &v.as_of,
            "estimate",
            "Value estimate",
            v.estimated_rent_cents
                .map(|r| format!("Rent estimate {}/mo", usd(r))),
            v.estimated_value_cents,
            false,
        ));
    }
    for t in s.taxes {
        out.push(ev(
            &format!("{}-01-01", t.tax_year),
            "tax",
            format!("{} tax assessment", t.tax_year),
            t.assessed_value_cents
                .map(|a| format!("Assessed at {}", usd(a))),
            t.tax_amount_cents,
            false,
        ));
    }
    for l in s.leases {
        out.push(ev(
            &l.start_date,
            "lease",
            "Lease started",
            l.end_date.as_ref().map(|e| format!("Through {e}")),
            Some(l.rent_cents),
            true,
        ));
    }
    for l in s.listings {
        out.push(ev(
            &l.created_at.to_rfc3339(),
            "listing",
            "Listed for rent",
            Some(l.status.clone()),
            Some(l.rent_cents),
            true,
        ));
    }
    for m in s.mortgages {
        if let Some(d) = &m.start_date {
            out.push(ev(
                d,
                "loan",
                format!("{} loan", m.kind.replace('_', " ")),
                Some(m.status.clone()),
                m.original_amount_cents,
                false,
            ));
        }
    }
    for o in s.ownership {
        if let Some(d) = &o.deed_recorded_date {
            out.push(ev(
                d,
                "deed",
                format!("Deed to {}", o.owner_name),
                o.deed_type.clone(),
                None,
                false,
            ));
        }
    }
    for l in s.liens {
        if let Some(d) = &l.recorded_date {
            out.push(ev(
                d,
                "lien",
                format!("{} lien", l.kind.replace('_', " ")),
                Some(format!("{} · {}", l.lienholder_name, l.status)),
                l.amount_cents,
                false,
            ));
        }
    }
    for p in s.permits {
        let what = p.permit_number.as_deref().map_or_else(
            || p.description.clone(),
            |n| format!("{} (#{n})", p.description),
        );
        if let Some(d) = &p.issued_on {
            out.push(ev(
                d,
                "permit",
                "Permit issued",
                Some(what.clone()),
                p.fee_cents,
                false,
            ));
        }
        if let Some(d) = &p.finaled_on {
            out.push(ev(d, "permit", "Permit finaled", Some(what), None, false));
        }
    }
    for p in s.policies {
        if let Some(d) = &p.effective_on {
            out.push(ev(
                d,
                "insurance",
                format!("{} policy started", p.carrier),
                Some(p.kind.replace('_', " ")),
                p.premium_cents,
                false,
            ));
        }
    }
    out.retain(|e| is_date(&e.date));
    // Newest first; on one day, the order they were pushed stays.
    out.sort_by(|a, b| b.date.cmp(&a.date));
    out
}

/// `GET /properties/<id>/timeline` — everything that has happened to the
/// property, newest first.
#[rocket_okapi::openapi(tag = "Property Profile")]
#[get("/properties/<id>/timeline")]
pub async fn timeline(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Vec<Event>>> {
    user.require(Permission::PropertyRead)?;
    let p = property_in(&db, scope.tenant_id, id).await?;
    let t = scope.tenant_id;
    let detail = PropertyDetail::find_by_id(p.id).one(&db).await?;
    let valuations = PropertyValuation::find()
        .filter(entity::property_valuation::Column::PropertyId.eq(p.id))
        .all(&db)
        .await?;
    let taxes = PropertyTax::find()
        .filter(entity::property_tax::Column::PropertyId.eq(p.id))
        .all(&db)
        .await?;
    let leases = Lease::find()
        .filter(entity::lease::Column::TenantId.eq(t))
        .filter(entity::lease::Column::PropertyId.eq(p.id))
        .all(&db)
        .await?;
    let listings = Listing::find()
        .filter(entity::listing::Column::TenantId.eq(t))
        .filter(entity::listing::Column::PropertyId.eq(p.id))
        .all(&db)
        .await?;
    let mortgages = Mortgage::find()
        .filter(entity::mortgage::Column::PropertyId.eq(p.id))
        .all(&db)
        .await?;
    let ownership = Ownership::find()
        .filter(entity::ownership::Column::PropertyId.eq(p.id))
        .all(&db)
        .await?;
    let liens = Lien::find()
        .filter(entity::lien::Column::PropertyId.eq(p.id))
        .all(&db)
        .await?;
    let permits = PropertyPermit::find()
        .filter(entity::property_permit::Column::PropertyId.eq(p.id))
        .all(&db)
        .await?;
    let policies = InsurancePolicy::find()
        .filter(entity::insurance_policy::Column::PropertyId.eq(p.id))
        .all(&db)
        .await?;
    Ok(Json(build(&Sources {
        property: &p,
        detail: detail.as_ref(),
        valuations: &valuations,
        taxes: &taxes,
        leases: &leases,
        listings: &listings,
        mortgages: &mortgages,
        ownership: &ownership,
        liens: &liens,
        permits: &permits,
        policies: &policies,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_a_date_and_drops_the_rest() {
        assert!(is_date("2026-03-05"));
        assert!(is_date("2026-03-05T10:00:00+00:00"));
        assert!(!is_date("sometime in March"));
        assert!(!is_date(""));
    }

    #[test]
    fn events_read_plainly() {
        let e = ev(
            "2026-03-05T10:00:00Z",
            "lease",
            "Lease started",
            None,
            Some(185000),
            true,
        );
        assert_eq!(e.date, "2026-03-05");
        assert_eq!(e.amount_label.as_deref(), Some("$1,850/mo"));
        let s = ev("2021-02-24", "sale", "Sold", None, Some(69853800), false);
        assert_eq!(s.amount_label.as_deref(), Some("$698,538"));
    }
}
