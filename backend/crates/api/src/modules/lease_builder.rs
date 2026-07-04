//! **Lease Builder & Tenancy** module — the application→onboarding→lease-signing
//! lifecycle: the conditional fee/discount/amenity schedule, resident vehicle
//! profiles, per-lease charges, templated lease-document generation + signing
//! (in person, or remotely via e-signature envelopes), application→lease
//! conversion, and the tenant-history view.

use super::{JobContext, JobOutcome, ModuleManifest, PlatformModule};
use crate::rbac::Permission;
use crate::routes::{
    applications, esign, fees, lease_charges, lease_docs, tenant_history, vehicles,
};
use rocket::Route;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;
use serde_json::json;
use uuid::Uuid;

pub struct LeaseBuilderModule;

#[rocket::async_trait]
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
            // Deferred signed-PDF stores (a storage hiccup at completion time
            // degrades to this retryable job instead of blocking the signer).
            job_kinds: &["esign_store_pdf"],
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
            esign::public::mark_viewed,
            esign::public::sign,
            esign::public::decline,
            // application -> lease
            applications::convert::convert,
            // tenant history
            tenant_history::list::list,
            tenant_history::property::property_history,
        ]
    }

    /// Retry a deferred signed-PDF store until it lands (or the retry budget
    /// coerces a terminal failure that keeps the error visible on the job).
    async fn handle_job(&self, ctx: &JobContext<'_>) -> Option<JobOutcome> {
        if ctx.job.kind != "esign_store_pdf" {
            return None;
        }
        let Some(envelope_id) = ctx
            .job
            .payload
            .get("envelope_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
        else {
            return Some(JobOutcome::failed(
                "esign_store_pdf payload missing envelope_id",
            ));
        };
        match crate::esign::retry_store_pdf(ctx.db, ctx.job.tenant_id, envelope_id).await {
            Ok(doc_id) => Some(JobOutcome::completed(
                json!({ "signed_document_id": doc_id }),
            )),
            Err(e) => Some(JobOutcome::retry(
                crate::providers::backoff(ctx.job.attempts),
                format!("signed-PDF store still failing: {e}"),
            )),
        }
    }
}
