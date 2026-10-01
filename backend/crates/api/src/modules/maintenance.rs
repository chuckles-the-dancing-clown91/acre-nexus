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
            job_kinds: &[crate::helpdesk::SCAN_KIND, crate::partner::DISPATCH_KIND],
            default_enabled: true,
            preview: false,
        }
    }

    fn api(&self) -> (Vec<Route>, OpenApi) {
        openapi_get_routes_spec![
            crate::routes::process::list_templates,
            crate::routes::process::create_template,
            crate::routes::process::update_template,
            crate::routes::process::list_processes,
            crate::routes::process::get_process,
            crate::routes::process::start_turn,
            crate::routes::process::step_action,
            crate::routes::process::step_ticket,
            crate::routes::process::finish_process,
            crate::routes::process::cancel_process,
            maintenance::issues::list_issues,
            maintenance::issues::create_issue,
            maintenance::issues::update_issue,
            maintenance::issues::retire_issue,
            maintenance::issues::generate_ticket,
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
            maintenance::portal::review_my_ticket,
            // parts / labor / fees + the stockroom
            maintenance::lines::add_line,
            maintenance::lines::remove_line,
            maintenance::inventory::list_inventory,
            maintenance::inventory::create_inventory,
            maintenance::inventory::update_inventory,
            // equipment registry (assets)
            maintenance::assets::list_assets,
            maintenance::assets::create_asset,
            maintenance::assets::update_asset,
            // helpdesk (Phase 6): quotes + preventive plans
            maintenance::quotes::add_quote,
            maintenance::quotes::approve_quote,
            maintenance::quotes::reject_quote,
            maintenance::desk::list_tasks,
            maintenance::desk::add_task,
            maintenance::desk::update_task,
            maintenance::desk::remove_task,
            maintenance::desk::dispatch_task,
            maintenance::desk::apply_kit,
            maintenance::desk::costs,
            maintenance::desk::upload,
            maintenance::desk::files,
            maintenance::desk::list_expenses,
            maintenance::desk::add_expense,
            maintenance::desk::vendors,
            maintenance::plans::list_plans,
            maintenance::plans::create_plan,
            maintenance::plans::update_plan,
            // phase 2C: appliances, findings, the parts loop and close-out
            maintenance::parts::asset_parts,
            maintenance::parts::asset_history,
            maintenance::parts::put_asset_part,
            maintenance::parts::delete_asset_part,
            maintenance::parts::asset_work_order,
            maintenance::parts::add_finding,
            maintenance::parts::list_findings,
            maintenance::parts::list_parts,
            maintenance::parts::create_part,
            maintenance::parts::update_part,
            maintenance::parts::delete_part,
            maintenance::parts::generate_list,
            maintenance::parts::parts_list_pdf,
            maintenance::parts::closeout,
            maintenance::parts::decide,
            maintenance::parts::receive,
            maintenance::parts::use_part,
            // stock: scan-in, receiving, counts, the ledger, reorder
            maintenance::stock::lookup,
            maintenance::stock::receive,
            maintenance::stock::count,
            maintenance::stock::movements,
            maintenance::stock::reorder,
            // Alpha ↔ Vantedge: link a vendor, send them work
            crate::routes::partner::get_link,
            crate::routes::partner::link,
            crate::routes::partner::unlink,
            crate::routes::partner::rotate_secret,
            crate::routes::partner::linked_vendors,
            crate::routes::partner::dispatch,
        ]
    }

    async fn handle_job(&self, ctx: &JobContext<'_>) -> Option<JobOutcome> {
        match ctx.job.kind.as_str() {
            k if k == crate::helpdesk::SCAN_KIND => {
                Some(crate::helpdesk::handle_scan_job(ctx.db, ctx.job).await)
            }
            k if k == crate::partner::DISPATCH_KIND => {
                Some(crate::partner::handle_dispatch_job(ctx.db, ctx.job).await)
            }
            _ => None,
        }
    }
}
