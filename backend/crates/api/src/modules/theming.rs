//! **Theming** module — per-tenant branding and legal-template management that
//! powers the white-label experience (logo, colours, default mode).

use super::{ModuleManifest, PlatformModule};
use crate::rbac::Permission;
use crate::routes::theme;
use rocket::{routes, Route};

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

    fn routes(&self) -> Vec<Route> {
        routes![theme::get_theme, theme::update_theme]
    }
}
