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
//!
//! ## Boot sequence
//! Connect to Postgres → (optionally) migrate + seed → spawn the scheduler →
//! launch Rocket with all routes mounted at `/`.

#[macro_use]
extern crate rocket;

mod auth;
mod config;
mod cors;
mod dto;
mod error;
mod modules;
mod rbac;
mod routes;
mod scheduler;
mod seed;
mod state;
mod tenancy;
mod tokens;

use config::Config;
use migration::{Migrator, MigratorTrait};
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
    tracing::info!("connecting to database…");
    let db = Database::connect(&config.database_url)
        .await
        .expect("failed to connect to database");

    if config.auto_migrate {
        tracing::info!("running migrations…");
        Migrator::up(&db, None).await.expect("migration failed");
        seed::run(&db).await.expect("seed failed");
    }

    // Spawn the Tokio background scheduler.
    scheduler::spawn(db.clone());

    let state = AppState { db, config };

    // Always-on core routes, then every pluggable module's routes. Each module
    // is mounted at the API root; collisions surface loudly at boot.
    let mut app = rocket::build()
        .manage(state)
        .attach(cors::Cors)
        .mount("/", routes::core());

    for module in modules::registry() {
        let manifest = module.manifest();
        let routes = module.routes();
        if !routes.is_empty() {
            tracing::info!(module = manifest.key, routes = routes.len(), "mounting module");
            app = app.mount("/", routes);
        }
    }

    app.mount("/", routes![cors::preflight])
}
