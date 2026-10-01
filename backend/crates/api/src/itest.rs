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
    invite_then_set_password_then_reset(&c).await;
    two_way_texts_and_stop(&c).await;
    back_office_hours_to_owner_bill(&c).await;
    crm_owner_lead_to_owner(&c).await;
    property_autofill_and_photo(&c).await;
    parts_loop_and_closeout(&c).await;
    alpha_vendor_link(&c).await;
    audit_trail_who_changed_what(&c).await;
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

/// The link token inside the most recent queued email for `to` + `template`.
async fn queued_link_token(c: &Ctx, to: &str, template: &str) -> Option<String> {
    use sea_orm::QueryOrder;
    let jobs = entity::prelude::BackgroundJob::find()
        .filter(entity::background_job::Column::Kind.eq("auto_email"))
        .order_by_desc(entity::background_job::Column::CreatedAt)
        .all(&c.db)
        .await
        .unwrap();
    jobs.into_iter()
        .find(|j| {
            j.payload["to"].as_str() == Some(to) && j.payload["template"].as_str() == Some(template)
        })
        .and_then(|j| j.payload["vars"]["link"].as_str().map(str::to_string))
        .and_then(|link| link.split("token=").nth(1).map(str::to_string))
}

async fn login_status(c: &Ctx, email: &str, password: &str) -> Status {
    c.client
        .post("/auth/login")
        .header(ContentType::JSON)
        .body(serde_json::json!({ "email": email, "password": password }).to_string())
        .dispatch()
        .await
        .status()
}

/// Phase 1 of the Vantedge roadmap: an invited member receives a link, chooses a
/// password (activating the account), can sign in, can't reuse the link, and
/// "forgot password" issues a reset that works once — without ever revealing
/// whether an address has an account.
async fn invite_then_set_password_then_reset(c: &Ctx) {
    let nw = tenant_id(c, "northwind").await;
    let admin = mint(c, Some(nw), false, &["member:manage", "member:read"]);
    // Unique per run so a reused test database doesn't already hold the account.
    let email: &'static str =
        Box::leak(format!("resident.{}@example.com", Uuid::new_v4().simple()).into_boxed_str());

    let (st, member) = post_json(
        c,
        "/members",
        &admin,
        serde_json::json!({ "email": email, "name": "Riley Resident", "profile_type": "renter" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "invite should succeed: {member}");
    assert_eq!(member["account_status"], "invited");
    assert_eq!(
        login_status(c, email, "anything at all").await,
        Status::Unauthorized,
        "an invited account has no usable password yet"
    );

    let token = queued_link_token(c, email, "account_invite")
        .await
        .expect("an invite email with a link is queued");
    let resp = c
        .client
        .get(format!("/auth/password/link/{token}"))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    let info: serde_json::Value = resp.into_json().await.unwrap();
    assert_eq!(info["purpose"], "invite");
    assert_eq!(info["email"], email);

    let set = |token: String, password: &'static str| async move {
        c.client
            .post("/auth/password/set")
            .header(ContentType::JSON)
            .body(serde_json::json!({ "token": token, "password": password }).to_string())
            .dispatch()
            .await
            .status()
    };
    assert_eq!(set(token.clone(), "short").await, Status::BadRequest);
    assert_eq!(set(token.clone(), "a quiet lake at dawn").await, Status::Ok);
    assert_eq!(
        set(token.clone(), "another good phrase").await,
        Status::NotFound,
        "a link works once"
    );
    assert_eq!(
        login_status(c, email, "a quiet lake at dawn").await,
        Status::Ok
    );

    // Forgot password: unknown addresses get the same answer and no email.
    let forgot = |email: &'static str| async move {
        c.client
            .post("/auth/password/forgot")
            .header(ContentType::JSON)
            .body(serde_json::json!({ "email": email }).to_string())
            .dispatch()
            .await
            .status()
    };
    assert_eq!(forgot("nobody@nowhere.example").await, Status::Ok);
    assert!(
        queued_link_token(c, "nobody@nowhere.example", "password_reset")
            .await
            .is_none()
    );
    assert_eq!(forgot(email).await, Status::Ok);
    let reset = queued_link_token(c, email, "password_reset")
        .await
        .expect("a reset email is queued");
    assert_eq!(set(reset, "a new quiet lake").await, Status::Ok);
    assert_eq!(
        login_status(c, email, "a quiet lake at dawn").await,
        Status::Unauthorized,
        "the old password stops working"
    );
    assert_eq!(login_status(c, email, "a new quiet lake").await, Status::Ok);
}

/// Phase 2 of the Vantedge roadmap: an inbound text opens a thread, a console
/// reply goes out through the queue and is filed as sent, STOP blocks both
/// console replies and automatic notification texts, START lifts it, and the
/// Twilio webhook refuses an unsigned request.
async fn two_way_texts_and_stop(c: &Ctx) {
    use crate::scheduler::run_due_jobs;
    let nw = tenant_id(c, "northwind").await;
    let staff = mint(c, Some(nw), false, &["message:read", "message:manage"]);
    let phone = format!("+1760555{:04}", Uuid::new_v4().as_u128() % 10_000);

    // A resident texts in (test mode).
    let (st, t) = post_json(
        c,
        "/texts/simulate",
        &staff,
        serde_json::json!({ "phone": phone, "body": "The kitchen sink is leaking" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "simulate inbound: {t}");
    assert_eq!(t["thread"]["unread_count"], 1);
    let thread_id = t["thread"]["id"].as_str().unwrap().to_string();

    // Reply from the console; the queue sends it and marks it sent.
    let (st, t) = post_json(
        c,
        &format!("/texts/{thread_id}/reply"),
        &staff,
        serde_json::json!({ "body": "Sorry! A tech will be there by 3." }),
    )
    .await;
    assert_eq!(st, Status::Ok, "reply: {t}");
    for _ in 0..4 {
        run_due_jobs(&c.db).await.unwrap();
    }
    let resp = c
        .client
        .get(format!("/texts/{thread_id}"))
        .header(bearer(&staff))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    let t: serde_json::Value = resp.into_json().await.unwrap();
    assert_eq!(t["thread"]["unread_count"], 0, "reading marks it read");
    let out = t["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["direction"] == "out")
        .expect("the reply is in the thread");
    assert_eq!(out["status"], "sent");

    // STOP (in Spanish): replies are refused, notification texts are skipped.
    let (st, t) = post_json(
        c,
        "/texts/simulate",
        &staff,
        serde_json::json!({ "phone": phone, "body": "ALTO" }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    assert_eq!(t["thread"]["opted_out"], true);
    let (st, _) = post_json(
        c,
        &format!("/texts/{thread_id}/reply"),
        &staff,
        serde_json::json!({ "body": "Are you sure?" }),
    )
    .await;
    assert_eq!(st, Status::Conflict, "no texts to a number that said STOP");
    let job_id = crate::scheduler::enqueue(
        &c.db,
        nw,
        "auto_sms",
        serde_json::json!({ "template": "test_notification", "to": phone }),
        0,
    )
    .await
    .unwrap();
    for _ in 0..4 {
        run_due_jobs(&c.db).await.unwrap();
    }
    let job = entity::prelude::BackgroundJob::find_by_id(job_id)
        .one(&c.db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(job.status, "completed");
    assert_eq!(
        job.result.as_ref().and_then(|r| r["reason"].as_str()),
        Some("opted_out")
    );

    // START lifts it.
    let (_, t) = post_json(
        c,
        "/texts/simulate",
        &staff,
        serde_json::json!({ "phone": phone, "body": "start" }),
    )
    .await;
    assert_eq!(t["thread"]["opted_out"], false);

    // Reading needs message:read; the webhook needs a real Twilio signature.
    let nobody = mint(c, Some(nw), false, &[]);
    let resp = c
        .client
        .get("/texts")
        .header(bearer(&nobody))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Forbidden);
    let resp = c
        .client
        .post("/webhooks/twilio/sms?tenant=northwind")
        .header(ContentType::Form)
        .body(format!("From={}&Body=hi", phone.replace('+', "%2B")))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Forbidden);
}

async fn get_json(c: &Ctx, path: &str, token: &str) -> (Status, serde_json::Value) {
    let resp = c.client.get(path).header(bearer(token)).dispatch().await;
    let status = resp.status();
    let val = resp
        .into_json::<serde_json::Value>()
        .await
        .unwrap_or(serde_json::Value::Null);
    (status, val)
}

async fn send_json(
    c: &Ctx,
    method: rocket::http::Method,
    path: &str,
    token: &str,
    body: serde_json::Value,
) -> (Status, serde_json::Value) {
    let resp = c
        .client
        .req(method, path)
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

/// Phase 2B: one set of hours drives everything. A technician's 13-hour day on a
/// work order (California rules: 8 regular, 4 at 1.5×, 1 at 2×) is approved,
/// billed to the owner at their bill rate with a marked-up receipt, and costed
/// with its overtime premium and burden — and once billed, the time is locked.
async fn back_office_hours_to_owner_bill(c: &Ctx) {
    use rocket::http::Method;
    let nw = tenant_id(c, "northwind").await;
    for (k, v) in [
        (
            crate::settings::WORKFORCE_OVERTIME_RULE,
            serde_json::json!("california"),
        ),
        (
            crate::settings::WORKFORCE_LABOR_BURDEN_BPS,
            serde_json::json!(1000),
        ),
        (
            crate::settings::WORKFORCE_MAINTENANCE_MARKUP_BPS,
            serde_json::json!(1000),
        ),
    ] {
        crate::settings::set_value(&c.db, nw, k, v).await.unwrap();
    }
    let office = mint(
        c,
        Some(nw),
        false,
        &[
            "team:read",
            "team:manage",
            "payroll:read",
            "expense:read",
            "expense:manage",
            "payable:manage",
        ],
    );
    let morgan = entity::prelude::User::find()
        .filter(entity::user::Column::Email.eq("morgan@northwind.com"))
        .one(&c.db)
        .await
        .unwrap()
        .expect("seeded back-office user");
    let tech = crate::auth::issue_access_token(&c.config, morgan.id, Some(nw), false, vec![])
        .expect("mint");

    // Not on the team yet → no clock.
    let (st, _) = get_json(c, "/me/clock", &tech).await;
    assert_eq!(st, Status::Forbidden);
    let (st, p) = send_json(
        c,
        Method::Put,
        &format!("/team/{}", morgan.id),
        &office,
        serde_json::json!({ "title": "Maintenance tech", "pay_rate_cents": 2500, "bill_rate_cents": 7500 }),
    )
    .await;
    assert_eq!(st, Status::Ok, "profile: {p}");
    assert_eq!(p["overtime_eligible"], true);

    let ticket = entity::prelude::MaintenanceTicket::find()
        .filter(entity::maintenance_ticket::Column::TenantId.eq(nw))
        .filter(entity::maintenance_ticket::Column::Title.eq("Kitchen faucet leaking"))
        .one(&c.db)
        .await
        .unwrap()
        .expect("seeded work order");

    // Clock in and out on the work order; the stub entry is then deleted.
    let target = serde_json::json!({ "kind": "work_order", "maintenance_ticket_id": ticket.id });
    let (st, clock) = post_json(c, "/me/clock/in", &tech, target.clone()).await;
    assert_eq!(st, Status::Ok, "clock in: {clock}");
    assert!(clock["open"].is_object());
    let stub = clock["open"]["id"].as_str().unwrap().to_string();
    let (st, clock) = post_json(c, "/me/clock/out", &tech, serde_json::json!({})).await;
    assert_eq!(st, Status::Ok);
    assert!(clock["open"].is_null());
    let (st, _) = send_json(
        c,
        Method::Delete,
        &format!("/me/time/{stub}"),
        &tech,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Ok);

    // A 13-hour day, logged by hand; an overlapping one is refused.
    let mut day = target.clone();
    day["started_at"] = serde_json::json!("2026-09-15T07:00:00-07:00");
    day["ended_at"] = serde_json::json!("2026-09-15T20:00:00-07:00");
    let (st, e) = post_json(c, "/me/time", &tech, day.clone()).await;
    assert_eq!(st, Status::Ok, "manual entry: {e}");
    assert_eq!(e["minutes"], 780);
    assert!(
        e["pay_rate_cents"].is_null(),
        "no pay figures on self-service"
    );
    let entry_id = e["id"].as_str().unwrap().to_string();
    let mut clash = target.clone();
    clash["started_at"] = serde_json::json!("2026-09-15T19:00:00-07:00");
    clash["ended_at"] = serde_json::json!("2026-09-15T21:00:00-07:00");
    let (st, _) = post_json(c, "/me/time", &tech, clash).await;
    assert_eq!(st, Status::Conflict, "overlapping time is refused");

    // The office approves; the technician can no longer change it.
    let (st, r) = post_json(
        c,
        "/team/time/approve",
        &office,
        serde_json::json!({ "ids": [entry_id] }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    assert_eq!(r["approved"], 1);
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/me/time/{entry_id}"),
        &tech,
        day.clone(),
    )
    .await;
    assert_eq!(st, Status::Conflict);

    // A receipt billable to the owner, and a mileage trip that isn't.
    let (st, x) = post_json(
        c,
        "/me/expenses",
        &tech,
        serde_json::json!({
            "incurred_on": "2026-09-15", "category": "materials", "vendor": "Home Depot",
            "amount_cents": 4000, "billable_to_owner": true,
            "maintenance_ticket_id": ticket.id,
        }),
    )
    .await;
    assert_eq!(st, Status::Ok, "expense: {x}");
    let (st, trip) = post_json(
        c,
        "/me/expenses",
        &tech,
        serde_json::json!({
            "incurred_on": "2026-09-15", "category": "mileage", "miles": 10,
            "vehicle": "personal", "maintenance_ticket_id": ticket.id,
        }),
    )
    .await;
    assert_eq!(st, Status::Ok, "trip: {trip}");
    assert_eq!(trip["amount_cents"], 700, "10 miles at $0.70");

    // The bill preview: 13h at $75, the receipt with 10% markup.
    let (st, pv) = get_json(
        c,
        &format!("/costs/work-orders/{}/bill-preview", ticket.id),
        &office,
    )
    .await;
    assert_eq!(st, Status::Ok, "preview: {pv}");
    let lines = pv["lines"].as_array().unwrap();
    let labor = lines
        .iter()
        .find(|l| l["description"].as_str().unwrap().starts_with("Labor"))
        .expect("a labor line");
    assert_eq!(labor["amount_cents"], 97_500);
    assert!(
        lines.iter().any(|l| l["amount_cents"] == 4_400),
        "receipt + markup: {pv}"
    );

    let (st, bill) = post_json(
        c,
        &format!("/costs/work-orders/{}/bill-owner", ticket.id),
        &office,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Ok, "bill: {bill}");
    assert_eq!(bill["amount_cents"], pv["total_cents"]);
    assert_eq!(bill["status"], "draft");

    // Billed time is locked, and there's nothing left to bill.
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/team/time/{entry_id}"),
        &office,
        day,
    )
    .await;
    assert_eq!(st, Status::Conflict);
    let (st, _) = post_json(
        c,
        &format!("/costs/work-orders/{}/bill-owner", ticket.id),
        &office,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Conflict);

    // Costing: $25 × 13h, premium 4h × $12.50 + 1h × $25 = $75, burden 10%.
    let (st, cost) = get_json(c, &format!("/costs/work-orders/{}", ticket.id), &office).await;
    assert_eq!(st, Status::Ok, "costs: {cost}");
    assert_eq!(cost["minutes"], 780);
    assert_eq!(cost["labor_pay_cents"], 32_500);
    assert_eq!(cost["overtime_premium_cents"], 7_500);
    assert_eq!(cost["burden_cents"], 4_000);
    assert_eq!(cost["mileage_cents"], 700);
    assert_eq!(cost["billed_cents"], bill["amount_cents"]);
    assert_eq!(cost["unbilled_cents"], 0);
    // The owner bill rides normal AP: approving it posts to the LLC's books.
    let bill_id = bill["bill_id"].as_str().unwrap().to_string();
    let approver = mint(
        c,
        Some(nw),
        false,
        &["payable:manage", "payable:approve", "payable:read"],
    );
    assert_eq!(
        post_empty(c, &format!("/payables/{bill_id}/submit"), &approver).await,
        Status::Ok
    );
    assert_eq!(
        post_empty(c, &format!("/payables/{bill_id}/approve"), &approver).await,
        Status::Ok
    );
    let (st, ap) = get_json(c, &format!("/payables/{bill_id}"), &approver).await;
    assert_eq!(st, Status::Ok);
    assert_eq!(ap["status"], "approved");
    assert_eq!(ap["vendor_name"], "In-house maintenance");
    assert!(
        ap["accrual_txn_id"].is_string(),
        "the bill posted to the owner's ledger"
    );

    // Payroll for that week: 8 regular, 4 at 1.5×, 1 at 2× → $400 at $25/h.
    let (st, pr) = get_json(
        c,
        "/reports/payroll?from=2026-09-14&to=2026-09-20&approved_only=true",
        &office,
    )
    .await;
    assert_eq!(st, Status::Ok, "payroll: {pr}");
    let row = pr["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["user_id"] == serde_json::json!(morgan.id))
        .expect("morgan's week");
    assert_eq!(row["regular_minutes"], 480);
    assert_eq!(row["overtime_minutes"], 240);
    assert_eq!(row["double_minutes"], 60);
    assert_eq!(row["gross_cents"], 40_000);
    // Everything prints, and exports to CSV.
    for path in [
        "/reports/payroll/export?from=2026-09-14&to=2026-09-20&format=pdf".to_string(),
        "/reports/profit/export?from=2026-09-01&to=2026-09-30&format=pdf".to_string(),
        "/reports/taxes/export?year=2026&format=pdf".to_string(),
        "/reports/timesheets/export?from=2026-09-14&to=2026-09-20&format=pdf".to_string(),
        format!("/costs/work-orders/{}/sheet.pdf", ticket.id),
    ] {
        let resp = c
            .client
            .get(path.clone())
            .header(bearer(&office))
            .dispatch()
            .await;
        assert_eq!(resp.status(), Status::Ok, "{path}");
        let bytes = resp.into_bytes().await.unwrap();
        assert!(bytes.starts_with(b"%PDF-1.4"), "{path} is a PDF");
    }
    let resp = c
        .client
        .get("/reports/taxes/export?year=2026&format=csv&section=mileage-log")
        .header(bearer(&office))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    let csv = resp.into_string().await.unwrap();
    assert!(csv.starts_with("Date,Driver"), "{csv}");
    assert!(csv.contains("$7.00"));
    // Profit and taxes see the same work and the same receipt.
    let (_, pf) = get_json(c, "/reports/profit?from=2026-09-01&to=2026-09-30", &office).await;
    assert!(pf["work"]
        .as_array()
        .unwrap()
        .iter()
        .any(|w| w["id"] == serde_json::json!(ticket.id)));
    let (_, tx) = get_json(c, "/reports/taxes?year=2026", &office).await;
    assert!(
        tx["missing_receipts"].as_u64().unwrap() >= 1,
        "the Home Depot receipt was never attached"
    );
    assert!(tx["pay_by_person"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["form"] == "W-2"));
    let (st, dash) = get_json(c, "/backoffice/dashboard", &office).await;
    assert_eq!(st, Status::Ok, "dashboard: {dash}");
    assert!(dash["billed_to_owners_this_month_cents"].is_number());

    // The same week goes to Gusto as 8 / 4 / 1 hours (simulated here).
    let (st, g) = get_json(
        c,
        "/payroll/gusto/hours?from=2026-09-14&to=2026-09-20",
        &office,
    )
    .await;
    assert_eq!(st, Status::Ok, "gusto hours: {g}");
    let line = g["lines"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["email"] == "morgan@northwind.com")
        .expect("morgan's hours");
    assert_eq!(line["regular_hours"], "8.000");
    assert_eq!(line["overtime_hours"], "4.000");
    assert_eq!(line["double_overtime_hours"], "1.000");
    let (st, pushed) = post_json(
        c,
        "/payroll/gusto/push",
        &office,
        serde_json::json!({ "payroll_id": "demo-payroll", "from": "2026-09-14", "to": "2026-09-20" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "gusto push: {pushed}");
    assert_eq!(pushed["simulated"], true);

    // Without payroll:read, no margins.
    let viewer = mint(c, Some(nw), false, &["team:read"]);
    let (st, _) = get_json(c, &format!("/costs/work-orders/{}", ticket.id), &viewer).await;
    assert_eq!(st, Status::Forbidden);
}

/// CRM: an owner lead gets a follow-up, moves to proposal (logged on its
/// timeline), prints a management proposal, and converts into an owner who
/// keeps the history.
async fn crm_owner_lead_to_owner(c: &Ctx) {
    use rocket::http::Method;
    let nw = tenant_id(c, "northwind").await;
    let crm = mint(c, Some(nw), false, &["entity:read", "entity:manage"]);
    let (st, lead) = post_json(
        c,
        "/crm/owner-leads",
        &crm,
        serde_json::json!({
            "name": "Dana Ortiz", "company": "Ortiz Family Trust", "doors": 12,
            "properties_count": 3, "monthly_rent_cents": 2_400_000, "fee_bps": 800,
            "source": "referral",
        }),
    )
    .await;
    assert_eq!(st, Status::Ok, "lead: {lead}");
    assert_eq!(lead["monthly_fee_cents"], 192_000, "8% of $24,000");
    let id = lead["id"].as_str().unwrap().to_string();

    let (st, note) = post_json(
        c,
        "/crm/notes",
        &crm,
        serde_json::json!({
            "subject_type": "owner_lead", "subject_id": id, "kind": "call",
            "body": "Wants a quote for the three Hesperia duplexes.", "follow_up_on": "2026-01-01",
        }),
    )
    .await;
    assert_eq!(st, Status::Ok, "note: {note}");
    assert_eq!(note["follow_up_due"], true);
    let (_, due) = get_json(c, "/crm/follow-ups", &crm).await;
    assert!(due
        .as_array()
        .unwrap()
        .iter()
        .any(|n| n["id"] == note["id"]));

    let (st, moved) = send_json(
        c,
        Method::Patch,
        &format!("/crm/owner-leads/{id}"),
        &crm,
        serde_json::json!({ "status": "proposal" }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    assert_eq!(moved["status"], "proposal");
    let resp = c
        .client
        .get(format!("/crm/owner-leads/{id}/proposal.pdf"))
        .header(bearer(&crm))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    assert!(resp.into_bytes().await.unwrap().starts_with(b"%PDF"));
    let (_, summary) = get_json(c, "/crm/owner-leads/summary", &crm).await;
    assert!(
        summary["weighted_monthly_fee_cents"].as_i64().unwrap() >= 96_000,
        "{summary}"
    );

    let (st, won) = post_json(
        c,
        &format!("/crm/owner-leads/{id}/convert"),
        &crm,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Ok, "convert: {won}");
    assert_eq!(won["status"], "won");
    let owner_id = won["owner_id"].as_str().unwrap().to_string();
    let (_, owners) = get_json(c, "/crm/owners", &crm).await;
    assert!(owners
        .as_array()
        .unwrap()
        .iter()
        .any(|o| o["id"] == owner_id && o["name"] == "Ortiz Family Trust"));
    let (_, timeline) = get_json(
        c,
        &format!("/crm/notes?subject_type=owner&subject_id={owner_id}"),
        &crm,
    )
    .await;
    let bodies: Vec<&str> = timeline
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|n| n["body"].as_str())
        .collect();
    assert!(
        bodies.iter().any(|b| b.contains("Hesperia duplexes")),
        "{bodies:?}"
    );
    assert!(
        bodies.iter().any(|b| b.contains("to proposal")),
        "{bodies:?}"
    );
    // A second convert is refused; without entity:manage nothing changes.
    let (st, _) = post_json(
        c,
        &format!("/crm/owner-leads/{id}/convert"),
        &crm,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Conflict);
    let reader = mint(c, Some(nw), false, &["entity:read"]);
    let (st, _) = post_json(
        c,
        "/crm/owner-leads",
        &reader,
        serde_json::json!({ "name": "X" }),
    )
    .await;
    assert_eq!(st, Status::Forbidden);
}

/// Phase 2C: a property saved with a full address gets a photo (a placeholder
/// card here — no live maps key), which becomes its hero; the fetch is queued
/// on create and the nightly scan finds properties still without one.
async fn property_autofill_and_photo(c: &Ctx) {
    use crate::scheduler::run_due_jobs;
    let nw = tenant_id(c, "northwind").await;
    let office = mint(c, Some(nw), false, &["property:read", "property:write"]);
    let (st, geo) = get_json(c, "/geo/status", &office).await;
    assert_eq!(st, Status::Ok, "{geo}");
    assert_eq!(geo["photos"], "placeholder");
    // Known-property suggestions come from our own rows (no network needed).
    let (st, sugg) = get_json(c, "/geo/suggest?q=123%20Map&limit=3", &office).await;
    assert_eq!(st, Status::Ok, "{sugg}");
    assert!(
        sugg.as_array()
            .unwrap()
            .iter()
            .any(|p| p["source"] == "known"),
        "{sugg}"
    );

    let (st, p) = post_json(
        c,
        "/properties",
        &office,
        serde_json::json!({
            "name": "Glendale barn", "address": "8929 Glendale Ave", "city": "Hesperia",
            "state": "California", "postal_code": "92345", "units": 1, "occupied_units": 0,
            "monthly_rent_cents": 0,
        }),
    )
    .await;
    assert_eq!(st, Status::Ok, "create: {p}");
    assert_eq!(p["state"], "CA");
    assert_eq!(p["photo_status"], "none");
    let pid = p["id"].as_str().unwrap().to_string();
    for _ in 0..3 {
        run_due_jobs(&c.db).await.unwrap();
    }
    let (st, prof) = get_json(c, &format!("/properties/{pid}"), &office).await;
    assert_eq!(st, Status::Ok);
    // The property's own fields are flattened into the profile.
    assert_eq!(prof["photo_status"], "placeholder", "{prof}");
    let hero = prof["image_url"]
        .as_str()
        .expect("the card became the hero");
    assert!(
        hero.contains("/storage/") || hero.starts_with("http"),
        "{hero}"
    );
    // Fetching again by hand keeps the placeholder (still no key) and doesn't error.
    let (st, again) = post_json(
        c,
        &format!("/properties/{pid}/photo"),
        &office,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Ok, "{again}");
    assert_eq!(again["status"], "placeholder");
    // The scan touches none|failed; a placeholder waits for a key.
    let summary = crate::geo::scan(&c.db, nw).await.unwrap();
    assert_eq!(summary["live"], false);
}

/// Phase 2C: the parts loop. A dryer with a parts catalog, a repair work
/// order that pre-lists them, a finding that adds baseboards, the generated
/// parts list, the night-before close-out (from stock / order), receiving and
/// using — with every used part landing on the work order's cost. Plus the
/// scan-in: a delivery with tax spread across lines and a weighted unit cost,
/// and the appliances showing up on the public listing.
async fn parts_loop_and_closeout(c: &Ctx) {
    use rocket::http::Method;
    let nw = tenant_id(c, "northwind").await;
    let tech = mint(
        c,
        Some(nw),
        false,
        &["maintenance:read", "maintenance:manage", "property:read"],
    );
    let pid = property_ids(c, nw).await[0];

    // Stock: a belt on the shelf, a filter that isn't.
    let (st, belt) = post_json(
        c,
        "/inventory",
        &tech,
        serde_json::json!({ "name": "Dryer belt 341241", "sku": "341241", "barcode": "0123456789012",
            "category": "part", "quantity": 4, "unit_cost_cents": 1200, "reorder_level": 2, "vendor": "Repair Clinic" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{belt}");
    assert_eq!(belt["barcode"], "0123456789012");
    let belt_id = belt["id"].as_str().unwrap().to_string();
    let (st, filter) = post_json(
        c,
        "/inventory",
        &tech,
        serde_json::json!({ "name": "Lint filter", "category": "part", "quantity": 0, "unit_cost_cents": 2500 }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{filter}");
    let filter_id = filter["id"].as_str().unwrap().to_string();

    // Scan-in finds the belt by barcode; a bad code is a clean 404.
    let (st, found) = get_json(c, "/inventory/lookup?code=0123456789012", &tech).await;
    assert_eq!(st, Status::Ok, "{found}");
    assert_eq!(found["id"], belt["id"]);
    let (st, _) = get_json(c, "/inventory/lookup?code=nope", &tech).await;
    assert_eq!(st, Status::NotFound);

    // The dryer, with its parts.
    let (st, dryer) = post_json(
        c,
        "/assets",
        &tech,
        serde_json::json!({ "property_id": pid, "kind": "appliance", "name": "Dryer", "make": "Whirlpool",
            "model": "WED4815EW", "purchased_on": "2022-03-01", "purchase_price_cents": 64900,
            "expected_life_years": 12, "warranty_expires": "2099-01-01", "warranty_provider": "Whirlpool", "location": "Laundry" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{dryer}");
    assert_eq!(dryer["warranty_state"], "active");
    assert!(dryer["years_left"].as_i64().unwrap() > 5, "{dryer}");
    let dryer_id = dryer["id"].as_str().unwrap().to_string();
    for (item, role) in [(&belt_id, "drive belt"), (&filter_id, "lint filter")] {
        let (st, cat) = send_json(
            c,
            Method::Put,
            &format!("/assets/{dryer_id}/parts"),
            &tech,
            serde_json::json!({ "inventory_item_id": item, "quantity": 1, "role": role }),
        )
        .await;
        assert_eq!(st, Status::Ok, "{cat}");
    }
    let (_, cat) = get_json(c, &format!("/assets/{dryer_id}/parts"), &tech).await;
    assert_eq!(cat.as_array().unwrap().len(), 2, "{cat}");

    // Repair it: the work order pre-lists both parts as potential.
    let (st, wo) = post_json(
        c,
        &format!("/assets/{dryer_id}/work-order"),
        &tech,
        serde_json::json!({ "kind": "repair", "priority": "high", "due_date": "2030-01-02" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{wo}");
    assert_eq!(wo["potential_parts"], 2);
    assert_eq!(wo["title"], "Repair Dryer");
    let tid = wo["ticket_id"].as_str().unwrap().to_string();

    // On site: the tech adds a finding with a part that isn't stock at all.
    let (st, finding) = post_json(
        c,
        &format!("/tickets/{tid}/findings"),
        &tech,
        serde_json::json!({ "body": "Belt is shredded; baseboard behind the dryer is water damaged.",
            "parts": [ { "inventory_item_id": belt_id, "quantity": 1 },
                       { "name": "Baseboard 8ft primed MDF", "quantity": 2, "note": "match existing 3.25in" } ] }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{finding}");
    assert_eq!(finding["kind"], "finding");
    assert_eq!(finding["parts"].as_array().unwrap().len(), 2);

    // Generate the parts list: belt from stock (4 on the shelf), baseboards to buy,
    // the filter stays "maybe".
    let (st, list) = post_json(
        c,
        &format!("/tickets/{tid}/parts/generate"),
        &tech,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Ok, "{list}");
    assert_eq!(list["from_stock"].as_array().unwrap().len(), 1, "{list}");
    assert_eq!(list["from_stock"][0]["name"], "Dryer belt 341241");
    assert_eq!(list["to_buy"].as_array().unwrap().len(), 1, "{list}");
    assert_eq!(list["maybe"].as_array().unwrap().len(), 2, "{list}");
    let belt_part = list["from_stock"][0]["id"].as_str().unwrap().to_string();
    let board_part = list["to_buy"][0]["id"].as_str().unwrap().to_string();
    // …and printable.
    let resp = c
        .client
        .get(format!("/tickets/{tid}/parts/list.pdf"))
        .header(bearer(&tech))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    let pdf = resp.into_bytes().await.unwrap();
    assert!(pdf.starts_with(b"%PDF"), "not a pdf");

    // The night before: close-out shows the work order and its parts.
    let (st, co) = get_json(c, "/closeout?date=2030-01-02", &tech).await;
    assert_eq!(st, Status::Ok, "{co}");
    let mine = co["tickets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["ticket_id"] == tid.as_str())
        .unwrap_or_else(|| panic!("{co}"));
    assert_eq!(mine["parts"].as_array().unwrap().len(), 4, "{mine}");
    assert!(co["to_decide"].as_u64().unwrap() >= 1);

    // Decide: the belt comes from stock (consumed now, on the ticket), the
    // baseboards are ordered from Home Depot to the property — an owner-billable expense.
    let (st, used) = post_json(
        c,
        &format!("/parts/{belt_part}/decide"),
        &tech,
        serde_json::json!({ "action": "from_stock" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{used}");
    assert_eq!(used["status"], "used");
    let (st, ordered) = post_json(
        c,
        &format!("/parts/{board_part}/decide"),
        &tech,
        serde_json::json!({ "action": "order", "vendor": "Home Depot", "ship_to": "property", "unit_cost_cents": 1899, "tracking": "HD-1" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{ordered}");
    assert_eq!(ordered["status"], "ordered");
    assert_eq!(ordered["ship_to"], "property");
    // Deciding it twice is refused.
    let (st, _) = post_json(
        c,
        &format!("/parts/{board_part}/decide"),
        &tech,
        serde_json::json!({ "action": "skip" }),
    )
    .await;
    assert_eq!(st, Status::Conflict);

    // The shelf went down by one, with a movement behind it.
    let (_, belt_now) = get_json(c, "/inventory/lookup?code=341241", &tech).await;
    assert_eq!(belt_now["quantity"], 3, "{belt_now}");
    let (st, moves) = get_json(c, &format!("/inventory/movements?item_id={belt_id}"), &tech).await;
    assert_eq!(st, Status::Ok, "{moves}");
    assert_eq!(moves[0]["kind"], "use");
    assert_eq!(moves[0]["quantity"], -1);

    // Next day: the boards arrive and go in.
    let (st, rec) = post_json(
        c,
        &format!("/parts/{board_part}/receive"),
        &tech,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Ok, "{rec}");
    assert_eq!(rec["status"], "received");
    let (st, line) = post_json(
        c,
        &format!("/parts/{board_part}/use"),
        &tech,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Ok, "{line}");
    assert_eq!(line["total_cents"], 2 * 1899);

    // The work order carries both parts at cost: belt $12 + boards 2 × $18.99.
    let (st, detail) = get_json(c, &format!("/tickets/{tid}"), &tech).await;
    assert_eq!(st, Status::Ok, "{detail}");
    assert_eq!(detail["cost_cents"], 1200 + 3798, "{detail}");
    assert_eq!(detail["parts"].as_array().unwrap().len(), 4);
    assert_eq!(
        detail["parts"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["status"] == "used")
            .count(),
        2
    );
    let (_, hist) = get_json(c, &format!("/assets/{dryer_id}/history"), &tech).await;
    assert_eq!(hist["tickets"].as_array().unwrap().len(), 1, "{hist}");
    assert_eq!(hist["parts"].as_array().unwrap().len(), 2);

    // A delivery: 10 filters at $20 + 5 belts at $10 with $15 tax spread by
    // value ($12 / $3) → landed $21.20 / $10.60; the belts' unit cost becomes
    // the weighted average of the 3 on the shelf at $12 and 5 arriving at
    // $10.60 → $11.125 ≈ $11.13.
    let (st, rcv) = post_json(
        c,
        "/inventory/receive",
        &tech,
        serde_json::json!({ "vendor": "Repair Clinic", "total_cents": 26500,
            "lines": [ { "inventory_item_id": filter_id, "quantity": 10, "unit_cost_cents": 2000 },
                       { "inventory_item_id": belt_id, "quantity": 5, "unit_cost_cents": 1000 } ] }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{rcv}");
    assert_eq!(rcv["items"][0]["quantity"], 10);
    assert_eq!(rcv["items"][0]["unit_cost_cents"], 2120);
    assert_eq!(rcv["items"][1]["quantity"], 8);
    assert_eq!(rcv["items"][1]["unit_cost_cents"], 1113);
    // A count fixes the shelf and leaves a trail.
    let (st, counted) = post_json(
        c,
        &format!("/inventory/{belt_id}/count"),
        &tech,
        serde_json::json!({ "counted": 7, "note": "one damaged" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{counted}");
    assert_eq!(counted["quantity"], 7);

    // A routine on the dryer: the plan runner opens a ticket with its parts pre-listed.
    let (st, plan) = post_json(
        c,
        "/maintenance-plans",
        &tech,
        serde_json::json!({ "property_id": pid, "asset_id": dryer_id, "title": "Clean dryer vent", "cadence_days": 180, "next_due_date": "2020-01-01" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{plan}");
    assert_eq!(plan["asset_id"], dryer_id.as_str());
    crate::helpdesk::run_due_plans(&c.db, nw).await.unwrap();
    let (_, plan_now) = get_json(c, &format!("/maintenance-plans?property_id={pid}"), &tech).await;
    let routine = plan_now
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == plan["id"])
        .unwrap();
    let routine_ticket = routine["last_ticket_id"]
        .as_str()
        .expect("the routine opened a ticket");
    let (_, rt) = get_json(c, &format!("/tickets/{routine_ticket}"), &tech).await;
    assert_eq!(rt["asset_id"], dryer_id.as_str(), "{rt}");
    assert_eq!(rt["parts"].as_array().unwrap().len(), 2, "{rt}");

    // The property's maintenance tab sees the money and the appliance.
    let (st, pm) = get_json(c, &format!("/properties/{pid}/maintenance"), &tech).await;
    assert_eq!(st, Status::Ok, "{pm}");
    assert!(pm["assets"]
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["id"] == dryer_id.as_str()));
    assert!(pm["plans"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["id"] == plan["id"]));
    assert!(pm["expenses_cents"].as_i64().unwrap() >= 3798, "{pm}");

    // Marketing: the public listing for the property shows the dryer and the routine.
    let listing = entity::prelude::Listing::find()
        .filter(entity::listing::Column::TenantId.eq(nw))
        .filter(entity::listing::Column::PropertyId.eq(pid))
        .filter(entity::listing::Column::IsPublic.eq(true))
        .one(&c.db)
        .await
        .unwrap();
    if let Some(l) = listing {
        let resp = c
            .client
            .get(format!("/public/listings/{}", l.id))
            .header(Header::new("X-Tenant", "northwind"))
            .dispatch()
            .await;
        assert_eq!(resp.status(), Status::Ok);
        let pl = resp.into_json::<serde_json::Value>().await.unwrap();
        assert!(
            pl["appliances"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["name"] == "Dryer"),
            "{pl}"
        );
        assert!(
            pl["upkeep"]
                .as_array()
                .unwrap()
                .iter()
                .any(|u| u["cadence"] == "Every 6 months"),
            "{pl}"
        );
    }
}

/// Phase 2C: Alpha ↔ Vantedge. A contractor is linked to their Alpha account
/// (simulated ping), a work order is dispatched to them (the job posts it and
/// records Alpha's job id), Alpha calls back — signed — as the job is
/// scheduled and finished, and the ticket follows: status, timeline, and the
/// vendor's price as a line on the work order. The vendor API sees the
/// ticket too.
async fn alpha_vendor_link(c: &Ctx) {
    use crate::scheduler::run_due_jobs;
    let nw = tenant_id(c, "northwind").await;
    let office = mint(
        c,
        Some(nw),
        false,
        &[
            "maintenance:read",
            "maintenance:manage",
            "entity:read",
            "entity:manage",
            "property:read",
        ],
    );
    let pid = property_ids(c, nw).await[0];

    let (st, vendor) = post_json(
        c,
        "/entities",
        &office,
        serde_json::json!({ "kind": "contractor", "name": "Alpha Power Wash", "email": "office@alpha.example" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{vendor}");
    let vid = vendor["id"].as_str().unwrap().to_string();
    assert!(vendor["partner_kind"].is_null());

    // Link: the key is stored, Alpha is pinged (simulated), the callback secret is shown once.
    let (st, link) = post_json(
        c,
        &format!("/entities/{vid}/partner/link"),
        &office,
        serde_json::json!({ "base_url": "https://api.alpha.example", "api_key": "apw_live_test" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{link}");
    assert_eq!(link["linked"], true);
    assert_eq!(link["status"], "ok", "{link}");
    assert!(
        link["callback_url"]
            .as_str()
            .unwrap()
            .ends_with("/webhooks/alpha?tenant=northwind"),
        "{link}"
    );
    let secret = link["callback_secret"]
        .as_str()
        .expect("secret shown once")
        .to_string();
    let (_, again) = get_json(c, &format!("/entities/{vid}/partner"), &office).await;
    assert_eq!(again["callback_secret_set"], true);
    assert!(again.get("callback_secret").is_none());
    let (_, vendors) = get_json(c, "/partner/vendors", &office).await;
    assert!(
        vendors
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["id"] == vid.as_str()),
        "{vendors}"
    );

    // A work order, sent to them.
    let (st, t) = post_json(
        c,
        &format!("/properties/{pid}/tickets"),
        &office,
        serde_json::json!({ "title": "Pressure wash the walkways", "category": "general", "priority": "normal",
            "description": "Front walk and the pool deck.", "access_notes": "Gate code 4411" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{t}");
    let tid = t["id"].as_str().unwrap().to_string();
    let (st, sent) = post_json(
        c,
        &format!("/tickets/{tid}/dispatch"),
        &office,
        serde_json::json!({ "counterparty_id": vid, "requested_for": "2030-03-04T09:00:00", "service_key": "driveway", "note": "Owner wants it before the showing." }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{sent}");
    assert_eq!(sent["partner_status"], "sending");
    assert_eq!(sent["assignee_entity_id"], vid.as_str());
    // Twice is refused.
    let (st, twice) = post_json(
        c,
        &format!("/tickets/{tid}/dispatch"),
        &office,
        serde_json::json!({ "counterparty_id": vid }),
    )
    .await;
    assert_eq!(st, Status::Conflict, "{twice}");
    for _ in 0..3 {
        run_due_jobs(&c.db).await.unwrap();
    }
    let (_, t) = get_json(c, &format!("/tickets/{tid}"), &office).await;
    assert_eq!(t["partner_job_id"], "4242", "{t}");
    assert_eq!(t["status"], "scheduled");
    assert!(
        t["comments"]
            .as_array()
            .unwrap()
            .iter()
            .any(|k| k["body"].as_str().unwrap().contains("Alpha job #4242")),
        "{t}"
    );

    // Alpha calls back, signed with the secret we gave the vendor.
    let call = |event: &str, job: serde_json::Value| {
        let body =
            serde_json::json!({ "event": event, "sent_at": "2030-03-04T17:00:00Z", "job": job })
                .to_string();
        let sig = crate::providers::webhook::sign(&secret, body.as_bytes());
        (body, sig)
    };
    let (body, sig) = call(
        "job.scheduled",
        serde_json::json!({ "id": 4242, "external_ref": tid, "status": "scheduled",
        "scheduled_for": "2030-03-04T09:00:00-08:00", "crew": ["Carlos Crew"] }),
    );
    let resp = c
        .client
        .post("/webhooks/alpha?tenant=northwind")
        .header(ContentType::JSON)
        .header(Header::new("X-Acre-Signature", sig))
        .body(body.clone())
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    // A bad signature is refused.
    let resp = c
        .client
        .post("/webhooks/alpha?tenant=northwind")
        .header(ContentType::JSON)
        .header(Header::new("X-Acre-Signature", "sha256=00"))
        .body(body)
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Unauthorized);
    let (body, sig) = call(
        "job.completed",
        serde_json::json!({ "id": 4242, "external_ref": tid, "status": "complete",
        "service": "Driveways & Concrete", "price": "189.50", "report_note": "Rinsed twice; oil stain lifted.",
        "photos": [{ "kind": "after", "url": "https://api.alpha.example/media/jobs/1.jpg" }] }),
    );
    let resp = c
        .client
        .post("/webhooks/alpha?tenant=northwind")
        .header(ContentType::JSON)
        .header(Header::new("X-Acre-Signature", sig))
        .body(body)
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    for _ in 0..3 {
        run_due_jobs(&c.db).await.unwrap();
    }
    let (_, t) = get_json(c, &format!("/tickets/{tid}"), &office).await;
    assert_eq!(t["status"], "resolved", "{t}");
    assert_eq!(t["partner_status"], "complete");
    assert_eq!(t["cost_cents"], 18950, "{t}");
    assert!(t["resolved_at"].is_string());
    let bodies: Vec<&str> = t["comments"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|k| k["body"].as_str())
        .collect();
    assert!(
        bodies.iter().any(|b| b.contains("Carlos Crew")),
        "{bodies:?}"
    );
    assert!(
        bodies
            .iter()
            .any(|b| b.contains("$189.50") && b.contains("1.jpg")),
        "{bodies:?}"
    );
    assert_eq!(t["lines"].as_array().unwrap().len(), 1);

    // The vendor API sees the ticket and can post progress on another one.
    let token = insert_api_token(c, nw, &["maintenance:read", "maintenance:manage"]).await;
    let (st, v) = get_json(c, &format!("/api/v1/tickets/{tid}"), &token).await;
    assert_eq!(st, Status::Ok, "{v}");
    assert_eq!(v["status"], "resolved");
    assert!(
        v["property_address"]
            .as_str()
            .map(|a| !a.is_empty())
            .unwrap_or(false),
        "{v}"
    );
    let (st, opened) = get_json(c, "/api/v1/tickets", &token).await;
    assert_eq!(st, Status::Ok, "{opened}");
    assert!(opened
        .as_array()
        .unwrap()
        .iter()
        .all(|x| x["status"] != "resolved"));
    let other = opened[0]["id"].as_str().unwrap().to_string();
    let (st, patched) = send_json(
        c,
        rocket::http::Method::Patch,
        &format!("/api/v1/tickets/{other}"),
        &token,
        serde_json::json!({ "status": "in_progress", "note": "Crew on site." }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{patched}");
    assert_eq!(patched["status"], "in_progress");
    assert!(patched["comments"]
        .as_array()
        .unwrap()
        .iter()
        .any(|k| k["body"].as_str().unwrap().contains("Crew on site.")));
    // Read-only tokens can't post.
    let ro = insert_api_token(c, nw, &["maintenance:read"]).await;
    let (st, _) = send_json(
        c,
        rocket::http::Method::Patch,
        &format!("/api/v1/tickets/{other}"),
        &ro,
        serde_json::json!({ "status": "resolved" }),
    )
    .await;
    assert_eq!(st, Status::Forbidden);

    // Unlink clears the key and the flag.
    let resp = c
        .client
        .delete(format!("/entities/{vid}/partner/link"))
        .header(bearer(&office))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    let (_, after) = get_json(c, &format!("/entities/{vid}/partner"), &office).await;
    assert_eq!(after["linked"], false);
}

/// Roadmap area 1: who changed what, on which property. Edits to a property,
/// its unit and a work order leave before → after diffs on the property's
/// history; a Vantedge employee's edit is flagged as support; contact details
/// never appear; another workspace sees none of it.
async fn audit_trail_who_changed_what(c: &Ctx) {
    use rocket::http::Method;
    let nw = tenant_id(c, "northwind").await;
    let cascade = tenant_id(c, "cascade").await;
    let mgr = mint(
        c,
        Some(nw),
        false,
        &[
            "property:read",
            "property:write",
            "lease:manage",
            "maintenance:manage",
            "maintenance:read",
            "audit:read",
        ],
    );
    let pid = property_ids(c, nw).await[0];

    // The property: rename it, and change its manager.
    let (st, before) = get_json(c, &format!("/properties/{pid}"), &mgr).await;
    assert_eq!(st, Status::Ok);
    let old_name = before["name"].as_str().unwrap().to_string();
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/properties/{pid}"),
        &mgr,
        serde_json::json!({ "name": "Renamed for the audit test", "manager": "Pat Lee" }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    // A save that changes nothing leaves no row.
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/properties/{pid}"),
        &mgr,
        serde_json::json!({ "name": "Renamed for the audit test" }),
    )
    .await;
    assert_eq!(st, Status::Ok);

    // A unit on it, then a rent change.
    let (st, unit) = post_json(c, &format!("/properties/{pid}/units"), &mgr, serde_json::json!({ "unit_number": "AUD-1", "market_rent_cents": 120000, "status": "vacant" })).await;
    assert_eq!(st, Status::Ok, "{unit}");
    let uid = unit["id"].as_str().unwrap().to_string();
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/units/{uid}"),
        &mgr,
        serde_json::json!({ "market_rent_cents": 135000 }),
    )
    .await;
    assert_eq!(st, Status::Ok);

    // A work order, then its status.
    let (st, t) = post_json(
        c,
        &format!("/properties/{pid}/tickets"),
        &mgr,
        serde_json::json!({ "title": "Audit leak", "priority": "normal" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{t}");
    let tid = t["id"].as_str().unwrap().to_string();
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/tickets/{tid}"),
        &mgr,
        serde_json::json!({ "status": "triage", "priority": "high" }),
    )
    .await;
    assert_eq!(st, Status::Ok);

    // A Vantedge employee, acting on the workspace, edits the property too.
    let staff = mint(c, Some(nw), true, &["property:read", "property:write"]);
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/properties/{pid}"),
        &staff,
        serde_json::json!({ "name": "Fixed by support" }),
    )
    .await;
    assert_eq!(st, Status::Ok);

    let (st, hist) = get_json(c, &format!("/properties/{pid}/history"), &mgr).await;
    assert_eq!(st, Status::Ok, "{hist}");
    let events = hist["events"].as_array().unwrap();
    let find = |label: &str, summary_has: &str| {
        events.iter().find(|e| {
            e["label"].as_str().unwrap_or("").contains(label)
                && e["summary"].as_str().unwrap_or("").contains(summary_has)
        })
    };
    // The rename: from the old name to the new one, by the manager, not support.
    let rename = events
        .iter()
        .find(|e| {
            e["changes"]
                .as_array()
                .map(|ch| {
                    ch.iter()
                        .any(|x| x["field"] == "name" && x["to"] == "Renamed for the audit test")
                })
                .unwrap_or(false)
        })
        .unwrap_or_else(|| panic!("{hist}"));
    assert_eq!(rename["support"], false);
    assert!(
        rename["changes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["field"] == "name" && x["from"] == old_name.as_str()),
        "{rename}"
    );
    assert!(rename["changes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|x| x["field"] == "manager" && x["to"] == "Pat Lee"));
    // Only one rename event: the no-op save left nothing.
    assert_eq!(
        events
            .iter()
            .filter(|e| e["changes"]
                .as_array()
                .map(|ch| ch.iter().any(|x| x["to"] == "Renamed for the audit test"))
                .unwrap_or(false))
            .count(),
        1
    );
    // The unit: created, then its rent changed from 1200 to 1350.
    assert!(find("Unit AUD-1", "created").is_some(), "{hist}");
    let rent = find("Unit AUD-1", "changed").expect("unit rent change");
    assert!(
        rent["changes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["field"] == "market_rent_cents" && x["from"] == 120000 && x["to"] == 135000),
        "{rent}"
    );
    // The work order: its status and priority.
    let tk = find("Audit leak", "changed").expect("ticket change");
    assert!(tk["changes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|x| x["field"] == "status" && x["to"] == "triage"));
    // Support is flagged, and filterable.
    let sup = events
        .iter()
        .find(|e| {
            e["changes"]
                .as_array()
                .map(|ch| ch.iter().any(|x| x["to"] == "Fixed by support"))
                .unwrap_or(false)
        })
        .expect("support edit");
    assert_eq!(sup["support"], true);
    let (_, only) = get_json(c, &format!("/properties/{pid}/history?support=true"), &mgr).await;
    assert!(only["events"]
        .as_array()
        .unwrap()
        .iter()
        .all(|e| e["support"] == true));
    assert!(!only["events"].as_array().unwrap().is_empty());
    // Kind filter and paging.
    let (_, units_only) = get_json(c, &format!("/properties/{pid}/history?kind=unit"), &mgr).await;
    assert!(units_only["events"]
        .as_array()
        .unwrap()
        .iter()
        .all(|e| e["target_type"] == "unit"));
    let (_, page1) = get_json(c, &format!("/properties/{pid}/history?limit=2"), &mgr).await;
    assert_eq!(page1["events"].as_array().unwrap().len(), 2);
    let next = page1["next"].as_str().expect("a second page");
    let (_, page2) = get_json(
        c,
        &format!(
            "/properties/{pid}/history?limit=2&before={}",
            next.replace('+', "%2B")
        ),
        &mgr,
    )
    .await;
    assert!(page2["events"]
        .as_array()
        .unwrap()
        .iter()
        .all(|e| e["id"] != page1["events"][0]["id"]));

    // The workspace trail and the CSV, for someone with audit:read.
    let (st, trail) = get_json(
        c,
        &format!("/audit/events?property_id={pid}&target_type=maintenance_ticket"),
        &mgr,
    )
    .await;
    assert_eq!(st, Status::Ok, "{trail}");
    assert!(!trail["events"].as_array().unwrap().is_empty());
    let resp = c
        .client
        .get(format!("/audit/events.csv?property_id={pid}"))
        .header(bearer(&mgr))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    let csv = resp.into_string().await.unwrap();
    assert!(csv.starts_with("When,Who,Vantedge support"), "{csv}");
    assert!(csv.contains("Renamed for the audit test"), "{csv}");
    // Without audit:read there's the property's history but not the workspace trail.
    let plain = mint(c, Some(nw), false, &["property:read"]);
    let (st, _) = get_json(c, &format!("/properties/{pid}/history"), &plain).await;
    assert_eq!(st, Status::Ok);
    let (st, _) = get_json(c, "/audit/events", &plain).await;
    assert_eq!(st, Status::Forbidden);

    // Another workspace sees none of it — not by property id, not in its trail.
    let other = mint(c, Some(cascade), false, &["property:read", "audit:read"]);
    let (st, _) = get_json(c, &format!("/properties/{pid}/history"), &other).await;
    assert_eq!(st, Status::NotFound);
    let (_, theirs) = get_json(c, "/audit/events?limit=200", &other).await;
    assert!(
        theirs["events"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["label"] != "Unit AUD-1" && e["label"] != "Audit leak"),
        "{theirs}"
    );
    // …and the older platform route no longer shows other workspaces' rows to a workspace-bound principal.
    let (st, legacy) = get_json(c, "/admin/audit?limit=500", &other).await;
    assert_eq!(st, Status::Ok);
    assert!(
        legacy
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["tenant_id"].is_null() || r["tenant_id"] == cascade.to_string().as_str()),
        "{legacy}"
    );
}
