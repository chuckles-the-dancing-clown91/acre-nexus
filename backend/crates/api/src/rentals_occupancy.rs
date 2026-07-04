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

/// The one rule both signing paths (e-signature completion and in-person)
/// share: signing **activates the tenancy** — the lease flips to `active`,
/// occupancy re-syncs, and the advertised listing (if the lease came from one)
/// closes out. Returns the up-to-date lease.
pub async fn activate_lease_on_signing(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    lease: entity::lease::Model,
) -> Result<entity::lease::Model, sea_orm::DbErr> {
    let property_id = lease.property_id;
    let lease = if lease.status != "active" {
        let from = lease.status.clone();
        let mut lm: entity::lease::ActiveModel = lease.into();
        lm.status = Set("active".into());
        lm.updated_at = Set(chrono::Utc::now().into());
        let lease = lm.update(db).await?;
        // The activation is its own domain event (actor = None: signing did
        // it), on top of the esign.complete / lease_document.sign umbrella.
        crate::audit::record(
            db,
            None,
            crate::audit::actions::LEASE_ACTIVATE,
            Some("lease"),
            Some(lease.id.to_string()),
            Some(tenant_id),
            Some(serde_json::json!({ "from": from, "trigger": "document_signed" })),
        )
        .await;
        lease
    } else {
        lease
    };
    sync_property_occupancy(db, property_id).await;
    crate::listing_sync::close_on_lease_activation(db, tenant_id, &lease).await;
    Ok(lease)
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
    let mut units_flipped = 0usize;
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
            units_flipped += 1;
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
            let tenant_id = p.tenant_id;
            let from_status = p.status.clone();
            let from_occupied = p.occupied_units;
            let mut am: entity::property::ActiveModel = p.into();
            am.occupied_units = Set(occupied);
            if let Some(status) = next_status {
                am.status = Set(status.into());
            }
            am.update(db).await?;
            // Occupancy changing hands is a domain event — one record per
            // reconciliation that actually changed something, never per tick.
            crate::audit::record(
                db,
                None,
                crate::audit::actions::PROPERTY_UPDATE,
                Some("property"),
                Some(property_id.to_string()),
                Some(tenant_id),
                Some(serde_json::json!({
                    "occupied_units": { "from": from_occupied, "to": occupied },
                    "status": next_status.map(|s| serde_json::json!({ "from": from_status, "to": s })),
                    "units_flipped": units_flipped,
                    "trigger": "occupancy_sync",
                })),
            )
            .await;
        }
    }
    Ok(())
}
