//! **Vendor API** module — the token-authenticated `/api/v1` surface sold to
//! third-party integrators, plus the tenant-facing token management endpoints.
//! Disabling this module for a tenant turns off their programmatic API access.

use super::{ModuleManifest, PlatformModule};
use crate::rbac::Permission;
use crate::routes::{api_tokens, vendor};
use rocket::Route;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;

pub struct VendorApiModule;

impl PlatformModule for VendorApiModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "vendor_api",
            name: "Vendor API",
            description: "Scoped, revocable API tokens and the public /api/v1 endpoints.",
            permissions: &[Permission::ApiTokenManage],
            job_kinds: &[],
            default_enabled: true,
            preview: false,
        }
    }

    fn api(&self) -> (Vec<Route>, OpenApi) {
        openapi_get_routes_spec![
            api_tokens::list,
            api_tokens::create,
            api_tokens::revoke,
            vendor::listings,
            vendor::properties,
        ]
    }
}
