//! **Reports & Exports** module (roadmap Phase 8, issue #56) — the standard PM
//! reports every operator expects: rent roll, T-12 income statement, AR aging,
//! delinquency, owner statements, and the annual 1099 tax export, each viewable
//! in the console and exportable to CSV / PDF. Read-only, gated by
//! `report:read`, self-gated on per-tenant enablement.

use super::{ModuleManifest, PlatformModule};
use crate::rbac::Permission;
use crate::routes::reports;
use rocket::Route;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;

pub struct ReportsModule;

impl PlatformModule for ReportsModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "reports",
            name: "Reports & Exports",
            description: "Standard PM reports — rent roll, T-12, aging, \
                          delinquency, owner statements, and 1099 tax export — \
                          with CSV / PDF export.",
            permissions: &[Permission::ReportRead],
            job_kinds: &[],
            default_enabled: true,
            preview: false,
        }
    }

    fn api(&self) -> (Vec<Route>, OpenApi) {
        openapi_get_routes_spec![
            reports::rent_roll::rent_roll,
            reports::rent_roll::rent_roll_export,
            reports::t12::t12,
            reports::t12::t12_export,
            reports::aging::aging,
            reports::aging::aging_export,
            reports::delinquency::delinquency,
            reports::delinquency::delinquency_export,
            reports::owner_statement::owner_statement,
            reports::owner_statement::owner_statement_export,
            reports::tax_1099::tax_1099,
            reports::tax_1099::tax_1099_export,
        ]
    }
}
