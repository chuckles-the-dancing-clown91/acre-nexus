//! **Title & Ownership** module — the title-level view of a property: who holds
//! the deed and how it is vested (ownership records, including fractional shares),
//! and the liens/encumbrances recorded against the title.

use super::{ModuleManifest, PlatformModule};
use crate::rbac::Permission;
use crate::routes::title;
use rocket::Route;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;

pub struct TitleModule;

impl PlatformModule for TitleModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "title",
            name: "Title & Ownership",
            description: "Ownership of record (deed/vesting, fractional shares) and \
                          liens recorded against a property's title.",
            permissions: &[Permission::TitleRead, Permission::TitleManage],
            job_kinds: &[],
            default_enabled: true,
            preview: false,
        }
    }

    fn api(&self) -> (Vec<Route>, OpenApi) {
        openapi_get_routes_spec![
            title::list_ownership::list_ownership,
            title::create_ownership::create_ownership,
            title::update_ownership::update_ownership,
            title::delete_ownership::delete_ownership,
            title::list_liens::list_liens,
            title::create_lien::create_lien,
            title::update_lien::update_lien,
            title::delete_lien::delete_lien,
        ]
    }
}
