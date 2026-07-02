//! **Lease Builder & Tenancy** module — the application→onboarding→lease-signing
//! lifecycle: the conditional fee/discount/amenity schedule, resident vehicle
//! profiles, per-lease charges, templated lease-document generation + signing
//! (in person, or remotely via e-signature envelopes), application→lease
//! conversion, and the tenant-history view.

use super::{ModuleManifest, PlatformModule};
use crate::rbac::Permission;
use crate::routes::{
    applications, esign, fees, lease_charges, lease_docs, tenant_history, vehicles,
};
use rocket::Route;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;

pub struct LeaseBuilderModule;

impl PlatformModule for LeaseBuilderModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "lease_builder",
            name: "Lease Builder & Tenancy",
            description: "Conditional fees & discounts, vehicle profiles, templated \
                          lease documents + signing, application→lease conversion, \
                          and tenant history.",
            permissions: &[
                Permission::FeeRead,
                Permission::FeeManage,
                Permission::VehicleRead,
                Permission::VehicleManage,
            ],
            job_kinds: &[],
            default_enabled: true,
            preview: false,
        }
    }

    fn api(&self) -> (Vec<Route>, OpenApi) {
        openapi_get_routes_spec![
            // fee schedule
            fees::list::list,
            fees::create::create,
            fees::update::update,
            fees::delete::delete,
            // vehicles
            vehicles::list::list,
            vehicles::create::create,
            vehicles::update::update,
            vehicles::delete::delete,
            // self-service: the signed-in person's own vehicles
            vehicles::portal::my_vehicles,
            vehicles::portal::add_my_vehicle,
            vehicles::portal::delete_my_vehicle,
            // lease charges
            lease_charges::list::list,
            lease_charges::add::add,
            lease_charges::delete::delete,
            lease_charges::apply_fees::apply_fees,
            // lease documents
            lease_docs::generate::generate,
            lease_docs::get::get,
            lease_docs::sign::sign,
            // e-signature envelopes (remote signing)
            esign::create::create,
            esign::get::get,
            esign::remind::remind,
            esign::void::void,
            esign::public::view,
            esign::public::sign,
            esign::public::decline,
            // application -> lease
            applications::convert::convert,
            // tenant history
            tenant_history::list::list,
            tenant_history::property::property_history,
        ]
    }
}
