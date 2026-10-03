//! **Calendar & Reminders** module (issue #54) — the cross-cutting scheduling
//! engine: a generic `reminder` entity (lease renewals, license / insurance
//! expirations, tours, inspections, custom dates), a per-tenant
//! self-rescheduling `reminder_scan` job that fires notifications through the
//! Phase 1 substrate at each configured lead time, and the console calendar's
//! API. On by default — expiry tracking is table stakes for a PM platform.

use super::{JobContext, JobOutcome, ModuleManifest, PlatformModule};
use crate::rbac::Permission;
use crate::routes::reminders;
use rocket::Route;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;

pub struct CalendarModule;

#[rocket::async_trait]
impl PlatformModule for CalendarModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "calendar",
            name: "Calendar & Reminders",
            description: "One schedule for everything with a due date: lease renewals \
                 (auto-synced), license / insurance expirations, tours, and inspections — \
                 notified at configurable lead times through the notification substrate.",
            permissions: &[Permission::CalendarRead, Permission::CalendarManage],
            job_kinds: &[
                crate::reminders::SCAN_KIND,
                crate::resident_reminders::KIND,
                crate::resident_reminders::DIGEST_KIND,
                crate::appointments::REMINDER_KIND,
            ],
            default_enabled: true,
            preview: false,
        }
    }

    fn api(&self) -> (Vec<Route>, OpenApi) {
        openapi_get_routes_spec![
            reminders::list::list_reminders,
            reminders::create::create_reminder,
            reminders::update::update_reminder,
            reminders::delete::delete_reminder,
            crate::routes::appointments::list_appointments,
            crate::routes::appointments::create_appointment,
            crate::routes::appointments::get_appointment,
            crate::routes::appointments::update_appointment,
            crate::routes::appointments::public_view,
            crate::routes::appointments::public_pick,
            crate::routes::appointments::public_decline,
            crate::routes::appointments::my_appointments,
            crate::routes::appointments::my_pick,
            crate::routes::appointments::my_decline,
        ]
    }

    async fn handle_job(&self, ctx: &JobContext<'_>) -> Option<JobOutcome> {
        match ctx.job.kind.as_str() {
            k if k == crate::reminders::SCAN_KIND => {
                Some(crate::reminders::handle_scan_job(ctx.db, ctx.job).await)
            }
            k if k == crate::resident_reminders::KIND => {
                Some(crate::resident_reminders::handle_job(ctx.db, ctx.job).await)
            }
            k if k == crate::resident_reminders::DIGEST_KIND => {
                Some(crate::resident_reminders::handle_digest_job(ctx.db, ctx.job).await)
            }
            k if k == crate::appointments::REMINDER_KIND => {
                Some(crate::appointments::handle_reminder_job(ctx.db, ctx.job).await)
            }
            _ => None,
        }
    }
}
