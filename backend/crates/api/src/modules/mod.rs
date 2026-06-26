//! # Pluggable platform modules
//!
//! The platform is assembled from self-contained **modules**. Each module
//! bundles everything that makes a feature area pluggable:
//!
//! * a [`ModuleManifest`] (stable key, human metadata, the permissions it owns,
//!   the background-job kinds it handles, and whether it is on by default),
//! * the Rocket [`routes`](PlatformModule::routes) it contributes, and
//! * an optional [`handle_job`](PlatformModule::handle_job) state-machine step
//!   for the Tokio scheduler.
//!
//! Modules are listed once in [`registry`]. From there:
//! * [`crate::main`] mounts every module's routes at boot,
//! * [`crate::scheduler`] dispatches each due background job to the owning
//!   module, and
//! * the `/modules` routes let a tenant enable/disable modules, gated per-tenant
//!   by the `tenant_module` table (see [`is_enabled`] / [`require_enabled`]).
//!
//! ## Adding a module
//!
//! 1. Create `modules/<name>.rs` with a unit struct implementing
//!    [`PlatformModule`].
//! 2. Add one line to [`registry`].
//!
//! That is the entire contract — no central wiring to touch. See
//! [`flips`] for a minimal, self-gating example.

pub mod flips;
pub mod leasing;
pub mod properties;
pub mod theming;
pub mod vendor_api;

use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use chrono::{DateTime, Utc};
use entity::background_job;
use entity::prelude::TenantModule;
use rocket::Route;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

/// Static description of a module, surfaced to operators and the settings UI.
#[derive(Clone, Debug)]
pub struct ModuleManifest {
    /// Stable, URL-safe key (e.g. `properties`). Used as the `tenant_module`
    /// discriminator and shared verbatim with the frontend module registry.
    pub key: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// Permissions this module introduces / requires.
    pub permissions: &'static [Permission],
    /// Background-job `kind`s this module's [`PlatformModule::handle_job`] owns.
    pub job_kinds: &'static [&'static str],
    /// Whether the module is enabled for a tenant that has no explicit override.
    pub default_enabled: bool,
    /// A preview module is shipped but not yet generally available; the UI marks
    /// it accordingly and it defaults to off.
    pub preview: bool,
}

/// Context handed to a module when the scheduler asks it to advance a job.
pub struct JobContext<'a> {
    pub db: &'a DatabaseConnection,
    pub job: &'a background_job::Model,
}

/// The result of advancing a background job by one step.
pub struct JobOutcome {
    /// New `status` to persist (e.g. `awaiting_callback`, `completed`).
    pub status: String,
    /// If set, reschedule the job to run again no earlier than this instant.
    pub run_at: Option<DateTime<Utc>>,
    /// Optional result/detail payload to persist.
    pub result: Option<serde_json::Value>,
}

impl JobOutcome {
    /// Terminal success with a result payload.
    pub fn completed(result: serde_json::Value) -> Self {
        JobOutcome { status: "completed".into(), run_at: None, result: Some(result) }
    }

    /// Move to `status` and try again after `delay_secs`.
    pub fn reschedule(status: impl Into<String>, delay_secs: i64) -> Self {
        JobOutcome {
            status: status.into(),
            run_at: Some(Utc::now() + chrono::Duration::seconds(delay_secs)),
            result: None,
        }
    }
}

/// The contract every pluggable module implements.
#[rocket::async_trait]
pub trait PlatformModule: Send + Sync {
    /// Static metadata describing the module.
    fn manifest(&self) -> ModuleManifest;

    /// Rocket routes contributed by this module. Mounted at the API root.
    fn routes(&self) -> Vec<Route> {
        vec![]
    }

    /// Advance one background job that this module owns (matched by
    /// `manifest().job_kinds`). Return `None` to fall through to the default
    /// "mark completed" behaviour.
    async fn handle_job(&self, _ctx: &JobContext<'_>) -> Option<JobOutcome> {
        None
    }
}

/// The single source of truth for which modules exist. Order is the mount order.
pub fn registry() -> Vec<Box<dyn PlatformModule>> {
    vec![
        Box::new(properties::PropertiesModule),
        Box::new(leasing::LeasingModule),
        Box::new(vendor_api::VendorApiModule),
        Box::new(theming::ThemingModule),
        Box::new(flips::FlipsModule),
    ]
}

/// The module that owns a given background-job kind, if any.
pub fn module_for_job_kind(kind: &str) -> Option<Box<dyn PlatformModule>> {
    registry()
        .into_iter()
        .find(|m| m.manifest().job_kinds.contains(&kind))
}

/// Whether `module_key` is enabled for `tenant_id`. Falls back to the module's
/// `default_enabled` when the tenant has no explicit override row.
pub async fn is_enabled(db: &DatabaseConnection, tenant_id: Uuid, module_key: &str) -> bool {
    let default = registry()
        .iter()
        .find(|m| m.manifest().key == module_key)
        .map(|m| m.manifest().default_enabled)
        .unwrap_or(false);

    match TenantModule::find()
        .filter(entity::tenant_module::Column::TenantId.eq(tenant_id))
        .filter(entity::tenant_module::Column::ModuleKey.eq(module_key))
        .one(db)
        .await
    {
        Ok(Some(row)) => row.enabled,
        Ok(None) => default,
        Err(e) => {
            tracing::error!("tenant_module lookup failed: {e}");
            default
        }
    }
}

/// Guard helper for routes belonging to an optional module: returns
/// `403 module_disabled` when the module is off for the active tenant.
pub async fn require_enabled(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    module_key: &str,
) -> ApiResult<()> {
    if is_enabled(db, tenant_id, module_key).await {
        Ok(())
    } else {
        Err(ApiError::Forbidden(format!(
            "module '{module_key}' is not enabled for this tenant"
        )))
    }
}
