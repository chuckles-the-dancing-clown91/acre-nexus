//! Shared computation for the property profile: the operating/levered economics
//! and the header blocks (home breakdown, address status, rental status).
//!
//! The economics mirror the design prototype: maintenance ≈ 9% of rent, taxes &
//! insurance ≈ 12%, management fee 8%; net = rent − those. Levered figures fold
//! in mortgage debt service and best-known value.

use super::dto::{ActiveLeaseSummary, AddressStatus, HomeBreakdown, RentalStatus};
use crate::dto::usd;
use crate::error::ApiResult;
use entity::prelude::{Lease, Mortgage, PropertyValuation};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

/// The computed operating + levered economics for a property.
pub struct Economics {
    pub maintenance_cents: i64,
    pub tax_cents: i64,
    pub mgmt_cents: i64,
    pub net_revenue_cents: i64,
    /// Whether any non-`paid_off` mortgage exists.
    pub financed: bool,
    pub debt_service_cents: i64,
    pub total_loan_balance_cents: i64,
    pub cash_flow_cents: i64,
    pub equity_cents: i64,
}

/// Compute economics from the rent roll, the active mortgages, and best-known
/// value. Pure so it is unit-testable; callers fetch the inputs.
pub fn economics(
    rent_cents: i64,
    mortgages: &[entity::mortgage::Model],
    best_known_value_cents: i64,
) -> Economics {
    let maintenance_cents = (rent_cents as f64 * 0.09).round() as i64;
    let tax_cents = (rent_cents as f64 * 0.12).round() as i64;
    let mgmt_cents = (rent_cents as f64 * 0.08).round() as i64;
    let net_revenue_cents = rent_cents - maintenance_cents - tax_cents - mgmt_cents;

    let active: Vec<_> = mortgages
        .iter()
        .filter(|m| m.status != "paid_off")
        .collect();
    let debt_service_cents: i64 = active
        .iter()
        .map(|m| m.monthly_payment_cents.unwrap_or(0) + m.escrow_monthly_cents.unwrap_or(0))
        .sum();
    let total_loan_balance_cents: i64 = active
        .iter()
        .map(|m| m.current_balance_cents.unwrap_or(0))
        .sum();
    let financed = !active.is_empty();
    let cash_flow_cents = net_revenue_cents - debt_service_cents;
    let equity_cents = best_known_value_cents - total_loan_balance_cents;

    Economics {
        maintenance_cents,
        tax_cents,
        mgmt_cents,
        net_revenue_cents,
        financed,
        debt_service_cents,
        total_loan_balance_cents,
        cash_flow_cents,
        equity_cents,
    }
}

/// All mortgages on a property, ordered by lien position.
pub async fn mortgages_for(
    db: &impl sea_orm::ConnectionTrait,
    pid: Uuid,
) -> ApiResult<Vec<entity::mortgage::Model>> {
    Ok(Mortgage::find()
        .filter(entity::mortgage::Column::PropertyId.eq(pid))
        .order_by_asc(entity::mortgage::Column::Position)
        .all(db)
        .await?)
}

/// Best-known value for equity: latest AVM estimate, else purchase price, else 0.
pub async fn best_known_value(
    db: &impl sea_orm::ConnectionTrait,
    pid: Uuid,
    purchase_price_cents: Option<i64>,
) -> ApiResult<i64> {
    Ok(PropertyValuation::find()
        .filter(entity::property_valuation::Column::PropertyId.eq(pid))
        .order_by_desc(entity::property_valuation::Column::CreatedAt)
        .one(db)
        .await?
        .and_then(|v| v.estimated_value_cents)
        .or(purchase_price_cents)
        .unwrap_or(0))
}

/// The physical breakdown, merging enriched `property_detail` over the property.
pub fn home_breakdown(
    property: &entity::property::Model,
    detail: Option<&entity::property_detail::Model>,
) -> HomeBreakdown {
    let property_type = detail
        .and_then(|d| d.property_type.clone())
        .filter(|s| !s.is_empty())
        .or_else(|| Some(property.property_type.clone()).filter(|s| !s.is_empty()));
    HomeBreakdown {
        beds: detail.and_then(|d| d.beds),
        baths: detail.and_then(|d| d.baths),
        sqft: detail.and_then(|d| d.sqft),
        lot_size_sqft: detail.and_then(|d| d.lot_size_sqft),
        stories: detail.and_then(|d| d.stories),
        parking_spaces: detail.and_then(|d| d.parking_spaces),
        heating: detail.and_then(|d| d.heating.clone()),
        cooling: detail.and_then(|d| d.cooling.clone()),
        year_built: (property.year_built > 0).then_some(property.year_built),
        property_type,
    }
}

/// The address block + geocode/verification status.
pub fn address_status(
    property: &entity::property::Model,
    detail: Option<&entity::property_detail::Model>,
) -> AddressStatus {
    let latitude = detail.and_then(|d| d.latitude);
    let longitude = detail.and_then(|d| d.longitude);
    AddressStatus {
        address: property.address.clone(),
        city: property.city.clone(),
        matched_address: detail.and_then(|d| d.matched_address.clone()),
        geocode_accuracy: detail.and_then(|d| d.geocode_accuracy.clone()),
        verified: latitude.is_some() && longitude.is_some(),
        latitude,
        longitude,
        county: detail.and_then(|d| d.county.clone()),
        apn: detail.and_then(|d| d.apn.clone()),
    }
}

/// A resident is behind when their payment standing is `late` or `partial`.
fn is_delinquent(payment_status: &str) -> bool {
    matches!(payment_status, "late" | "partial")
}

/// The rental picture: occupancy plus current tenancies and their standing.
/// Current = leases in `active` or `notice` status.
pub async fn rental_status(
    db: &impl sea_orm::ConnectionTrait,
    property: &entity::property::Model,
) -> ApiResult<RentalStatus> {
    let leases = Lease::find()
        .filter(entity::lease::Column::PropertyId.eq(property.id))
        .filter(entity::lease::Column::Status.is_in(["active", "notice"]))
        .order_by_asc(entity::lease::Column::StartDate)
        .all(db)
        .await?;

    let delinquent_leases = leases
        .iter()
        .filter(|l| is_delinquent(&l.payment_status))
        .count() as i32;

    let active_leases = leases
        .into_iter()
        .map(|l| ActiveLeaseSummary {
            rent_label: usd(l.rent_cents),
            balance_label: usd(l.balance_cents),
            lease_id: l.id,
            unit_id: l.unit_id,
            tenant_name: l.tenant_name,
            rent_cents: l.rent_cents,
            status: l.status,
            payment_status: l.payment_status,
            balance_cents: l.balance_cents,
        })
        .collect();

    Ok(RentalStatus {
        status: property.status.clone(),
        occupancy: format!("{}/{}", property.occupied_units, property.units),
        units: property.units,
        occupied_units: property.occupied_units,
        vacant_units: (property.units - property.occupied_units).max(0),
        monthly_rent_cents: property.monthly_rent_cents,
        monthly_rent_label: usd(property.monthly_rent_cents),
        delinquent_leases,
        active_leases,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mortgage(status: &str, payment: i64, escrow: i64, balance: i64) -> entity::mortgage::Model {
        entity::mortgage::Model {
            id: Uuid::nil(),
            tenant_id: Uuid::nil(),
            property_id: Uuid::nil(),
            lender_id: None,
            kind: "purchase".into(),
            position: 1,
            original_amount_cents: None,
            current_balance_cents: Some(balance),
            interest_rate_bps: None,
            term_months: None,
            monthly_payment_cents: Some(payment),
            escrow_monthly_cents: Some(escrow),
            start_date: None,
            maturity_date: None,
            loan_number: None,
            status: status.into(),
            notes: None,
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        }
    }

    #[test]
    fn unfinanced_economics() {
        let e = economics(100_000, &[], 0);
        assert_eq!(e.maintenance_cents, 9_000);
        assert_eq!(e.tax_cents, 12_000);
        assert_eq!(e.mgmt_cents, 8_000);
        assert_eq!(e.net_revenue_cents, 71_000);
        assert!(!e.financed);
        assert_eq!(e.debt_service_cents, 0);
        assert_eq!(e.cash_flow_cents, 71_000);
    }

    #[test]
    fn levered_economics_folds_in_debt_and_equity() {
        let ms = [
            mortgage("active", 40_000, 10_000, 8_000_000),
            mortgage("paid_off", 99_999, 99_999, 99_999), // excluded
        ];
        let e = economics(100_000, &ms, 12_000_000);
        assert!(e.financed);
        assert_eq!(e.debt_service_cents, 50_000);
        assert_eq!(e.total_loan_balance_cents, 8_000_000);
        assert_eq!(e.cash_flow_cents, 71_000 - 50_000);
        assert_eq!(e.equity_cents, 12_000_000 - 8_000_000);
    }
}
