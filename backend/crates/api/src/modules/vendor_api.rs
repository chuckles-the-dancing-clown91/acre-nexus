//! **Vendor API** module — the token-authenticated `/api/v1` surface sold to
//! third-party integrators, plus the tenant-facing token management endpoints.
//! Disabling this module for a tenant turns off their programmatic API access.
//!
//! Beyond the read endpoints, vendors *subscribe* to change events (issue
//! #68): `/api/v1/webhooks` manages callback registrations (scope-gated to
//! what the token can already read), and the module owns the
//! `webhook_deliver` job that signs and POSTs each event with retries +
//! dead-lettering.

use super::{JobContext, JobOutcome, ModuleManifest, PlatformModule};
use crate::rbac::Permission;
use crate::routes::{api_tokens, vendor};
use rocket::Route;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;

pub struct VendorApiModule;

#[rocket::async_trait]
impl PlatformModule for VendorApiModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "vendor_api",
            name: "Vendor API",
            description: "Scoped, revocable API tokens, the public /api/v1 endpoints, \
                          and outbound webhook subscriptions (signed, retried, replayable).",
            permissions: &[Permission::ApiTokenManage],
            job_kinds: &[crate::webhooks_out::DELIVER_JOB_KIND],
            default_enabled: true,
            preview: false,
        }
    }

    fn api(&self) -> (Vec<Route>, OpenApi) {
        openapi_get_routes_spec![
            api_tokens::list::list,
            api_tokens::create::create,
            api_tokens::revoke::revoke,
            vendor::listings::listings,
            vendor::properties::properties,
            // outbound webhooks: subscribe, don't poll (#68)
            vendor::webhooks::event_catalog,
            vendor::webhooks::list_subscriptions,
            vendor::webhooks::create_subscription,
            vendor::webhooks::update_subscription,
            vendor::webhooks::delete_subscription,
            vendor::webhooks::list_deliveries,
            vendor::webhooks::replay_delivery,
        ]
    }

    async fn handle_job(&self, ctx: &JobContext<'_>) -> Option<JobOutcome> {
        match ctx.job.kind.as_str() {
            k if k == crate::webhooks_out::DELIVER_JOB_KIND => {
                Some(crate::webhooks_out::handle_deliver_job(ctx.db, ctx.job).await)
            }
            _ => None,
        }
    }
}
