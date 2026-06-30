//! **Domains & Routing** module — white-label host mapping: a tenant can serve an
//! admin app, an owner portal, and a renter portal, each on its own verified
//! hostname (§7). The public host→tenant resolver is a core route; this module
//! owns the authenticated management surface.

use super::{ModuleManifest, PlatformModule};
use crate::rbac::Permission;
use crate::routes::domains;
use rocket::Route;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;

pub struct DomainsModule;

impl PlatformModule for DomainsModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "domains",
            name: "Domains & Routing",
            description: "White-label custom domains and audience routing \
                          (admin / owner / renter portals).",
            permissions: &[Permission::DomainRead, Permission::DomainManage],
            job_kinds: &[],
            default_enabled: true,
            preview: false,
        }
    }

    fn api(&self) -> (Vec<Route>, OpenApi) {
        openapi_get_routes_spec![
            domains::list::list,
            domains::create::create,
            domains::verify::verify,
            domains::delete::delete,
        ]
    }
}
