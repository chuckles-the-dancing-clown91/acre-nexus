//! **Import & Export** module: moving a workspace in from another property
//! management tool (AppFolio, Buildium, Yardi Breeze, Rent Manager, DoorLoop,
//! or any spreadsheet) and taking its data out as CSV. See
//! [`crate::imports`].

use super::{ModuleManifest, PlatformModule};
use crate::rbac::Permission;
use crate::routes::imports;
use rocket::Route;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;

pub struct DataModule;

impl PlatformModule for DataModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "data",
            name: "Import & Export",
            description: "Bring properties, units, tenants and leases, owners and \
                          vendors over from another tool's CSV export, with a preview \
                          and undo; download the workspace as CSV.",
            permissions: &[Permission::DataImport, Permission::DataExport],
            job_kinds: &[],
            default_enabled: true,
            preview: false,
        }
    }

    fn api(&self) -> (Vec<Route>, OpenApi) {
        openapi_get_routes_spec![
            imports::catalog,
            imports::template,
            imports::upload,
            imports::update_draft,
            imports::preview,
            imports::commit,
            imports::discard,
            imports::list,
            imports::undo_import,
            imports::exports,
            imports::download,
        ]
    }
}
