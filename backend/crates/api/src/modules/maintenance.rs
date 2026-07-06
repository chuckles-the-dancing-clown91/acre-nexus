//! **Maintenance & Work Orders** module — repair/turn tickets tracked against
//! properties (optionally a unit/lease), assignable to a member or an external
//! contractor, with a per-ticket activity timeline of comments and status changes.

use super::{ModuleManifest, PlatformModule};
use crate::rbac::Permission;
use crate::routes::maintenance;
use rocket::Route;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;

pub struct MaintenanceModule;

impl PlatformModule for MaintenanceModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "maintenance",
            name: "Maintenance & Work Orders",
            description: "Repair/turn tickets against properties, units and leases, \
                          assignable to members or contractors, with a comment timeline.",
            permissions: &[Permission::MaintenanceRead, Permission::MaintenanceManage],
            job_kinds: &[],
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
        ]
    }
}
