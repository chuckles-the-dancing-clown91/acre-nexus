//! **Leasing** module — the public white-label website (listings + the apply
//! funnel), the renter-portal and back-office application intake doors, the
//! back-office applications inbox, and console listing management. It owns the
//! tenant-screening background jobs; the `auto_email` jobs it enqueues are
//! owned by the `integrations` module (which renders and delivers them).

use super::{JobContext, JobOutcome, ModuleManifest, PlatformModule};
use crate::rbac::Permission;
use crate::routes::{applications, listings, public};
use rocket::Route;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use uuid::Uuid;

pub struct LeasingModule;

#[rocket::async_trait]
impl PlatformModule for LeasingModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "leasing",
            name: "Leasing & Listings",
            description: "Public listings website, listing management, applications \
                          (website, renter portal, back office), and tenant screening.",
            permissions: &[
                Permission::ListingRead,
                Permission::ListingWrite,
                Permission::ApplicationRead,
                Permission::ApplicationWrite,
            ],
            job_kinds: &["background_check", "screening"],
            default_enabled: true,
            preview: false,
        }
    }

    fn api(&self) -> (Vec<Route>, OpenApi) {
        openapi_get_routes_spec![
            public::listings::listings,
            public::listing_detail::listing_detail,
            public::public_theme::public_theme,
            public::apply::apply,
            // console listing management
            listings::list::list,
            listings::create::create,
            listings::update::update,
            // applications: back-office inbox + intake
            applications::list::list,
            applications::create::create,
            applications::update_status::update_status,
            // renter portal: apply + track as the signed-in user
            applications::portal::my_applications,
            applications::portal::my_apply,
            // application workflow (pipeline + history + advance)
            applications::workflow::catalog,
            applications::workflow::get_workflow,
            applications::workflow::advance,
            // application reuse (recent application → any property), gated by setting
            applications::reuse::reusable,
            applications::reuse::reuse,
        ]
    }

    /// Durable screening state machine: pending → awaiting the (simulated)
    /// provider callback → completed. Completion writes the outcome onto the
    /// application and either auto-approves it (workspace setting) or asks
    /// staff to review.
    async fn handle_job(&self, ctx: &JobContext<'_>) -> Option<JobOutcome> {
        let now = chrono::Utc::now();
        match (ctx.job.kind.as_str(), ctx.job.status.as_str()) {
            ("background_check" | "screening", "pending") => {
                Some(JobOutcome::reschedule("awaiting_callback", 6))
            }
            ("background_check" | "screening", "awaiting_callback") => {
                // The simulated provider always clears; a real FCRA provider
                // (roadmap Phase 4) drops its verdict in here instead.
                let result = "cleared";
                if let Err(e) = record_screening_outcome(ctx, result).await {
                    tracing::error!("failed to apply screening outcome: {e}");
                }
                Some(JobOutcome::completed(json!({
                    "cleared": true,
                    "credit_band": "good",
                    "eviction_records": 0,
                    "result": result,
                    "completed_at": now.to_rfc3339(),
                })))
            }
            _ => None,
        }
    }
}

/// Land a finished screening on its application: record the outcome, then
/// auto-approve (when the workspace setting is on and the check cleared) or
/// notify staff that a decision is waiting.
async fn record_screening_outcome(ctx: &JobContext<'_>, result: &str) -> anyhow::Result<()> {
    use sea_orm::{ActiveModelTrait, Set};

    let Some(app_id) = ctx
        .job
        .payload
        .get("application_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    else {
        // Legacy/manual jobs without an application reference: nothing to land.
        return Ok(());
    };
    let tenant_id = ctx.job.tenant_id;
    let Some(app) = entity::prelude::Application::find_by_id(app_id)
        .filter(entity::application::Column::TenantId.eq(tenant_id))
        .one(ctx.db)
        .await?
    else {
        return Ok(());
    };

    // Record the outcome (idempotent: a retried job re-writes the same state).
    let mut am: entity::application::ActiveModel = app.clone().into();
    am.screening_status = Set(Some(result.to_string()));
    am.screened_at = Set(Some(chrono::Utc::now().into()));
    let app = am.update(ctx.db).await?;

    // The application may have been decided while screening ran.
    if app.status != "Screening" {
        return Ok(());
    }

    let auto_approve =
        crate::settings::get_bool(ctx.db, tenant_id, crate::settings::APPLICATION_AUTO_APPROVE)
            .await;

    if auto_approve && result == "cleared" {
        crate::routes::applications::apply_transition(
            ctx.db,
            tenant_id,
            None,
            app,
            "Approved",
            Some("Auto-approved: screening cleared".into()),
        )
        .await
        .map_err(|e| anyhow::anyhow!("auto-approve transition failed: {e}"))?;
    } else {
        crate::notify::notify_staff(
            ctx.db,
            tenant_id,
            "application:read",
            "application_screened",
            json!({ "applicant": app.applicant_name, "result": result }),
            Some(("application", app.id)),
            "screened",
            None,
        )
        .await;
    }
    Ok(())
}
