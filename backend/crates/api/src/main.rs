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

mod audit;
mod auth;
mod config;
mod cors;
mod documents;
mod dto;
mod email;
mod enrichment;
mod error;
mod modules;
mod openapi;
mod pdf;
mod pii;
mod rbac;
mod routes;
mod scheduler;
mod seed;
mod state;
mod storage;
mod templating;
mod tenancy;
mod tokens;
mod workflow;

use config::Config;
use migration::{ClientMigrator, MigratorTrait, PropertyMigrator, UserMigrator};
use rocket::data::ToByteUnit;
use rocket_okapi::okapi::merge::merge_specs;
use rocket_okapi::okapi::openapi3::{Info, OpenApi};
use rocket_okapi::rapidoc::{make_rapidoc, GeneralConfig, HideShowConfig, RapiDocConfig};
use rocket_okapi::settings::OpenApiSettings;
use rocket_okapi::swagger_ui::{make_swagger_ui, SwaggerUIConfig};
use sea_orm::Database;
use state::AppState;

#[launch]
async fn rocket() -> _ {
    // Structured logging.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,sqlx=warn".into()),
        )
        .try_init();

    let config = Config::from_env();

    // Migrations run as the schema-owner role (DDL); the runtime connections
    // below use the least-privilege `_app` role so RLS policies bite.
    if config.auto_migrate {
        tracing::info!("running migrations…");
        migrate::<UserMigrator>("acre_user", &config.user_owner_url).await;
        migrate::<PropertyMigrator>("acre_property", &config.property_owner_url).await;
        migrate::<ClientMigrator>("acre_client", &config.client_owner_url).await;
    }

    let user_db = connect("acre_user", &config.user_db_url).await;
    let property_db = connect("acre_property", &config.property_db_url).await;
    let client_db = connect("acre_client", &config.client_db_url).await;

    if config.auto_migrate {
        seed::run(&user_db, &property_db, &client_db)
            .await
            .expect("seed failed");
    }

    // Spawn the Tokio background scheduler over all three databases (it polls
    // background_job in acre_user and dispatches to handlers that may touch any).
    scheduler::spawn(scheduler::Pools {
        user: user_db.clone(),
        property: property_db.clone(),
        client: client_db.clone(),
    });

    let state = AppState {
        user_db,
        property_db,
        client_db,
        config,
    };

    // Accumulate the merged OpenAPI document as we mount routes. Core routes
    // first, then every pluggable module's routes — each module contributes both
    // its routes and a matching spec fragment.
    let mut spec = OpenApi::new();
    // Raise body limits so document/logo uploads (multipart) are accepted.
    let figment = rocket::Config::figment()
        .merge(("cli_colors", false))
        .merge((
            "limits",
            rocket::data::Limits::default()
                .limit("file", 25.mebibytes())
                .limit("data-form", 30.mebibytes())
                .limit("bytes", 30.mebibytes()),
        ));
    let mut app = rocket::custom(figment)
        .manage(state)
        .attach(cors::Cors)
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
}

/// Open a runtime connection pool to a domain database (as the `_app` role).
async fn connect(name: &str, url: &str) -> sea_orm::DatabaseConnection {
    tracing::info!("connecting to {name}…");
    Database::connect(url)
        .await
        .unwrap_or_else(|e| panic!("failed to connect to {name}: {e}"))
}

/// Connect to a domain database as its owner role and apply all pending
/// migrations for that domain.
async fn migrate<M: MigratorTrait>(name: &str, url: &str) {
    let db = Database::connect(url)
        .await
        .unwrap_or_else(|e| panic!("failed to connect to {name} (owner): {e}"));
    M::up(&db, None)
        .await
        .unwrap_or_else(|e| panic!("migration for {name} failed: {e}"));
}
