//! **Resident messaging** module (roadmap Phase 5, issue #9) — "message the
//! manager" from the renter portal, answered from the console. One thread per
//! conversation on a lease with a flat message timeline; both directions
//! notify through the Phase 1 substrate.

use super::{ModuleManifest, PlatformModule};
use crate::rbac::Permission;
use crate::routes::messages;
use rocket::Route;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;

pub struct MessagingModule;

impl PlatformModule for MessagingModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "messaging",
            name: "Resident Messaging",
            description: "Resident ↔ manager message threads: residents write from the \
                          portal, staff reply from the console, both sides notified.",
            permissions: &[Permission::MessageRead, Permission::MessageManage],
            job_kinds: &[],
            default_enabled: true,
            preview: false,
        }
    }

    fn api(&self) -> (Vec<Route>, OpenApi) {
        openapi_get_routes_spec![
            // staff console
            messages::console::list_threads,
            messages::console::get_thread,
            messages::console::reply_thread,
            messages::console::update_thread,
            // renter portal
            messages::portal::my_threads,
            messages::portal::create_my_thread,
            messages::portal::my_thread_detail,
            messages::portal::reply_my_thread,
        ]
    }
}
