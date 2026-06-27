//! **Rentals & Leasing** module — the operator's view of a rental portfolio:
//! the rentable units within each property, the leases that occupy them, and the
//! per-lease rent payment ledger that drives each lease's balance and standing.

use super::{ModuleManifest, PlatformModule};
use crate::rbac::Permission;
use crate::routes::rentals;
use rocket::Route;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;

pub struct RentalsModule;

impl PlatformModule for RentalsModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "rentals",
            name: "Rentals & Leasing",
            description: "Units, leases, and the rent payment ledger for a rental portfolio.",
            permissions: &[Permission::LeaseRead, Permission::LeaseManage],
            job_kinds: &[],
            default_enabled: true,
            preview: false,
        }
    }

    fn api(&self) -> (Vec<Route>, OpenApi) {
        openapi_get_routes_spec![
            rentals::list_units::list_units,
            rentals::create_unit::create_unit,
            rentals::update_unit::update_unit,
            rentals::list_leases::list_leases,
            rentals::list_property_leases::list_property_leases,
            rentals::create_lease::create_lease,
            rentals::get_lease::get_lease,
            rentals::update_lease::update_lease,
            rentals::list_payments::list_payments,
            rentals::record_payment::record_payment,
        ]
    }
}
