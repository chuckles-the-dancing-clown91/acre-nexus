//! **Theming** module — per-tenant branding and legal-template management that
//! powers the white-label experience (logo, colours, default mode).

use super::{ModuleManifest, PlatformModule};
use crate::rbac::Permission;
use crate::routes::theme;
use rocket::Route;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;

pub struct ThemingModule;

impl PlatformModule for ThemingModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "theming",
            name: "Branding & Theming",
            description: "White-label branding, colours, and legal templates.",
            permissions: &[Permission::ThemeWrite],
            job_kinds: &[],
            default_enabled: true,
            preview: false,
        }
    }

    fn api(&self) -> (Vec<Route>, OpenApi) {
        openapi_get_routes_spec![
            theme::get_theme::get_theme,
            theme::update_theme::update_theme
        ]
    }
}
