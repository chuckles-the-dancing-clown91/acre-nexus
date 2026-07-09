//! # Acre API
//!
//! Rust backend for the Acre multi-tenant property-management platform, built on
//! **Rocket** (HTTP), **SeaORM** (Postgres), and **Tokio** (background automation).
//!
//! ## Modules
//! * [`auth`] — JWT access/refresh tokens + Argon2 passwords + the `AuthUser` guard
//! * [`rbac`] — fine-grained permissions and system roles
//! * [`tenancy`] — tenant-resolution guards (shared-schema multi-tenancy)
//! * [`tokens`] — scoped, revocable API tokens for the vendor API
//! * [`scheduler`] — Tokio background job engine (screening, automated emails)
//! * [`routes`] — HTTP handlers, grouped by audience
//! * [`modules`] — pluggable feature modules; each contributes routes + OpenAPI
//! * [`openapi`] — trait impls that let `rocket_okapi` document our guards/errors
//!
//! ## Boot sequence
//! Connect to Postgres → (optionally) migrate + seed → spawn the scheduler →
//! launch Rocket with core + module routes, the merged OpenAPI doc, and the
//! Swagger UI / RapiDoc explorers mounted.

#[macro_use]
extern crate rocket;

mod accounting;
mod app_workflow;
mod audit;
mod auth;
mod bankfeed;
mod billing;
mod config;
mod cors;
mod db;
mod deals;
mod deposits;
mod dto;
mod enrichment;
mod error;
mod esign;
mod finance;
mod guards;
mod helpdesk;
mod leasedoc;
mod listing_sync;
mod mail;
mod modules;
mod notify;
mod observability;
mod openapi;
mod payables;
mod payments;
mod payouts;
mod pdf;
mod pii;
mod providers;
mod ratelimit;
mod rbac;
mod reminders;
mod rentals_occupancy;
mod routes;
mod saas;
mod scheduler;
mod screening;
mod secrets;
mod seed;
mod settings;
mod state;
mod storage;
mod tenancy;
mod tokens;
mod underwriting;
mod webhooks_out;
mod workflow;

use config::Config;
use migration::{Migrator, MigratorTrait};
use rocket_okapi::okapi::merge::merge_specs;
use rocket_okapi::okapi::openapi3::{Info, OpenApi};
use rocket_okapi::rapidoc::{make_rapidoc, GeneralConfig, HideShowConfig, RapiDocConfig};
use rocket_okapi::settings::OpenApiSettings;
use rocket_okapi::swagger_ui::{make_swagger_ui, SwaggerUIConfig};
use sea_orm::Database;
use state::AppState;

#[launch]
async fn rocket() -> _ {
    // Structured logging. `LOG_FORMAT=json` switches to newline-delimited JSON
    // (for shipping to a log aggregator, where each line's `request_id` field —
    // when present, see `error::ApiError`'s Responder — joins it to the matching
    // `audit_log` row written by `AuditFairing`); anything else stays
    // human-readable `fmt` output for local development.
    let env_filter = || {
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "info,sqlx=warn".into())
    };
    let json_logs = std::env::var("LOG_FORMAT")
        .map(|v| v.eq_ignore_ascii_case("json"))
        .unwrap_or(false);
    let _ = if json_logs {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(env_filter())
            .try_init()
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(env_filter())
            .try_init()
    };

    let config = Config::global().clone();
    tracing::info!("connecting to database…");
    let db = Database::connect(&config.database_url)
        .await
        .expect("failed to connect to database");

    if config.auto_migrate {
        tracing::info!("running migrations…");
        Migrator::up(&db, None).await.expect("migration failed");
        seed::run(&db).await.expect("seed failed");
    }

    // Spawn the Tokio background scheduler, and make sure every tenant has
    // its recurring billing cycle + reminder scan scheduled (idempotent).
    scheduler::spawn(db.clone());
    billing::ensure_recurring_jobs(&db).await;
    reminders::ensure_recurring_jobs(&db).await;
    helpdesk::ensure_recurring_jobs(&db).await;
    saas::ensure_recurring_jobs(&db).await;

    let state = AppState { db, config };

    // Accumulate the merged OpenAPI document as we mount routes. Core routes
    // first, then every pluggable module's routes — each module contributes both
    // its routes and a matching spec fragment.
    let mut spec = OpenApi::new();
    // Raise the default body limits: document uploads (`Vec<u8>` blobs, 25 MiB
    // to match `routes::documents::MAX_SIZE_BYTES`) and raw webhook payloads
    // (`String`, 1 MiB) both exceed Rocket's 8 KiB defaults.
    let figment = rocket::Config::figment()
        .merge(("limits.bytes", "25MiB"))
        .merge(("limits.string", "1MiB"));
    let mut app = rocket::custom(figment)
        .manage(state)
        .attach(observability::MetricsFairing)
        .attach(ratelimit::RateLimiter::from_env())
        .attach(cors::Cors)
        .attach(db::TxCommit)
        .attach(audit::AuditFairing);

    let (core_routes, core_spec) = routes::core_api();
    if let Err(e) = merge_specs(&mut spec, &"", &core_spec) {
        tracing::error!("failed to merge core OpenAPI spec: {e}");
    }
    app = app.mount("/", core_routes);

    for module in modules::registry() {
        let manifest = module.manifest();
        let (routes, module_spec) = module.api();
        if routes.is_empty() {
            continue;
        }
        if let Err(e) = merge_specs(&mut spec, &"", &module_spec) {
            tracing::error!(module = manifest.key, "failed to merge OpenAPI spec: {e}");
        }
        tracing::info!(
            module = manifest.key,
            routes = routes.len(),
            "mounting module"
        );
        app = app.mount("/", routes);
    }

    // Top-level API metadata (set after merging so module fragments don't clobber it).
    spec.info = Info {
        title: "Acre Nexus API".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        description: Some(
            "Multi-tenant property-management & investment platform API.\n\n\
             Auth: humans send a JWT (`Authorization: Bearer <access_token>` from \
             POST /auth/login); vendors send a scoped API key (`acre_live_…`). \
             Tenant context comes from the JWT, the `X-Tenant` header (staff / \
             public site), or the API token. See docs/API.md and docs/MODULES.md."
                .to_owned(),
        ),
        ..Default::default()
    };

    // Serve the spec + interactive explorers.
    let settings = OpenApiSettings::new();
    app = app.mount("/", vec![rocket_okapi::get_openapi_route(spec, &settings)]);
    app = app.mount(
        "/swagger-ui/",
        make_swagger_ui(&SwaggerUIConfig {
            url: "/openapi.json".to_owned(),
            ..Default::default()
        }),
    );
    app = app.mount(
        "/rapidoc/",
        make_rapidoc(&RapiDocConfig {
            general: GeneralConfig {
                spec_urls: vec![rocket_okapi::settings::UrlObject::new(
                    "API",
                    "/openapi.json",
                )],
                ..Default::default()
            },
            hide_show: HideShowConfig {
                allow_spec_url_load: false,
                allow_spec_file_load: false,
                ..Default::default()
            },
            ..Default::default()
        }),
    );

    app.mount("/", routes![cors::preflight])
        .mount(
            "/",
            routes![
                ratelimit::reject_get,
                ratelimit::reject_post,
                ratelimit::reject_put,
                ratelimit::reject_patch,
                ratelimit::reject_delete,
            ],
        )
        .mount(
            "/",
            routes![observability::metrics, observability::readiness],
        )
}
