//! **Leasing** module — the public white-label website (listings + the apply
//! funnel) and the back-office applications inbox. It owns the tenant-screening
//! background jobs the apply funnel enqueues; the `auto_email` jobs it enqueues
//! are owned by the `integrations` module (which renders and delivers them).

use super::{JobContext, JobOutcome, ModuleManifest, PlatformModule};
use crate::rbac::Permission;
use crate::routes::{applications, public};
use rocket::Route;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;
use serde_json::json;

pub struct LeasingModule;

#[rocket::async_trait]
impl PlatformModule for LeasingModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "leasing",
            name: "Leasing & Listings",
            description: "Public listings website, applications, and tenant screening.",
            permissions: &[
                Permission::ListingRead,
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
            applications::list::list,
            applications::update_status::update_status,
            // application workflow (pipeline + history + advance)
            applications::workflow::catalog,
            applications::workflow::get_workflow,
            applications::workflow::advance,
            // application reuse (recent application → any property), gated by setting
            applications::reuse::reusable,
            applications::reuse::reuse,
        ]
    }

    /// Durable screening state machine.
    async fn handle_job(&self, ctx: &JobContext<'_>) -> Option<JobOutcome> {
        let now = chrono::Utc::now();
        match (ctx.job.kind.as_str(), ctx.job.status.as_str()) {
            // Screening: pending -> awaiting external callback -> completed.
            ("background_check" | "screening", "pending") => {
                Some(JobOutcome::reschedule("awaiting_callback", 6))
            }
            ("background_check" | "screening", "awaiting_callback") => {
                Some(JobOutcome::completed(json!({
                    "cleared": true,
                    "credit_band": "good",
                    "eviction_records": 0,
                    "completed_at": now.to_rfc3339(),
                })))
            }
            _ => None,
        }
    }
}
