//! **Leasing** module — the public white-label website (listings + the apply
//! funnel), the renter-portal and back-office application intake doors, the
//! back-office applications inbox, and console listing management. It owns the
//! tenant-screening background jobs (delegated to [`crate::screening`], the
//! Phase 4 FCRA pipeline); the `auto_email` jobs it enqueues are owned by the
//! `integrations` module (which renders and delivers them).

use super::{JobContext, JobOutcome, ModuleManifest, PlatformModule};
use crate::rbac::Permission;
use crate::routes::{applications, listings, public};
use rocket::Route;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;

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
                Permission::ScreeningRead,
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
            // screening report + adverse action (FCRA)
            applications::screening::get_screening,
            applications::screening::adverse_action,
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

    /// Durable screening state machine — the Phase 4 pipeline in
    /// [`crate::screening`]: order a real report through the provider
    /// framework, wait for the (simulated or webhook-delivered) result,
    /// evaluate the workspace's screening policy, and land the verdict on the
    /// application (auto-approve or staff review).
    async fn handle_job(&self, ctx: &JobContext<'_>) -> Option<JobOutcome> {
        match ctx.job.kind.as_str() {
            "background_check" | "screening" => {
                Some(crate::screening::handle_job(ctx.db, ctx.job).await)
            }
            _ => None,
        }
    }
}
