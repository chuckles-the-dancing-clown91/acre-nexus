//! **Property management** module — the portfolio, individual property profiles,
//! and the LLC holding entities that group them. Core to the product, so it is
//! enabled for every tenant by default.

use super::{ModuleManifest, PlatformModule};
use crate::rbac::Permission;
use crate::routes::{
    banking, cap_table, llcs, mortgages, onboarding, portfolio, portfolios, properties, workflow,
};
use rocket::Route;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;

pub struct PropertiesModule;

impl PlatformModule for PropertiesModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "properties",
            name: "Property Management",
            description: "Portfolio, onboarding, property profiles, financing, \
                          investment workflows, and LLC holding entities.",
            permissions: &[
                Permission::PropertyRead,
                Permission::PropertyWrite,
                Permission::FinanceRead,
                Permission::FinanceManage,
                Permission::EntityRead,
                Permission::EntityManage,
            ],
            job_kinds: &[],
            default_enabled: true,
            preview: false,
        }
    }

    fn api(&self) -> (Vec<Route>, OpenApi) {
        openapi_get_routes_spec![
            properties::list::list,
            properties::create::create,
            properties::profile::profile,
            properties::update::update,
            onboarding::onboard::onboard,
            portfolio::summary::summary,
            portfolio::llc_groups::llc_groups,
            llcs::list::list,
            llcs::create::create,
            mortgages::list::list,
            mortgages::create::create,
            mortgages::update::update,
            mortgages::delete::delete,
            workflow::get::get_workflow,
            workflow::advance::advance,
            // tenancy spec: portfolios, cap table, banking, onboarding workflow
            portfolios::list::list,
            portfolios::create::create,
            cap_table::list::list,
            cap_table::add::add,
            banking::list::list,
            banking::create::create,
            onboarding::workflow::get_onboarding_workflow,
            onboarding::workflow::advance_onboarding,
        ]
    }
}
