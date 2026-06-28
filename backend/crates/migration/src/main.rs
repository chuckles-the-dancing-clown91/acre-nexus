//! CLI to apply migrations to all three Acre databases.
//!
//! Usage: `cargo run -p migration -- [up|down|status|fresh|refresh|reset]`
//! (default: `up`). Each domain connects with its **owner** URL
//! (`<DOMAIN>_DATABASE_OWNER_URL`, falling back to `<DOMAIN>_DATABASE_URL`, then
//! `DATABASE_URL`) since migrations require DDL privileges.

use migration::{ClientMigrator, MigratorTrait, PropertyMigrator, UserMigrator};
use sea_orm::Database;

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();
    let cmd = std::env::args().nth(1).unwrap_or_else(|| "up".to_string());

    run::<UserMigrator>("user", &owner_url("USER"), &cmd).await;
    run::<PropertyMigrator>("property", &owner_url("PROPERTY"), &cmd).await;
    run::<ClientMigrator>("client", &owner_url("CLIENT"), &cmd).await;
}

/// Resolve the owner (DDL) connection URL for a domain.
fn owner_url(domain: &str) -> String {
    std::env::var(format!("{domain}_DATABASE_OWNER_URL"))
        .or_else(|_| std::env::var(format!("{domain}_DATABASE_URL")))
        .or_else(|_| std::env::var("DATABASE_URL"))
        .unwrap_or_else(|_| format!("postgres://localhost:5432/acre_{}", domain.to_lowercase()))
}

async fn run<M: MigratorTrait>(name: &str, url: &str, cmd: &str) {
    let db = Database::connect(url)
        .await
        .unwrap_or_else(|e| panic!("[{name}] connect failed: {e}"));
    let res = match cmd {
        "up" => M::up(&db, None).await,
        "down" => M::down(&db, Some(1)).await,
        "status" => M::status(&db).await,
        "fresh" => M::fresh(&db).await,
        "refresh" => M::refresh(&db).await,
        "reset" => M::reset(&db).await,
        other => {
            eprintln!("[{name}] unknown command '{other}' (use up|down|status|fresh|refresh|reset)");
            return;
        }
    };
    res.unwrap_or_else(|e| panic!("[{name}] migration '{cmd}' failed: {e}"));
    println!("[{name}] {cmd} ✓");
}
