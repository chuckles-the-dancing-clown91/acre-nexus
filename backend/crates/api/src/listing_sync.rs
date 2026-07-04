//! Listing lifecycle synchronization — keeps an advertised listing in step
//! with the pipeline built on top of it, so the website never shows a home
//! that's already spoken for.
//!
//! * Application → lease **conversion** marks the listing `Pending` (still
//!   visible, flagged as under contract).
//! * Lease **activation** (e-signature completion or in-person signing) marks
//!   it `Leased` and unpublishes it.
//! * A **declined** envelope reopens a `Pending` listing (`Available`) — the
//!   deal died from the resident's side, so the home goes back on the market.
//!   (A staff **void** leaves the listing alone: staff are actively managing
//!   the deal and may re-send.)
//!
//! Best-effort like [`crate::rentals_occupancy`]: failures are logged, never
//! propagated — a listing badge must not fail a signing.

use entity::prelude::{Application, Listing};
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// Record the pipeline-driven status change in the audit log (actor = `None`:
/// the pipeline moved it, not a person).
async fn audit_sync(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    listing_id: Uuid,
    from: &str,
    to: &str,
    trigger: &str,
) {
    crate::audit::record(
        db,
        None,
        crate::audit::actions::LISTING_SYNC,
        Some("listing"),
        Some(listing_id.to_string()),
        Some(tenant_id),
        Some(serde_json::json!({ "from": from, "to": to, "trigger": trigger })),
    )
    .await;
}

/// The listing an application points at, if any.
async fn listing_for_application(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    application_id: Uuid,
) -> Option<entity::listing::Model> {
    let app = Application::find_by_id(application_id)
        .filter(entity::application::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .ok()
        .flatten()?;
    Listing::find_by_id(app.listing_id?)
        .filter(entity::listing::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .ok()
        .flatten()
}

/// Conversion: the listing is now under contract (`Pending`), pending signatures.
pub async fn mark_pending_on_convert(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    application_id: Uuid,
) {
    let Some(listing) = listing_for_application(db, tenant_id, application_id).await else {
        return;
    };
    if listing.status == "Leased" {
        return;
    }
    let id = listing.id;
    let from = listing.status.clone();
    let mut am: entity::listing::ActiveModel = listing.into();
    am.status = Set("Pending".into());
    match am.update(db).await {
        Ok(_) => audit_sync(db, tenant_id, id, &from, "Pending", "application_converted").await,
        Err(e) => tracing::warn!("failed to mark listing {id} pending: {e}"),
    }
}

/// Deal death (resident declined to sign): a listing parked at `Pending` by
/// conversion goes back on the market.
pub async fn reopen_on_deal_death(db: &impl ConnectionTrait, tenant_id: Uuid, lease_id: Uuid) {
    let Ok(Some(lease)) = entity::prelude::Lease::find_by_id(lease_id)
        .filter(entity::lease::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
    else {
        return;
    };
    let Some(app_id) = lease.application_id else {
        return;
    };
    let Some(listing) = listing_for_application(db, tenant_id, app_id).await else {
        return;
    };
    if listing.status != "Pending" {
        return;
    }
    let id = listing.id;
    let from = listing.status.clone();
    let mut am: entity::listing::ActiveModel = listing.into();
    am.status = Set("Available".into());
    match am.update(db).await {
        Ok(_) => audit_sync(db, tenant_id, id, &from, "Available", "envelope_declined").await,
        Err(e) => tracing::warn!("failed to reopen listing {id} after declined envelope: {e}"),
    }
}

/// Activation: the lease is signed — the listing is `Leased` and comes off
/// the public site.
pub async fn close_on_lease_activation(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    lease: &entity::lease::Model,
) {
    let Some(app_id) = lease.application_id else {
        return;
    };
    let Some(listing) = listing_for_application(db, tenant_id, app_id).await else {
        return;
    };
    let id = listing.id;
    let from = listing.status.clone();
    let mut am: entity::listing::ActiveModel = listing.into();
    am.status = Set("Leased".into());
    am.is_public = Set(false);
    match am.update(db).await {
        Ok(_) => audit_sync(db, tenant_id, id, &from, "Leased", "lease_activated").await,
        Err(e) => tracing::warn!("failed to close listing {id} on lease activation: {e}"),
    }
}
