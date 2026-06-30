//! **Lease charge** endpoints — the resolved line items on a lease, plus the
//! `apply-fees` action that evaluates the tenant's [`crate::routes::fees`] schedule
//! against the lease's attributes (pets, military, vehicles) and auto-populates the
//! matching fees, discounts, and amenities.

pub mod add;
pub mod apply_fees;
pub mod delete;
pub mod dto;
pub mod list;

/// Signed amount for a charge given its kind: discounts/rebates reduce the total.
pub fn signed_amount(kind: &str, amount_cents: i64) -> i64 {
    let a = amount_cents.abs();
    match kind {
        "discount" | "rebate" => -a,
        _ => a,
    }
}
