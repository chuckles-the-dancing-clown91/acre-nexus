//! **Rehab & Construction** module (roadmap Phase 7, issue #40) — the flip/BRRRR
//! renovation domain: a per-property rehab **budget**, **draws** against it (with
//! progress photos via the document service), **change orders**, and generated
//! **lien waivers**. Self-gated on per-tenant enablement, gated by
//! `rehab:read` / `rehab:manage`.

use super::{ModuleManifest, PlatformModule};
use crate::rbac::Permission;
use crate::routes::rehab;
use rocket::Route;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;

pub struct RehabModule;

impl PlatformModule for RehabModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "rehab",
            name: "Rehab & Construction",
            description: "Renovation budgets, draw requests with progress photos, \
                          change orders, and lien waivers for flip/BRRRR projects.",
            permissions: &[Permission::RehabRead, Permission::RehabManage],
            job_kinds: &[],
            default_enabled: true,
            preview: false,
        }
    }

    fn api(&self) -> (Vec<Route>, OpenApi) {
        openapi_get_routes_spec![
            rehab::projects::list,
            rehab::projects::create,
            rehab::projects::get,
            rehab::projects::update,
            rehab::lines::create,
            rehab::lines::update,
            rehab::lines::delete,
            rehab::change_orders::create,
            rehab::change_orders::decide,
            rehab::draws::create,
            rehab::draws::get,
            rehab::draws::set_status,
            rehab::lien_waivers::create,
            rehab::lien_waivers::update,
        ]
    }
}
