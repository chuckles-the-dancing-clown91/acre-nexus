//! **Team & Time** module (Vantedge phase 2B) — the back office's people and
//! clock: staff profiles with pay and bill rates, the time clock against work
//! orders / rehab projects / properties, timesheets with approval and missed
//! punches, shifts, and time off. A per-tenant `workforce_scan` job closes
//! clock-ins left running and, once a week, tells the office what's still
//! waiting for approval.

use super::{JobContext, JobOutcome, ModuleManifest, PlatformModule};
use crate::rbac::Permission;
use crate::routes::team;
use rocket::Route;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;

pub struct TeamModule;

#[rocket::async_trait]
impl PlatformModule for TeamModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "team",
            name: "Team & Time",
            description: "Staff profiles, the time clock against work orders and projects, \
                 timesheets with approval and missed punches, shifts, and time off — the \
                 hours payroll, work-order costs and owner billing are built from.",
            permissions: &[
                Permission::TeamRead,
                Permission::TeamManage,
                Permission::PayrollRead,
            ],
            job_kinds: &[crate::workforce::SCAN_KIND],
            default_enabled: true,
            preview: false,
        }
    }

    fn api(&self) -> (Vec<Route>, OpenApi) {
        openapi_get_routes_spec![
            team::employees::roster,
            team::employees::upsert,
            team::time::list,
            team::time::create,
            team::time::update,
            team::time::remove,
            team::time::approve,
            team::time::approve_many,
            team::time::resolve,
            team::time::clock_out,
            team::time::summary,
            team::schedule::list_shifts,
            team::schedule::create_shift,
            team::schedule::update_shift,
            team::schedule::delete_shift,
            team::schedule::list_time_off,
            team::schedule::review_time_off,
            // self-service
            team::clock::clock,
            team::clock::clock_in,
            team::clock::clock_out,
            team::clock::my_time,
            team::clock::add_time,
            team::clock::edit_time,
            team::clock::delete_time,
            team::clock::claim,
            team::clock::my_hours,
            team::clock::my_work,
            team::schedule::my_shifts,
            team::schedule::my_time_off,
            team::schedule::request_time_off,
            team::schedule::cancel_time_off,
        ]
    }

    async fn handle_job(&self, ctx: &JobContext<'_>) -> Option<JobOutcome> {
        match ctx.job.kind.as_str() {
            k if k == crate::workforce::SCAN_KIND => {
                Some(crate::workforce::handle_scan_job(ctx.db, ctx.job).await)
            }
            _ => None,
        }
    }
}
