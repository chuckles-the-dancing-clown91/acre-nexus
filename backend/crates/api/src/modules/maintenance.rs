//! **Maintenance & Work Orders** module — repair/turn tickets tracked against
//! properties (optionally a unit/lease), assignable to a member or an external
//! contractor, with a per-ticket activity timeline of comments and status changes.
//!
//! Phase 6 grew it into the helpdesk: per-priority SLA targets stamped on
//! every ticket (breaches surfaced by the per-tenant `helpdesk_scan` job),
//! contractor quotes whose approval feeds the vendor-bill prefill, and
//! preventive-maintenance plans that open tickets on schedule.

use super::{JobContext, JobOutcome, ModuleManifest, PlatformModule};
use crate::rbac::Permission;
use crate::routes::maintenance;
use rocket::Route;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;

pub struct MaintenanceModule;

#[rocket::async_trait]
impl PlatformModule for MaintenanceModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "maintenance",
            name: "Maintenance & Work Orders",
            description: "Repair/turn tickets against properties, units and leases, \
                          assignable to members or contractors, with a comment timeline, \
                          SLA tracking, contractor quotes, and preventive plans.",
            permissions: &[Permission::MaintenanceRead, Permission::MaintenanceManage],
            job_kinds: &[crate::helpdesk::SCAN_KIND],
            default_enabled: true,
            preview: false,
        }
    }

    fn api(&self) -> (Vec<Route>, OpenApi) {
        openapi_get_routes_spec![
            maintenance::list_tickets::list_tickets,
            maintenance::list_property_tickets::list_property_tickets,
            maintenance::property_maintenance::property_maintenance,
            maintenance::create_ticket::create_ticket,
            maintenance::get_ticket::get_ticket,
            maintenance::update_ticket::update_ticket,
            maintenance::add_comment::add_comment,
            // renter portal: the resident's own maintenance requests
            maintenance::portal::my_tickets,
            maintenance::portal::create_my_ticket,
            maintenance::portal::my_ticket_detail,
            maintenance::portal::add_my_comment,
            maintenance::portal::add_my_ticket_photo,
            // equipment registry (assets)
            maintenance::assets::list_assets,
            maintenance::assets::create_asset,
            maintenance::assets::update_asset,
            // helpdesk (Phase 6): quotes + preventive plans
            maintenance::quotes::add_quote,
            maintenance::quotes::approve_quote,
            maintenance::quotes::reject_quote,
            maintenance::plans::list_plans,
            maintenance::plans::create_plan,
            maintenance::plans::update_plan,
        ]
    }

    async fn handle_job(&self, ctx: &JobContext<'_>) -> Option<JobOutcome> {
        match ctx.job.kind.as_str() {
            k if k == crate::helpdesk::SCAN_KIND => {
                Some(crate::helpdesk::handle_scan_job(ctx.db, ctx.job).await)
            }
            _ => None,
        }
    }
}
