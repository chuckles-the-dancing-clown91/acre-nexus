//! **Entities & Contacts** module — the counterparty registry (banks, lenders,
//! insurers, title companies, contractors …) where investors keep who everyone is
//! and the running notes about them.

use super::{ModuleManifest, PlatformModule};
use crate::rbac::Permission;
use crate::routes::entities;
use rocket::Route;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;

pub struct EntitiesModule;

impl PlatformModule for EntitiesModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "entities",
            name: "Entities & Contacts",
            description: "Registry of banks, lenders, contractors and other \
                          counterparties, with notes.",
            permissions: &[Permission::EntityRead, Permission::EntityManage],
            job_kinds: &[],
            default_enabled: true,
            preview: false,
        }
    }

    fn api(&self) -> (Vec<Route>, OpenApi) {
        openapi_get_routes_spec![
            entities::list::list,
            entities::create::create,
            entities::get::get,
            entities::update::update,
            entities::add_note::add_note,
        ]
    }
}
