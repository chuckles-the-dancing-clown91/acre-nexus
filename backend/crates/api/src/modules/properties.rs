//! **Property management** module — the portfolio, individual property profiles,
//! and the LLC holding entities that group them. Core to the product, so it is
//! enabled for every tenant by default.

use super::{ModuleManifest, PlatformModule};
use crate::rbac::Permission;
use crate::routes::{llcs, portfolio, properties};
use rocket::Route;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;

pub struct PropertiesModule;

impl PlatformModule for PropertiesModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "properties",
            name: "Property Management",
            description: "Portfolio, property profiles, and LLC holding entities.",
            permissions: &[Permission::PropertyRead, Permission::PropertyWrite],
            job_kinds: &[],
            default_enabled: true,
            preview: false,
        }
    }

    fn api(&self) -> (Vec<Route>, OpenApi) {
        openapi_get_routes_spec![
            properties::list,
            properties::create,
            properties::profile,
            properties::update,
            portfolio::summary,
            portfolio::llc_groups,
            llcs::list,
            llcs::create,
        ]
    }
}
