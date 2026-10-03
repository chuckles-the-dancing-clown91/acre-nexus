//! **Property management** module — the portfolio, individual property profiles,
//! and the LLC holding entities that group them. Core to the product, so it is
//! enabled for every tenant by default.

use super::{ModuleManifest, PlatformModule};
use crate::rbac::Permission;
use crate::routes::{
    assignments, banking, cap_table, llcs, mortgages, onboarding, portfolio, portfolios,
    properties, workflow,
};
use rocket::Route;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;

pub struct PropertiesModule;

impl PlatformModule for PropertiesModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "properties",
            name: "Properties & Portfolio",
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
            crate::routes::properties::autofill::get_autofill,
            crate::routes::properties::autofill::apply_autofill,
            crate::routes::sitemaps::list_maps,
            crate::routes::sitemaps::create_map,
            crate::routes::sitemaps::get_map,
            crate::routes::sitemaps::update_map,
            crate::routes::sitemaps::delete_map,
            crate::routes::sitemaps::save_features,
            crate::routes::sitemaps::export_geojson,
            crate::routes::sitemaps::import_geojson,
            crate::routes::sitemaps::plan,
            properties::list::list,
            properties::create::create,
            properties::profile::profile,
            properties::financials::financials,
            properties::update::update,
            properties::media::list_media,
            properties::media::set_hero,
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
            workflow::catalog::catalog,
            // tenancy spec: portfolios, cap table, banking, onboarding workflow
            portfolios::list::list,
            portfolios::create::create,
            cap_table::list::list,
            cap_table::add::add,
            banking::list::list,
            banking::create::create,
            onboarding::workflow::get_onboarding_workflow,
            onboarding::workflow::advance_onboarding,
            // staff assignments (property + LLC), each grants scoped access
            assignments::property::list,
            assignments::property::create,
            assignments::property::delete,
            assignments::llc::list,
            assignments::llc::create,
            assignments::llc::delete,
            // the full profile: permits, insurance, schools, action items
            crate::routes::property_records::permits::list_permits,
            crate::routes::property_records::permits::create_permit,
            crate::routes::property_records::permits::update_permit,
            crate::routes::property_records::permits::delete_permit,
            crate::routes::property_records::insurance::list_policies,
            crate::routes::property_records::insurance::create_policy,
            crate::routes::property_records::insurance::update_policy,
            crate::routes::property_records::insurance::delete_policy,
            crate::routes::property_records::schools::list_schools,
            crate::routes::property_records::schools::create_school,
            crate::routes::property_records::schools::update_school,
            crate::routes::property_records::schools::delete_school,
            crate::routes::property_records::action_items::list_action_items,
            crate::routes::property_records::action_items::create_action_item,
            crate::routes::property_records::action_items::update_action_item,
            crate::routes::property_records::action_items::delete_action_item,
            crate::routes::property_records::attention::attention,
        ]
    }
}
