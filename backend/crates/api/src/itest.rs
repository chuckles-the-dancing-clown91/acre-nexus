//! End-to-end integration tests exercised through **real HTTP requests** against
//! a test Postgres — the first route-level coverage of the guard stack.
//!
//! * **#26** — auth happy path (login → refresh → logout) and RBAC: every
//!   permission-gated route returns `401` without a token, `403` with a token
//!   that lacks the permission, and `200` with it; plus the vendor API-key
//!   (`ApiPrincipal`) scope check. A PR that drops a `user.require(…)` /
//!   `principal.require(…)` check flips a `403` to `200` and fails here.
//! * **#27** — cross-tenant isolation: a tenant-A token can never read another
//!   tenant's rows (even by guessing an id), the `X-Tenant` header can't move a
//!   tenant-bound (non-staff) user across tenants, and Postgres RLS is shown to
//!   actually *bite* for a non-superuser role — not merely assumed.
//!
//! These run only when `TEST_DATABASE_URL` points at a disposable Postgres. When
//! it is unset (a contributor's plain `cargo test`) the suite skips with a note,
//! so `cargo test` stays green with no database; CI sets it (see
//! `.github/workflows/ci.yml`) so the coverage runs on every push.
//!
//! Everything runs as a **single** `#[rocket::async_test]`: each async test gets
//! its own Tokio runtime, so a DB pool / HTTP client built once and shared across
//! separate test fns would outlive the runtime that created it. One test = one
//! runtime = one migrate/seed, with the scenarios run in sequence.

use crate::config::Config;
use crate::state::AppState;
use entity::prelude::{Property, Tenant};
use migration::{Migrator, MigratorTrait};
use rocket::http::{ContentType, Header, Status};
use rocket::local::asynchronous::Client;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, Database, DatabaseConnection,
    DatabaseTransaction, DbBackend, EntityTrait, QueryFilter, Set, Statement, TransactionTrait,
    Value,
};
use serde::Deserialize;
use std::collections::HashSet;
use uuid::Uuid;

/// A migrated + seeded database and the real app in front of it.
struct Ctx {
    client: Client,
    db: DatabaseConnection,
    config: Config,
    db_url: String,
}

/// Connect, migrate, seed, and stand up the real app. Returns `None` (→ skip)
/// when no test database is configured.
async fn setup() -> Option<Ctx> {
    let db_url = std::env::var("TEST_DATABASE_URL").ok()?;
    // Keep the rate limiter out of the way of tightly-packed test requests.
    std::env::set_var("RATE_LIMIT_ENABLED", "false");

    let db = Database::connect(&db_url)
        .await
        .expect("connect to TEST_DATABASE_URL");
    Migrator::up(&db, None)
        .await
        .expect("migrate test database");
    crate::seed::run(&db).await.expect("seed test database");

    // Reuse the process-wide config so the JWT secret we mint tokens with matches
    // the one the `AuthUser` guard verifies against.
    let config = Config::global().clone();
    let state = AppState {
        db: db.clone(),
        config: config.clone(),
    };
    let client = Client::tracked(crate::build_rocket(state))
        .await
        .expect("build the test Rocket client");
    Some(Ctx {
        client,
        db,
        config,
        db_url,
    })
}

#[rocket::async_test]
async fn integration_suite() {
    let Some(c) = setup().await else {
        eprintln!("skipping integration tests: TEST_DATABASE_URL not set");
        return;
    };

    // #26 — auth + RBAC.
    login_refresh_logout_happy_path(&c).await;
    login_with_wrong_password_is_unauthorized(&c).await;
    rbac_permission_gates_are_enforced(&c).await;
    vendor_api_key_scope_is_enforced(&c).await;

    // #27 — cross-tenant isolation.
    a_tenant_cannot_reach_another_tenants_rows(&c).await;
    x_tenant_header_cannot_cross_a_non_staff_user(&c).await;
    rls_bites_for_a_non_superuser_role(&c).await;

    // #13 — Beyond-GA vertical modules.
    syndication_end_to_end(&c).await;
    hoa_end_to_end(&c).await;

    // #28 — background job queue retry/backoff/terminal-failure contract.
    bg_job_queue_contract(&c).await;
}

/// #28 — the durable job queue's retry/backoff/terminal-failure contract, driven
/// one deterministic tick at a time (see `modules::test_jobs`).
async fn bg_job_queue_contract(c: &Ctx) {
    use crate::scheduler::{enqueue_with_retries, run_due_jobs};
    let nw = tenant_id(c, "northwind").await;

    async fn job(c: &Ctx, id: Uuid) -> entity::background_job::Model {
        entity::prelude::BackgroundJob::find_by_id(id)
            .one(&c.db)
            .await
            .unwrap()
            .unwrap()
    }
    // Tick until the job reaches a terminal state (or we give up).
    async fn drain(c: &Ctx, id: Uuid) -> entity::background_job::Model {
        for _ in 0..8 {
            let j = job(c, id).await;
            if j.status == "failed" || j.status == "completed" {
                return j;
            }
            run_due_jobs(&c.db).await.unwrap();
        }
        job(c, id).await
    }

    // (1) A job that always fails exhausts its budget → terminal `failed`, with
    //     last_error populated and attempts capped at max_attempts.
    let id = enqueue_with_retries(
        &c.db,
        nw,
        "test_retry",
        serde_json::json!({"fail_until": 99}),
        0,
        3,
    )
    .await
    .unwrap();
    let j = drain(c, id).await;
    assert_eq!(j.status, "failed", "always-failing job must end failed");
    assert_eq!(j.attempts, 3, "attempts must stop at max_attempts");
    assert!(
        j.last_error.is_some(),
        "a failed job must record last_error"
    );

    // (2) A job that fails twice then succeeds → `completed` after 3 attempts.
    let id = enqueue_with_retries(
        &c.db,
        nw,
        "test_retry",
        serde_json::json!({"fail_until": 2}),
        0,
        5,
    )
    .await
    .unwrap();
    let j = drain(c, id).await;
    assert_eq!(j.status, "completed", "fail-then-succeed job must complete");
    assert_eq!(j.attempts, 3, "two failures + one success = 3 attempts");

    // (3) Backoff defers the retry: a long retry delay pushes run_at into the
    //     future so the queue doesn't busy-loop.
    let id = enqueue_with_retries(
        &c.db,
        nw,
        "test_retry",
        serde_json::json!({"fail_until": 99, "retry_delay_secs": 3600}),
        0,
        5,
    )
    .await
    .unwrap();
    run_due_jobs(&c.db).await.unwrap();
    let after1 = job(c, id).await;
    assert_eq!(after1.attempts, 1);
    assert_eq!(after1.status, "pending");
    let deadline = (chrono::Utc::now() + chrono::Duration::minutes(30)).timestamp();
    assert!(
        after1.run_at.timestamp() > deadline,
        "a retry must be deferred by its backoff"
    );

    // (4) A subsequent tick does NOT reprocess the not-yet-due job — jobs are
    //     durable and processed once per due window (no double-processing).
    run_due_jobs(&c.db).await.unwrap();
    let after2 = job(c, id).await;
    assert_eq!(
        after2.attempts, 1,
        "a deferred job must not be re-run before run_at (no double-processing)"
    );
}

// ---- shared request helpers for the module scenarios ----

/// POST a JSON body and return the status + parsed JSON response.
async fn post_json(
    c: &Ctx,
    path: &str,
    token: &str,
    body: serde_json::Value,
) -> (Status, serde_json::Value) {
    let resp = c
        .client
        .post(path)
        .header(bearer(token))
        .header(ContentType::JSON)
        .body(body.to_string())
        .dispatch()
        .await;
    let status = resp.status();
    let val = resp
        .into_json::<serde_json::Value>()
        .await
        .unwrap_or(serde_json::Value::Null);
    (status, val)
}

/// POST with no body (for action endpoints), returning the status.
async fn post_empty(c: &Ctx, path: &str, token: &str) -> Status {
    c.client
        .post(path)
        .header(bearer(token))
        .dispatch()
        .await
        .status()
}

// ---- helpers -------------------------------------------------------------

#[derive(Deserialize)]
struct Tokens {
    access_token: String,
    refresh_token: String,
}

#[derive(Deserialize)]
struct IdRow {
    id: Uuid,
}

fn bearer(token: &str) -> Header<'static> {
    Header::new("Authorization", format!("Bearer {token}"))
}

/// Mint a JWT for a synthetic principal with an exact permission set — lets a
/// test assert the guard, independent of which seeded user holds what.
fn mint(c: &Ctx, tenant: Option<Uuid>, staff: bool, perms: &[&str]) -> String {
    crate::auth::issue_access_token(
        &c.config,
        Uuid::new_v4(),
        tenant,
        staff,
        perms.iter().map(|s| s.to_string()).collect(),
    )
    .expect("mint access token")
}

async fn tenant_id(c: &Ctx, slug: &str) -> Uuid {
    Tenant::find()
        .filter(entity::tenant::Column::Slug.eq(slug))
        .one(&c.db)
        .await
        .unwrap()
        .unwrap_or_else(|| panic!("seed tenant `{slug}` is missing"))
        .id
}

async fn property_ids(c: &Ctx, tenant: Uuid) -> Vec<Uuid> {
    Property::find()
        .filter(entity::property::Column::TenantId.eq(tenant))
        .all(&c.db)
        .await
        .unwrap()
        .into_iter()
        .map(|p| p.id)
        .collect()
}

/// Insert a vendor API token with the given scopes; returns the raw key.
async fn insert_api_token(c: &Ctx, tenant: Uuid, scopes: &[&str]) -> String {
    let raw = format!("acre_live_{}", crate::auth::random_secret(16));
    entity::api_token::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant),
        name: Set("itest".into()),
        prefix: Set("acre_live_itest".into()),
        token_hash: Set(crate::auth::hash_secret(&raw)),
        scopes: Set(serde_json::json!(scopes)),
        last_used_at: Set(None),
        expires_at: Set(None),
        revoked_at: Set(None),
        created_at: Set(chrono::Utc::now().into()),
    }
    .insert(&c.db)
    .await
    .expect("insert api token");
    raw
}

/// Rewrite a Postgres URL's user-info, e.g. to reconnect under a different role.
fn with_user(url: &str, user: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    let (authority, tail) = match rest.split_once('/') {
        Some((a, t)) => (a, Some(t)),
        None => (rest, None),
    };
    let hostport = authority.rsplit_once('@').map_or(authority, |(_, hp)| hp);
    let mut out = format!("{scheme}://{user}@{hostport}");
    if let Some(t) = tail {
        out.push('/');
        out.push_str(t);
    }
    Some(out)
}

async fn set_tenant(txn: &DatabaseTransaction, tenant: Uuid) {
    txn.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT set_config('app.tenant_id', $1, true)",
        [Value::from(tenant.to_string())],
    ))
    .await
    .unwrap();
}

async fn count(txn: &DatabaseTransaction, sql: &str) -> i64 {
    txn.query_one(Statement::from_string(DbBackend::Postgres, sql))
        .await
        .unwrap()
        .unwrap()
        .try_get_by_index::<i64>(0)
        .unwrap()
}

// ---- #26: auth + RBAC ----------------------------------------------------

async fn login_refresh_logout_happy_path(c: &Ctx) {
    let resp = c
        .client
        .post("/auth/login")
        .header(ContentType::JSON)
        .body(r#"{"email":"jordan@northwind.com","password":"password"}"#)
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok, "seeded login should succeed");
    let toks: Tokens = resp.into_json().await.expect("login token body");

    // Refresh rotates to a fresh refresh token.
    let resp = c
        .client
        .post("/auth/refresh")
        .header(ContentType::JSON)
        .body(format!(r#"{{"refresh_token":"{}"}}"#, toks.refresh_token))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok, "refresh should succeed");
    let toks2: Tokens = resp.into_json().await.expect("refresh token body");
    assert_ne!(
        toks2.refresh_token, toks.refresh_token,
        "refresh must rotate the token"
    );

    // Logout revokes the presented refresh token.
    let resp = c
        .client
        .post("/auth/logout")
        .header(bearer(&toks2.access_token))
        .header(ContentType::JSON)
        .body(format!(r#"{{"refresh_token":"{}"}}"#, toks2.refresh_token))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok, "logout should succeed");

    // The revoked refresh token can no longer mint tokens.
    let resp = c
        .client
        .post("/auth/refresh")
        .header(ContentType::JSON)
        .body(format!(r#"{{"refresh_token":"{}"}}"#, toks2.refresh_token))
        .dispatch()
        .await;
    assert_eq!(
        resp.status(),
        Status::Unauthorized,
        "a revoked refresh token must be rejected"
    );
}

async fn login_with_wrong_password_is_unauthorized(c: &Ctx) {
    let resp = c
        .client
        .post("/auth/login")
        .header(ContentType::JSON)
        .body(r#"{"email":"jordan@northwind.com","password":"not-the-password"}"#)
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Unauthorized);
}

async fn rbac_permission_gates_are_enforced(c: &Ctx) {
    let nw = tenant_id(c, "northwind").await;

    // Representative permission-gated GET route per module: (path, permission).
    let cases = [
        ("/properties", "property:read"),
        ("/listings", "listing:read"),
        ("/applications", "application:read"),
        ("/entities", "entity:read"),
        ("/reminders", "calendar:read"),
    ];

    for (path, perm) in cases {
        // No credentials → 401.
        let r = c.client.get(path).dispatch().await;
        assert_eq!(
            r.status(),
            Status::Unauthorized,
            "{path} without a token must be 401"
        );

        // Authenticated + tenant-scoped, but missing the permission → 403.
        let no_perm = mint(c, Some(nw), false, &[]);
        let r = c.client.get(path).header(bearer(&no_perm)).dispatch().await;
        assert_eq!(
            r.status(),
            Status::Forbidden,
            "{path} without `{perm}` must be 403"
        );

        // With exactly the required permission the gate opens.
        let with_perm = mint(c, Some(nw), false, &[perm]);
        let r = c
            .client
            .get(path)
            .header(bearer(&with_perm))
            .dispatch()
            .await;
        assert_eq!(r.status(), Status::Ok, "{path} with `{perm}` should be 200");
    }
}

async fn vendor_api_key_scope_is_enforced(c: &Ctx) {
    let nw = tenant_id(c, "northwind").await;

    // No key → 401.
    let r = c.client.get("/api/v1/properties").dispatch().await;
    assert_eq!(r.status(), Status::Unauthorized, "vendor route needs a key");

    // Key lacking `property:read` → 403.
    let weak = insert_api_token(c, nw, &["listing:read"]).await;
    let r = c
        .client
        .get("/api/v1/properties")
        .header(Header::new("X-Api-Key", weak))
        .dispatch()
        .await;
    assert_eq!(
        r.status(),
        Status::Forbidden,
        "a key without the scope must be 403"
    );

    // Key with the scope → 200.
    let ok = insert_api_token(c, nw, &["property:read"]).await;
    let r = c
        .client
        .get("/api/v1/properties")
        .header(Header::new("X-Api-Key", ok))
        .dispatch()
        .await;
    assert_eq!(r.status(), Status::Ok, "a scoped key should be 200");
}

// ---- #27: cross-tenant isolation ----------------------------------------

async fn a_tenant_cannot_reach_another_tenants_rows(c: &Ctx) {
    let nw = tenant_id(c, "northwind").await;
    let cs = tenant_id(c, "cascade").await;
    let nw_props = property_ids(c, nw).await;
    let cs_props = property_ids(c, cs).await;
    assert!(
        !nw_props.is_empty() && !cs_props.is_empty(),
        "fixture: both tenants need at least one property"
    );

    let token = mint(c, Some(nw), false, &["property:read"]);
    let victim = cs_props[0];

    // Fetch another tenant's property by id → handler's tenant filter yields 404.
    let path = format!("/properties/{victim}");
    let r = c
        .client
        .get(path.as_str())
        .header(bearer(&token))
        .dispatch()
        .await;
    assert_eq!(
        r.status(),
        Status::NotFound,
        "cross-tenant fetch-by-id must not resolve"
    );

    // Own property is reachable (sanity — the filter isn't blanket-denying).
    let own = format!("/properties/{}", nw_props[0]);
    let r = c
        .client
        .get(own.as_str())
        .header(bearer(&token))
        .dispatch()
        .await;
    assert_eq!(r.status(), Status::Ok);

    // The list route returns only this tenant's rows.
    let r = c
        .client
        .get("/properties")
        .header(bearer(&token))
        .dispatch()
        .await;
    assert_eq!(r.status(), Status::Ok);
    let listed: HashSet<Uuid> = r
        .into_json::<Vec<IdRow>>()
        .await
        .unwrap()
        .into_iter()
        .map(|x| x.id)
        .collect();
    let nw_set: HashSet<Uuid> = nw_props.iter().copied().collect();
    assert_eq!(listed, nw_set, "list must be exactly the tenant's own rows");
    assert!(
        !listed.contains(&victim),
        "another tenant's row leaked into the list"
    );
}

async fn x_tenant_header_cannot_cross_a_non_staff_user(c: &Ctx) {
    let nw = tenant_id(c, "northwind").await;
    let cs = tenant_id(c, "cascade").await;
    let nw_set: HashSet<Uuid> = property_ids(c, nw).await.into_iter().collect();
    let cs_props = property_ids(c, cs).await;

    // A non-staff, tenant-bound token tries to borrow another tenant's context.
    let token = mint(c, Some(nw), false, &["property:read"]);
    let r = c
        .client
        .get("/properties")
        .header(bearer(&token))
        .header(Header::new("X-Tenant", "cascade"))
        .dispatch()
        .await;
    assert_eq!(r.status(), Status::Ok);
    let listed: HashSet<Uuid> = r
        .into_json::<Vec<IdRow>>()
        .await
        .unwrap()
        .into_iter()
        .map(|x| x.id)
        .collect();
    // Still scoped to the token's own tenant — the header is ignored.
    assert_eq!(
        listed, nw_set,
        "X-Tenant must not re-scope a tenant-bound token"
    );
    assert!(
        cs_props.iter().all(|id| !listed.contains(id)),
        "X-Tenant let a non-staff user cross into another tenant"
    );
}

async fn rls_bites_for_a_non_superuser_role(c: &Ctx) {
    let nw = tenant_id(c, "northwind").await;
    let cs = tenant_id(c, "cascade").await;
    let nw_ids = property_ids(c, nw).await;
    let nw_count = nw_ids.len() as i64;
    let nw_prop = nw_ids[0];
    let cs_prop = property_ids(c, cs).await[0];
    let total = Property::find().all(&c.db).await.unwrap().len() as i64;
    assert!(
        total > nw_count,
        "need more than one tenant's rows for a meaningful RLS test"
    );

    // Superusers (and BYPASSRLS roles) skip RLS, so the second wall can only be
    // *proven* by a role that is genuinely subject to the policy. Provision one.
    c.db.execute_unprepared(
        "DO $$ BEGIN \
             IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'acre_rls_probe') THEN \
                 CREATE ROLE acre_rls_probe LOGIN NOSUPERUSER; \
             END IF; \
         END $$; \
         GRANT USAGE ON SCHEMA public TO acre_rls_probe; \
         GRANT SELECT, UPDATE ON property TO acre_rls_probe;",
    )
    .await
    .expect("provision the non-superuser probe role");

    let probe_url = with_user(&c.db_url, "acre_rls_probe").expect("rewrite db url for probe");
    // Pin to a single physical connection so every transaction below reuses it —
    // that's what surfaces the `SET LOCAL` → `''` GUC-reset quirk in step (3),
    // making this a real regression test for the pooled-connection platform plane.
    let mut probe_opt = sea_orm::ConnectOptions::new(probe_url);
    probe_opt.max_connections(1).min_connections(1);
    let probe = Database::connect(probe_opt)
        .await
        .expect("connect as the probe role");

    // (1) Under tenant A's context only tenant A's rows are visible.
    let txn = probe.begin().await.unwrap();
    set_tenant(&txn, nw).await;
    assert_eq!(
        count(&txn, "SELECT count(*) FROM property").await,
        nw_count,
        "RLS should hide other tenants' rows from a scoped session"
    );
    assert_eq!(
        count(
            &txn,
            &format!("SELECT count(*) FROM property WHERE id = '{cs_prop}'")
        )
        .await,
        0,
        "tenant A must not see tenant B's row even by id"
    );
    txn.rollback().await.unwrap();

    // (2) WITH CHECK blocks re-homing a visible row into another tenant.
    let txn = probe.begin().await.unwrap();
    set_tenant(&txn, nw).await;
    let bad = txn
        .execute(Statement::from_string(
            DbBackend::Postgres,
            format!("UPDATE property SET tenant_id = '{cs}' WHERE id = '{nw_prop}'"),
        ))
        .await;
    assert!(
        bad.is_err(),
        "WITH CHECK must reject moving a row into another tenant"
    );
    let _ = txn.rollback().await;

    // (3) With no tenant context (the platform plane) every row is visible —
    //     proving the policy keys on `app.tenant_id`, not a blanket deny.
    let txn = probe.begin().await.unwrap();
    assert_eq!(
        count(&txn, "SELECT count(*) FROM property").await,
        total,
        "an unset tenant context is the intentional cross-tenant plane"
    );
    txn.rollback().await.unwrap();
}

// ---- #13: Beyond-GA vertical modules ----

/// Insert a fresh legal entity (LLC) for a tenant so a scenario is isolated from
/// seed data and re-runs.
async fn create_llc(c: &Ctx, tenant: Uuid) -> Uuid {
    let id = Uuid::new_v4();
    entity::llc::ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant),
        name: Set("Syndication Test Fund LLC".into()),
        ein: Set("00-0000000".into()),
        state: Set("DE".into()),
        entity_type: Set("llc".into()),
        registered_agent: Set(None),
        status: Set("active".into()),
        created_at: Set(chrono::Utc::now().into()),
    }
    .insert(&c.db)
    .await
    .expect("insert test llc");
    id
}

/// Turn a per-tenant module on (for the default-off `hoa` module).
async fn enable_module(c: &Ctx, tenant: Uuid, key: &str) {
    entity::tenant_module::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant),
        module_key: Set(key.into()),
        enabled: Set(true),
        updated_at: Set(chrono::Utc::now().into()),
    }
    .insert(&c.db)
    .await
    .expect("enable module");
}

fn sum_field(lines: &serde_json::Value, field: &str) -> i64 {
    lines
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l[field].as_i64().unwrap())
        .sum()
}

async fn syndication_end_to_end(c: &Ctx) {
    let nw = tenant_id(c, "northwind").await;
    let llc = create_llc(c, nw).await;
    let manage = mint(c, Some(nw), false, &["investor:read", "investor:manage"]);
    let commitments = format!("/entities/{llc}/commitments");

    // RBAC: no permission → 403; no token → 401.
    let noperm = mint(c, Some(nw), false, &[]);
    let r = c
        .client
        .get(commitments.as_str())
        .header(bearer(&noperm))
        .dispatch()
        .await;
    assert_eq!(
        r.status(),
        Status::Forbidden,
        "commitments without investor:read must be 403"
    );
    let r = c.client.get(commitments.as_str()).dispatch().await;
    assert_eq!(r.status(), Status::Unauthorized);

    // A GP (manager) and an LP (investor), committing 1:3.
    let (st, _) = post_json(
        c,
        &commitments,
        &manage,
        serde_json::json!({"owner_name":"GP Sponsor","role":"manager","committed_cents":1_000_000}),
    )
    .await;
    assert_eq!(st, Status::Ok, "add GP commitment");
    let (st, _) = post_json(
        c,
        &commitments,
        &manage,
        serde_json::json!({"owner_name":"LP Investor","role":"investor","committed_cents":3_000_000}),
    )
    .await;
    assert_eq!(st, Status::Ok, "add LP commitment");

    // Call 4,000,000 (split 1:3) and fund it → contributed capital = committed.
    let (st, call) = post_json(
        c,
        &format!("/entities/{llc}/capital-calls"),
        &manage,
        serde_json::json!({"amount_cents":4_000_000}),
    )
    .await;
    assert_eq!(st, Status::Ok, "issue capital call");
    let call_id = call["id"].as_str().unwrap();
    assert_eq!(
        sum_field(&call["lines"], "amount_cents"),
        4_000_000,
        "call lines sum to the called amount"
    );
    assert_eq!(
        post_empty(c, &format!("/capital-calls/{call_id}/fund"), &manage).await,
        Status::Ok,
        "fund the capital call"
    );

    let r = c
        .client
        .get(commitments.as_str())
        .header(bearer(&manage))
        .dispatch()
        .await;
    assert_eq!(r.status(), Status::Ok);
    let stack: serde_json::Value = r.into_json().await.unwrap();
    assert_eq!(
        stack["total_contributed_cents"].as_i64().unwrap(),
        4_000_000,
        "funding credited contributed capital"
    );

    // Distribution 1: 4,000,000 → all return of capital, no carry.
    let (st, d1) = post_json(
        c,
        &format!("/entities/{llc}/distributions"),
        &manage,
        serde_json::json!({"amount_cents":4_000_000,"carry_bps":2000}),
    )
    .await;
    assert_eq!(st, Status::Ok, "post first distribution");
    assert_eq!(
        sum_field(&d1["lines"], "total_cents"),
        4_000_000,
        "distribution conserves the amount"
    );
    assert_eq!(
        sum_field(&d1["lines"], "return_of_capital_cents"),
        4_000_000,
        "capital is returned before any profit"
    );

    // Distribution 2: 1,000,000 pure profit → 20% carry to the GP = 200,000.
    let (st, d2) = post_json(
        c,
        &format!("/entities/{llc}/distributions"),
        &manage,
        serde_json::json!({"amount_cents":1_000_000,"carry_bps":2000}),
    )
    .await;
    assert_eq!(st, Status::Ok, "post second distribution");
    assert_eq!(
        sum_field(&d2["lines"], "carry_cents"),
        200_000,
        "GP carry = 20% of the 1,000,000 profit tier"
    );
    assert_eq!(
        sum_field(&d2["lines"], "total_cents"),
        1_000_000,
        "distribution conserves the amount"
    );
}

async fn hoa_end_to_end(c: &Ctx) {
    let nw = tenant_id(c, "northwind").await;
    enable_module(c, nw, "hoa").await;
    let manage = mint(c, Some(nw), false, &["hoa:read", "hoa:manage"]);

    // RBAC: no permission → 403; no token → 401.
    let noperm = mint(c, Some(nw), false, &[]);
    let r = c
        .client
        .get("/hoa/associations")
        .header(bearer(&noperm))
        .dispatch()
        .await;
    assert_eq!(
        r.status(),
        Status::Forbidden,
        "hoa without hoa:read must be 403"
    );
    let r = c.client.get("/hoa/associations").dispatch().await;
    assert_eq!(r.status(), Status::Unauthorized);

    // Association with monthly dues + two members.
    let (st, assoc) = post_json(
        c,
        "/hoa/associations",
        &manage,
        serde_json::json!({"name":"Maple Grove HOA","dues_cents":25_000,"dues_frequency":"monthly"}),
    )
    .await;
    assert_eq!(st, Status::Ok, "create association");
    let aid = assoc["id"].as_str().unwrap().to_string();

    let (st, m1) = post_json(
        c,
        &format!("/hoa/associations/{aid}/members"),
        &manage,
        serde_json::json!({"name":"Alice","unit_label":"1A"}),
    )
    .await;
    assert_eq!(st, Status::Ok, "add member 1");
    let member1 = m1["id"].as_str().unwrap().to_string();
    let (st, _) = post_json(
        c,
        &format!("/hoa/associations/{aid}/members"),
        &manage,
        serde_json::json!({"name":"Bob","unit_label":"1B"}),
    )
    .await;
    assert_eq!(st, Status::Ok, "add member 2");

    // Bill all active members (no member_id) → two assessments of 25,000.
    let (st, assessments) = post_json(
        c,
        &format!("/hoa/associations/{aid}/assessments"),
        &manage,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Ok, "assess dues to all members");
    let arr = assessments.as_array().unwrap();
    assert_eq!(
        arr.len(),
        2,
        "assessing with no member_id bills every member"
    );
    assert!(arr
        .iter()
        .all(|a| a["amount_cents"].as_i64().unwrap() == 25_000));

    // Violation → fine.
    let (st, v) = post_json(
        c,
        &format!("/hoa/associations/{aid}/violations"),
        &manage,
        serde_json::json!({"member_id":member1,"kind":"landscaping","description":"overgrown"}),
    )
    .await;
    assert_eq!(st, Status::Ok, "log violation");
    let vid = v["id"].as_str().unwrap().to_string();
    let vpath = format!("/hoa/violations/{vid}");
    let resp = c
        .client
        .patch(vpath.as_str())
        .header(bearer(&manage))
        .header(ContentType::JSON)
        .body(serde_json::json!({"status":"fined","fine_cents":5_000}).to_string())
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok, "fine the violation");
    let vv: serde_json::Value = resp.into_json().await.unwrap();
    assert_eq!(vv["status"].as_str().unwrap(), "fined");
    assert_eq!(vv["fine_cents"].as_i64().unwrap(), 5_000);

    // ARC request → approve.
    let (st, arc) = post_json(
        c,
        &format!("/hoa/associations/{aid}/arc-requests"),
        &manage,
        serde_json::json!({"member_id":member1,"title":"New fence"}),
    )
    .await;
    assert_eq!(st, Status::Ok, "submit ARC request");
    let arc_id = arc["id"].as_str().unwrap().to_string();
    let (st, decided) = post_json(
        c,
        &format!("/hoa/arc-requests/{arc_id}/decide"),
        &manage,
        serde_json::json!({"decision":"approved","note":"looks good"}),
    )
    .await;
    assert_eq!(st, Status::Ok, "decide ARC request");
    assert_eq!(decided["status"].as_str().unwrap(), "approved");
}
