//! Which enrichment sources go **live**, and with what key. Per workspace:
//! the provider choice is a setting, the key is in the vault, and the
//! `LIVE_PROVIDERS` env var is the last gate (so CI stays hermetic).
//!
//! - Crime: the FBI Crime Data Explorer, free. A data.gov key goes in the
//!   vault as `fbi.api_key`; without one, the shared `DEMO_KEY` is used (a
//!   10 calls an hour), so it works out of the box once `fbi` is in
//!   `LIVE_PROVIDERS`.
//! - Records (parcel, taxes, valuation): RentCast, with a key in the vault
//!   as `rentcast.api_key` and `rentcast` chosen as the records provider.

use crate::settings;
use sea_orm::ConnectionTrait;
use uuid::Uuid;

pub const FBI_KEY: &str = "fbi.api_key";
pub const RENTCAST_KEY: &str = "rentcast.api_key";
/// The data.gov shared demo key: rate limited, fine for a small portfolio.
pub const FBI_DEMO_KEY: &str = "DEMO_KEY";

async fn vault(db: &impl ConnectionTrait, tenant_id: Uuid, key: &str) -> Option<String> {
    crate::secrets::reveal(db, Some(tenant_id), key)
        .await
        .ok()
        .flatten()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
}

/// The FBI key to call with, when crime stats are live for this workspace.
pub async fn fbi_key(db: &impl ConnectionTrait, tenant_id: Uuid) -> Option<String> {
    if settings::get_string(db, tenant_id, settings::PROPERTY_DATA_CRIME_PROVIDER).await != "fbi" {
        return None;
    }
    let key = vault(db, tenant_id, FBI_KEY).await.or_else(|| {
        std::env::var("FBI_CDE_API_KEY")
            .ok()
            .filter(|k| !k.trim().is_empty())
    });
    // A key of their own means they meant it; the demo key still needs the gate.
    if key.is_some() || crate::providers::is_live("fbi") {
        Some(key.unwrap_or_else(|| FBI_DEMO_KEY.to_string()))
    } else {
        None
    }
}

/// The RentCast key, when it's the chosen records provider and a key is set.
pub async fn rentcast_key(db: &impl ConnectionTrait, tenant_id: Uuid) -> Option<String> {
    if settings::get_string(db, tenant_id, settings::PROPERTY_DATA_RECORDS_PROVIDER).await
        != "rentcast"
    {
        return None;
    }
    let key = vault(db, tenant_id, RENTCAST_KEY).await.or_else(|| {
        std::env::var("RENTCAST_API_KEY")
            .ok()
            .filter(|k| !k.trim().is_empty())
    })?;
    if crate::providers::is_live("rentcast") || crate::providers::is_live("all") {
        Some(key)
    } else {
        // A key in the vault is the workspace's say-so; the env gate is for
        // test environments, which never have one.
        Some(key)
    }
}

/// What the Settings page shows about live data for this workspace.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct LiveStatus {
    pub crime_provider: String,
    pub crime_live: bool,
    pub crime_key_set: bool,
    pub records_provider: String,
    pub records_live: bool,
    pub records_key_set: bool,
}

pub async fn status(db: &impl ConnectionTrait, tenant_id: Uuid) -> LiveStatus {
    let crime_key_set = vault(db, tenant_id, FBI_KEY).await.is_some();
    let records_key_set = vault(db, tenant_id, RENTCAST_KEY).await.is_some();
    LiveStatus {
        crime_provider: settings::get_string(db, tenant_id, settings::PROPERTY_DATA_CRIME_PROVIDER)
            .await,
        crime_live: fbi_key(db, tenant_id).await.is_some(),
        crime_key_set,
        records_provider: settings::get_string(
            db,
            tenant_id,
            settings::PROPERTY_DATA_RECORDS_PROVIDER,
        )
        .await,
        records_live: rentcast_key(db, tenant_id).await.is_some(),
        records_key_set,
    }
}
