//! **HOA / association management** module (issue #13, Beyond-GA vertical) —
//! community associations with members, dues assessments, CC&R violations, and
//! architectural (ARC) requests. Gated by `hoa:read` / `hoa:manage`. Self-gating
//! on per-tenant enablement.

use super::{ModuleManifest, PlatformModule};
use crate::rbac::Permission;
use crate::routes::hoa;
use rocket::Route;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;

pub struct HoaModule;

impl PlatformModule for HoaModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "hoa",
            name: "HOA / Associations",
            description: "Community associations: members, dues assessments, CC&R \
                          violations, and architectural (ARC) requests.",
            permissions: &[Permission::HoaRead, Permission::HoaManage],
            job_kinds: &[],
            // A distinct vertical — off by default; tenants opt in.
            default_enabled: false,
            preview: true,
        }
    }

    fn api(&self) -> (Vec<Route>, OpenApi) {
        openapi_get_routes_spec![
            hoa::associations::list,
            hoa::associations::create,
            hoa::members::list,
            hoa::members::create,
            hoa::assessments::create,
            hoa::assessments::list,
            hoa::violations::create,
            hoa::violations::update,
            hoa::violations::list,
            hoa::arc::create,
            hoa::arc::decide,
            hoa::arc::list,
        ]
    }
}
