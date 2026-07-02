//! Occupancy synchronization — keeps a property's `occupied_units` counter and its
//! units' `status` in step with active leases, so the property view reflects who is
//! actually housed instead of a hand-maintained number.
//!
//! Called (best-effort) whenever a lease is created, converted, or has its status
//! changed. A unit is marked `occupied` when an active lease references it and
//! reverted to `vacant` when none does (leaving `make_ready`/`down` untouched).

use entity::prelude::{Lease, Property, Unit};
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use std::collections::HashSet;
use uuid::Uuid;

/// Recompute occupancy for one property. Best-effort: errors are logged, not
/// propagated, so the underlying lease mutation never fails on a sync hiccup.
pub async fn sync_property_occupancy(db: &impl ConnectionTrait, property_id: Uuid) {
    if let Err(e) = try_sync(db, property_id).await {
        tracing::warn!("occupancy sync for {property_id} failed: {e}");
    }
}

async fn try_sync(db: &impl ConnectionTrait, property_id: Uuid) -> Result<(), sea_orm::DbErr> {
    let leases = Lease::find()
        .filter(entity::lease::Column::PropertyId.eq(property_id))
        .all(db)
        .await?;
    let active: Vec<&entity::lease::Model> =
        leases.iter().filter(|l| l.status == "active").collect();
    let occupied_unit_ids: HashSet<Uuid> = active.iter().filter_map(|l| l.unit_id).collect();

    // Reconcile each unit's status with whether an active lease references it.
    let units = Unit::find()
        .filter(entity::unit::Column::PropertyId.eq(property_id))
        .all(db)
        .await?;
    for u in units {
        let should_be_occupied = occupied_unit_ids.contains(&u.id);
        let next = if should_be_occupied {
            "occupied"
        } else if u.status == "occupied" {
            // Was occupied, no active lease now → vacant. Leave make_ready/down as-is.
            "vacant"
        } else {
            continue;
        };
        if u.status != next {
            let mut am: entity::unit::ActiveModel = u.into();
            am.status = Set(next.into());
            am.update(db).await?;
        }
    }

    // Property occupied_units = distinct occupied units + active whole-property
    // leases that carry no unit (single-family). Counting raw active leases would
    // double-count renewals on the same unit and inflate the occupancy ratio.
    if let Some(p) = Property::find_by_id(property_id).one(db).await? {
        let no_unit_active = active.iter().filter(|l| l.unit_id.is_none()).count();
        let mut occupied = (occupied_unit_ids.len() + no_unit_active) as i32;
        if p.units > 0 && occupied > p.units {
            occupied = p.units;
        }
        // The availability-facing status flips with real occupancy so a leased
        // home stops presenting as available: Vacant ↔ Stabilized only —
        // operational statuses (rehab, down, …) stay staff-owned and untouched.
        let next_status = if occupied > 0 && p.status == "Vacant" {
            Some("Stabilized")
        } else if occupied == 0 && p.status == "Stabilized" {
            Some("Vacant")
        } else {
            None
        };
        if p.occupied_units != occupied || next_status.is_some() {
            let mut am: entity::property::ActiveModel = p.into();
            am.occupied_units = Set(occupied);
            if let Some(status) = next_status {
                am.status = Set(status.into());
            }
            am.update(db).await?;
        }
    }
    Ok(())
}
