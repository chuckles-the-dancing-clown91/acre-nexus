//! **Lease renewals** (issue #44) — the ongoing-tenancy motion that closes the
//! last gap in `FEATURES.md` §2. A renewal proposes new terms (typically a
//! rent increase + extended end date) on an existing lease, generates an
//! addendum document, sends it out through the Phase 2 e-signature substrate,
//! and — when every party signs — applies the new terms to the lease in place.
//!
//! This module owns the pure term math (unit-tested) and the
//! completion-side effect ([`apply_on_signed`], invoked by the e-sign engine
//! when a `purpose = "renewal"` envelope completes). The HTTP surface lives in
//! [`crate::routes::renewals`].

use chrono::{Months, NaiveDate, Utc};
use entity::prelude::LeaseRenewal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde_json::json;
use uuid::Uuid;

/// States in which a renewal is still "in flight" — only one may exist per
/// lease at a time, and completion looks these up. The full lifecycle is
/// `proposed → sent → activated`, with `declined`/`cancelled` off-ramps (see
/// [`entity::lease_renewal`]).
pub const OPEN_STATUSES: &[&str] = &["proposed", "sent", "signed"];

// ---------------------------------------------------------------------------
// Pure term helpers (unit-tested)
// ---------------------------------------------------------------------------

/// Add `months` calendar months to a `YYYY-MM-DD` date, clamping to the end of
/// the target month (e.g. Jan 31 + 1mo = Feb 28). `None` if it doesn't parse.
pub fn add_months_str(date: &str, months: u32) -> Option<String> {
    let d = NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?;
    let out = d.checked_add_months(Months::new(months))?;
    Some(out.format("%Y-%m-%d").to_string())
}

/// The day after `date` (`YYYY-MM-DD`) — the natural start of a renewed term
/// following the current lease's end. `None` if it doesn't parse.
pub fn day_after(date: &str) -> Option<String> {
    let d = NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?;
    Some(d.succ_opt()?.format("%Y-%m-%d").to_string())
}

/// Whether `end` (`YYYY-MM-DD`) is strictly after `start` — both must parse.
pub fn end_after_start(start: &str, end: &str) -> bool {
    match (
        NaiveDate::parse_from_str(start, "%Y-%m-%d"),
        NaiveDate::parse_from_str(end, "%Y-%m-%d"),
    ) {
        (Ok(s), Ok(e)) => e > s,
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Completion side-effect
// ---------------------------------------------------------------------------

/// Apply a fully-signed renewal to its lease: bump the rent, extend the end
/// date, and mark the renewal `activated`. Called by
/// [`crate::esign::complete_envelope`] when a `purpose = "renewal"` envelope
/// completes; the addendum document uniquely identifies the renewal.
pub async fn apply_on_signed(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    lease: entity::lease::Model,
    envelope: &entity::esign_envelope::Model,
) -> anyhow::Result<()> {
    let renewal = LeaseRenewal::find()
        .filter(entity::lease_renewal::Column::TenantId.eq(tenant_id))
        .filter(entity::lease_renewal::Column::LeaseDocumentId.eq(envelope.lease_document_id))
        .filter(entity::lease_renewal::Column::Status.is_in(OPEN_STATUSES.to_vec()))
        .order_by_desc(entity::lease_renewal::Column::CreatedAt)
        .one(db)
        .await?;
    let Some(renewal) = renewal else {
        anyhow::bail!(
            "no open renewal for envelope {} (document {})",
            envelope.id,
            envelope.lease_document_id
        );
    };

    let now = Utc::now();
    let lease_id = lease.id;
    let property_id = lease.property_id;
    let new_rent = renewal.new_rent_cents;
    let new_end = renewal.new_end_date.clone();

    // Apply the renewed terms to the lease in place. A signed renewal means the
    // tenancy continues, so any "notice" standing clears back to active.
    let mut lm: entity::lease::ActiveModel = lease.into();
    lm.rent_cents = Set(new_rent);
    lm.end_date = Set(new_end);
    lm.status = Set("active".into());
    lm.updated_at = Set(now.into());
    lm.update(db).await?;

    let renewal_id = renewal.id;
    let mut rm: entity::lease_renewal::ActiveModel = renewal.into();
    rm.status = Set("activated".into());
    rm.activated_at = Set(Some(now.into()));
    rm.updated_at = Set(now.into());
    rm.update(db).await?;

    // Keep property occupancy + unit status coherent with the refreshed lease
    // (the calendar scan will also re-date the lease-renewal reminder to the
    // new end date on its next pass).
    crate::rentals_occupancy::sync_property_occupancy(db, property_id).await;

    crate::audit::record(
        db,
        None,
        crate::audit::actions::LEASE_RENEWAL_ACTIVATE,
        Some("lease_renewal"),
        Some(renewal_id.to_string()),
        Some(tenant_id),
        Some(json!({
            "lease_id": lease_id,
            "new_rent_cents": new_rent,
        })),
    )
    .await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_months_clamps_month_end() {
        assert_eq!(
            add_months_str("2026-01-31", 1).as_deref(),
            Some("2026-02-28")
        );
        assert_eq!(
            add_months_str("2026-07-01", 12).as_deref(),
            Some("2027-07-01")
        );
        assert_eq!(
            add_months_str("2026-07-01", 6).as_deref(),
            Some("2027-01-01")
        );
        assert_eq!(add_months_str("not-a-date", 12), None);
    }

    #[test]
    fn day_after_rolls_over() {
        assert_eq!(day_after("2026-12-31").as_deref(), Some("2027-01-01"));
        assert_eq!(day_after("2026-07-11").as_deref(), Some("2026-07-12"));
        assert_eq!(day_after("bad"), None);
    }

    #[test]
    fn end_after_start_compares() {
        assert!(end_after_start("2026-07-01", "2027-07-01"));
        assert!(!end_after_start("2027-07-01", "2026-07-01"));
        assert!(!end_after_start("2026-07-01", "2026-07-01"));
        assert!(!end_after_start("2026-07-01", "bad"));
    }
}
