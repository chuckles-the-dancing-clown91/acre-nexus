//! **Rentals & Leasing** module — the operator's view of a rental portfolio:
//! the rentable units within each property, the leases that occupy them, and the
//! per-lease rent payment ledger that drives each lease's balance and standing.

use super::{ModuleManifest, PlatformModule};
use crate::rbac::Permission;
use crate::routes::{lifecycle, rentals};
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
            // move-in / move-out inspections (Phase 5)
            lifecycle::inspections::create_inspection,
            lifecycle::inspections::list_lease_inspections,
            lifecycle::inspections::get_inspection,
            lifecycle::inspections::update_inspection,
            lifecycle::inspections::complete_inspection,
            lifecycle::inspections::add_item,
            lifecycle::inspections::update_item,
            lifecycle::inspections::delete_item,
            lifecycle::inspections::my_inspections,
            // security-deposit disposition (Phase 5)
            lifecycle::deposits::get_lease_deposit,
            lifecycle::deposits::upsert_disposition,
            lifecycle::deposits::finalize_disposition,
            lifecycle::deposits::my_deposit,
        ]
    }
}
