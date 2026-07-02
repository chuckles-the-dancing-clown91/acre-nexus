//! Listing lifecycle synchronization — keeps an advertised listing in step
//! with the pipeline built on top of it, so the website never shows a home
//! that's already spoken for.
//!
//! * Application → lease **conversion** marks the listing `Pending` (still
//!   visible, flagged as under contract).
//! * Lease **activation** (e-signature completion or in-person signing) marks
//!   it `Leased` and unpublishes it.
//!
//! Best-effort like [`crate::rentals_occupancy`]: failures are logged, never
//! propagated — a listing badge must not fail a signing.

use entity::prelude::{Application, Listing};
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

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
    let mut am: entity::listing::ActiveModel = listing.into();
    am.status = Set("Pending".into());
    if let Err(e) = am.update(db).await {
        tracing::warn!("failed to mark listing {id} pending: {e}");
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
    let mut am: entity::listing::ActiveModel = listing.into();
    am.status = Set("Leased".into());
    am.is_public = Set(false);
    if let Err(e) = am.update(db).await {
        tracing::warn!("failed to close listing {id} on lease activation: {e}");
    }
}
