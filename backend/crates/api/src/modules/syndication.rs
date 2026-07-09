//! **Investor / syndication** module (issue #13, Beyond-GA vertical) — capital
//! commitments, capital calls (split pro-rata by committed capital), and
//! distribution waterfalls (return of capital → preferred → carried interest) on
//! a legal entity. Gated by `investor:read` / `investor:manage`; the waterfall
//! math lives in [`crate::syndication`]. Self-gating on per-tenant enablement.

use super::{ModuleManifest, PlatformModule};
use crate::rbac::Permission;
use crate::routes::syndication;
use rocket::Route;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;

pub struct SyndicationModule;

impl PlatformModule for SyndicationModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "syndication",
            name: "Investor Syndication",
            description: "Investor commitments, capital calls, and distribution \
                          waterfalls (return of capital → preferred → carry) for \
                          GP/LP legal entities.",
            permissions: &[Permission::InvestorRead, Permission::InvestorManage],
            job_kinds: &[],
            default_enabled: true,
            preview: false,
        }
    }

    fn api(&self) -> (Vec<Route>, OpenApi) {
        openapi_get_routes_spec![
            syndication::commitments::list,
            syndication::commitments::create,
            syndication::capital_calls::create,
            syndication::capital_calls::fund,
            syndication::distributions::create,
            syndication::distributions::list,
        ]
    }
}
