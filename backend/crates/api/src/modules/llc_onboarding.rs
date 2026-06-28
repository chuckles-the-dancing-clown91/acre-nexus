//! **LLC Onboarding** module — turns a bare holding entity into a fully
//! onboarded company: its filing/contact profile, uploaded logo + legal
//! documents, branding (logo / colours / signature block / boilerplate
//! verbiage), reusable templates, and auto-generated lease & letter PDFs. Also
//! owns the per-tenant storage-backend configuration.

use super::{ModuleManifest, PlatformModule};
use crate::rbac::Permission;
use crate::routes::llcs;
use rocket::Route;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;

pub struct LlcOnboardingModule;

impl PlatformModule for LlcOnboardingModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "llc_onboarding",
            name: "LLC Onboarding",
            description: "Onboard holding companies: documents, branding, signature blocks, and auto-generated leases & letters.",
            permissions: &[
                Permission::LlcRead,
                Permission::LlcManage,
                Permission::StorageManage,
            ],
            job_kinds: &[],
            default_enabled: true,
            preview: false,
        }
    }

    fn api(&self) -> (Vec<Route>, OpenApi) {
        let (mut routes, spec) = openapi_get_routes_spec![
            llcs::get::get,
            llcs::update::update,
            llcs::documents::list_documents,
            llcs::documents::delete_document,
            llcs::branding::get_branding,
            llcs::branding::put_branding,
            llcs::templates::list_templates,
            llcs::templates::create_template,
            llcs::templates::update_template,
            llcs::templates::delete_template,
            llcs::templates::preview_template,
            llcs::generate::generate_document,
            llcs::generate::list_generated,
            llcs::storage::get_storage_config,
            llcs::storage::put_storage_config,
        ];
        // Multipart upload + binary downloads carry non-JSON bodies, so they are
        // plain Rocket routes, mounted alongside but absent from the OpenAPI doc.
        routes.append(&mut rocket::routes![
            llcs::documents::upload_document,
            llcs::documents::download_document,
            llcs::generate::download_generated,
        ]);
        (routes, spec)
    }
}
