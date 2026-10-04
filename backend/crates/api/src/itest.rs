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

    // Quiet hours follow the wall clock; scenarios that send texts shouldn't.
    // `batch_c_texts` turns them on with a window it controls.
    for slug in ["northwind", "cascade"] {
        let t = tenant_id(&c, slug).await;
        crate::settings::set_value(
            &c.db,
            t,
            crate::settings::TEXTS_QUIET_HOURS,
            serde_json::json!(false),
        )
        .await
        .unwrap();
    }

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
    turnover_step_logic(&c).await;
    issue_catalog_generates_ticket_and_shopping_list(&c).await;
    business_profile_and_google_reviews(&c).await;
    site_maps_apartment_and_campground(&c).await;
    public_search_tours_and_autofill(&c).await;
    alpha_single_sign_on(&c).await;
    embed_settings(&c).await;
    seo_site_info(&c).await;
    batch_a_limits_jobs_and_reminders(&c).await;
    batch_b_vendor_compliance(&c).await;
    batch_c_texts(&c).await;
    batch_d_listing_photos(&c).await;
    property_reach(&c).await;
    // Older scenarios send work out freely; the owner-approval gate is
    // exercised on its own in `owner_approvals_flow` through an owner's own
    // limit.
    crate::settings::set_value(
        &c.db,
        tenant_id(&c, "northwind").await,
        crate::settings::MAINTENANCE_OWNER_APPROVAL_CENTS,
        serde_json::json!(0),
    )
    .await
    .unwrap();
    service_desk(&c).await;
    imports_and_exports(&c).await;
    listing_syndication(&c).await;
    maintenance_actions_and_resident(&c).await;
    property_profile_records(&c).await;
    desk_queues_and_vendor_batches(&c).await;
    appointments_flow(&c).await;
    vendor_link_flow(&c).await;
    showings_flow(&c).await;
    property_data_sources(&c).await;
    owner_approvals_flow(&c).await;
    attention_and_mandates(&c).await;
    routes_and_shopping(&c).await;
    follow_ups_go_out(&c).await;
    analytics_and_map(&c).await;
    texts_round_two(&c).await;
    go_live_page(&c).await;
    spanish_messages(&c).await;
    onboarding_checklist(&c).await;
    vendor_portal_flow(&c).await;
    property_story_and_timeline(&c).await;
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
    // Tick until the job reaches a terminal state (or we give up). Each tick
    // advances at most 25 due jobs, oldest first, so earlier scenarios' queued
    // notifications can take several ticks before this job's turn comes.
    async fn drain(c: &Ctx, id: Uuid) -> entity::background_job::Model {
        for _ in 0..200 {
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

    // One task of a bigger work order (no access notes) goes to them: the job
    // is that task, and their progress moves that task, not the work order.
    let (_, t2) = post_json(
        c,
        &format!("/properties/{pid}/tickets"),
        &office,
        serde_json::json!({ "title": "Unit turn", "category": "general", "priority": "normal" }),
    )
    .await;
    let tid2 = t2["id"].as_str().unwrap().to_string();
    for (title, trade) in [("Wash the stairs", "exterior"), ("Repaint", "paint")] {
        let (st, v) = post_json(
            c,
            &format!("/tickets/{tid2}/tasks"),
            &office,
            serde_json::json!({ "title": title, "trade": trade, "needs_contractor": true }),
        )
        .await;
        assert_eq!(st, Status::Ok, "{v}");
    }
    let (_, tasks) = get_json(c, &format!("/tickets/{tid2}/tasks"), &office).await;
    let wash = tasks[0]["id"].as_str().unwrap().to_string();
    let (st, v) = post_json(
        c,
        &format!("/tickets/{tid2}/tasks/{wash}/dispatch"),
        &office,
        serde_json::json!({ "entity_id": vid }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{v}");
    let spec = entity::prelude::BackgroundJob::find()
        .filter(entity::background_job::Column::Kind.eq(crate::partner::DISPATCH_KIND))
        .all(&c.db)
        .await
        .unwrap()
        .into_iter()
        .find(|j| j.payload["ticket_id"] == tid2.as_str())
        .expect("queued");
    assert_eq!(spec.payload["title"], "Wash the stairs — Unit turn");
    for _ in 0..3 {
        run_due_jobs(&c.db).await.unwrap();
    }
    for (event, status) in [
        ("job.started", "in_progress"),
        ("job.completed", "complete"),
    ] {
        let (body, sig) = call(
            event,
            serde_json::json!({ "id": 4242, "external_ref": tid2, "status": status,
                "service": "Exterior wash", "price": "245.00" }),
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
    }
    let (_, tasks) = get_json(c, &format!("/tickets/{tid2}/tasks"), &office).await;
    assert_eq!(tasks[0]["status"], "done", "{tasks}");
    assert_eq!(tasks[1]["status"], "todo", "{tasks}");
    let (_, t2) = get_json(c, &format!("/tickets/{tid2}"), &office).await;
    assert_eq!(t2["status"], "in_progress", "the repaint is still to do");
    assert_eq!(t2["cost_cents"], 24500, "{t2}");

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

/// Turnover step logic: dependencies unblock steps, gates hold, a resolved
/// work order completes its step, the unit cannot go vacant mid-turn, and
/// the whole thing lands in the property's history.
async fn turnover_step_logic(c: &Ctx) {
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
    let (st, unit) = post_json(
        c,
        &format!("/properties/{pid}/units"),
        &mgr,
        serde_json::json!({ "unit_number": "TURN-1", "market_rent_cents": 150000, "status": "occupied" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{unit}");
    let uid = unit["id"].as_str().unwrap().to_string();

    // Start the turn: 13 steps, only the first is ready.
    let (st, p) = post_json(
        c,
        &format!("/units/{uid}/turn"),
        &mgr,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Ok, "{p}");
    let proc_id = p["id"].as_str().unwrap().to_string();
    assert_eq!(p["total"], 13);
    assert_eq!(p["done"], 0);
    let step_id = |p: &serde_json::Value, key: &str| -> String {
        p["steps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["key"] == key)
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let status_of = |p: &serde_json::Value, key: &str| -> String {
        p["steps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["key"] == key)
            .unwrap()["status"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(status_of(&p, "notice"), "ready");
    assert_eq!(status_of(&p, "inspect"), "blocked");

    // A second turn on the same unit is refused.
    let (st, _) = post_json(
        c,
        &format!("/units/{uid}/turn"),
        &mgr,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Conflict);

    // A blocked step cannot be completed; finishing it unblocks the next.
    let act = |id: String, body: serde_json::Value| {
        let mgr = mgr.clone();
        async move { post_json(c, &format!("/process-steps/{id}/action"), &mgr, body).await }
    };
    let (st, _) = act(
        step_id(&p, "inspect"),
        serde_json::json!({ "action": "complete" }),
    )
    .await;
    assert_eq!(st, Status::Conflict);
    let (st, p) = act(
        step_id(&p, "notice"),
        serde_json::json!({ "action": "complete" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{p}");
    assert_eq!(status_of(&p, "notice"), "done");
    assert_eq!(status_of(&p, "inspect"), "ready");
    assert_eq!(status_of(&p, "repairs"), "blocked");

    // The inspection needs a photo; skipping needs a reason.
    let (st, _) = act(
        step_id(&p, "inspect"),
        serde_json::json!({ "action": "complete" }),
    )
    .await;
    assert_eq!(st, Status::Conflict);
    let (st, _) = act(
        step_id(&p, "inspect"),
        serde_json::json!({ "action": "skip" }),
    )
    .await;
    assert_eq!(st, Status::BadRequest);
    let (st, p) = act(
        step_id(&p, "inspect"),
        serde_json::json!({ "action": "skip", "reason": "Done on paper at move-out" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{p}");
    assert_eq!(status_of(&p, "repairs"), "ready");
    assert_eq!(status_of(&p, "paint"), "blocked");

    // Repairs opens a work order; resolving it completes the step.
    let (st, p) = post_json(
        c,
        &format!("/process-steps/{}/ticket", step_id(&p, "repairs")),
        &mgr,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Ok, "{p}");
    assert_eq!(status_of(&p, "repairs"), "doing");
    let tid = p["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["key"] == "repairs")
        .unwrap()["ticket_id"]
        .as_str()
        .unwrap()
        .to_string();
    let (st, _) = post_json(
        c,
        &format!("/process-steps/{}/ticket", step_id(&p, "repairs")),
        &mgr,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Conflict);
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/tickets/{tid}"),
        &mgr,
        serde_json::json!({ "status": "resolved" }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    let (st, p) = get_json(c, &format!("/processes/{proc_id}"), &mgr).await;
    assert_eq!(st, Status::Ok, "{p}");
    assert_eq!(status_of(&p, "repairs"), "done");
    assert_eq!(status_of(&p, "trash_out"), "ready");
    assert_eq!(status_of(&p, "paint"), "blocked"); // waits on trash-out too
    assert_eq!(status_of(&p, "rekey"), "ready");

    // The unit cannot go vacant while required steps are open.
    let (st, body) = send_json(
        c,
        Method::Patch,
        &format!("/units/{uid}"),
        &mgr,
        serde_json::json!({ "status": "vacant" }),
    )
    .await;
    assert_eq!(st, Status::Conflict, "{body}");

    // Finishing needs the required steps, or an override reason.
    let (st, _) = post_json(
        c,
        &format!("/processes/{proc_id}/finish"),
        &mgr,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Conflict);
    let (st, p) = post_json(
        c,
        &format!("/processes/{proc_id}/finish"),
        &mgr,
        serde_json::json!({ "override_reason": "Owner is selling as-is" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{p}");
    assert_eq!(p["status"], "done");
    assert_eq!(p["override_reason"], "Owner is selling as-is");
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/units/{uid}"),
        &mgr,
        serde_json::json!({ "status": "vacant" }),
    )
    .await;
    assert_eq!(st, Status::Ok);

    // Templates: a loop is refused, a valid recipe saves and can be started.
    let (st, tpls) = get_json(c, "/process-templates?kind=turnover", &mgr).await;
    assert_eq!(st, Status::Ok);
    assert_eq!(tpls[0]["steps"].as_array().unwrap().len(), 13);
    let (st, _) = post_json(
        c,
        "/process-templates",
        &mgr,
        serde_json::json!({ "name": "Loop", "steps": [
            { "key": "a", "title": "A", "depends_on": ["b"] },
            { "key": "b", "title": "B", "depends_on": ["a"] }
        ]}),
    )
    .await;
    assert_eq!(st, Status::BadRequest);
    let (st, t) = post_json(
        c,
        "/process-templates",
        &mgr,
        serde_json::json!({ "name": "Quick turn", "steps": [
            { "key": "clean", "title": "Clean" },
            { "key": "list", "title": "List it", "depends_on": ["clean"] }
        ]}),
    )
    .await;
    assert_eq!(st, Status::Ok, "{t}");
    let (st, p2) = post_json(
        c,
        &format!("/units/{uid}/turn"),
        &mgr,
        serde_json::json!({ "template_id": t["id"] }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{p2}");
    assert_eq!(p2["total"], 2);
    let (st, p2) = post_json(
        c,
        &format!("/processes/{}/cancel", p2["id"].as_str().unwrap()),
        &mgr,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Ok);
    assert_eq!(p2["status"], "cancelled");

    // Another workspace sees none of it.
    let other = mint(c, Some(cascade), false, &["maintenance:read"]);
    let (st, _) = get_json(c, &format!("/processes/{proc_id}"), &other).await;
    assert_eq!(st, Status::NotFound);
    let (st, list) = get_json(c, "/processes", &other).await;
    assert_eq!(st, Status::Ok);
    assert!(list.as_array().unwrap().is_empty());

    // It all shows up on the property's history.
    let (st, hist) = get_json(c, &format!("/properties/{pid}/history?limit=100"), &mgr).await;
    assert_eq!(st, Status::Ok, "{hist}");
    let actions: Vec<&str> = hist["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["action"].as_str())
        .collect();
    for want in ["process.start", "process.step_update", "process.finish"] {
        assert!(actions.contains(&want), "missing {want} in {actions:?}");
    }
}

/// Issue catalog: pick an issue, generate the ticket, get the shopping list
/// split into what is on the shelf and what to buy.
async fn issue_catalog_generates_ticket_and_shopping_list(c: &Ctx) {
    use rocket::http::Method;
    let nw = tenant_id(c, "northwind").await;
    let cascade = tenant_id(c, "cascade").await;
    let mgr = mint(
        c,
        Some(nw),
        false,
        &[
            "property:read",
            "maintenance:manage",
            "maintenance:read",
            "audit:read",
        ],
    );
    let pid = property_ids(c, nw).await[0];

    // The starter set appears on first use.
    let (st, issues) = get_json(c, "/issue-templates", &mgr).await;
    assert_eq!(st, Status::Ok, "{issues}");
    let faucet = issues
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["name"] == "Replace faucet cartridge")
        .expect("catalog kit")
        .clone();
    assert!(issues.as_array().unwrap().len() >= 10);

    // One of its parts is on the shelf; the other is not.
    let (st, item) = post_json(
        c,
        "/inventory",
        &mgr,
        serde_json::json!({ "name": "Faucet cartridge", "quantity": 5, "unit_cost_cents": 1800 }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{item}");

    let (st, out) = post_json(
        c,
        &format!(
            "/issue-templates/{}/generate",
            faucet["id"].as_str().unwrap()
        ),
        &mgr,
        serde_json::json!({ "property_id": pid, "note": "Drips every few seconds" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{out}");
    assert_eq!(out["ticket"]["category"], "plumbing");
    assert!(out["ticket"]["description"]
        .as_str()
        .unwrap()
        .contains("Drips every few seconds"));
    // An action kit, not a symptom: its steps are tasks on the work order.
    let tid = out["ticket"]["id"].as_str().unwrap();
    let (_, tasks) = get_json(c, &format!("/tickets/{tid}/tasks"), &mgr).await;
    assert!(tasks
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["title"] == "Install the new cartridge"));
    // The old symptom entries aren't in the catalog.
    assert!(!issues
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["name"] == "AC not cooling" || i["name"] == "Leaking faucet"));
    let from_stock: Vec<&str> = out["parts"]["from_stock"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    let to_buy: Vec<&str> = out["parts"]["to_buy"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(from_stock, vec!["Faucet cartridge"]);
    assert_eq!(to_buy, vec!["Supply line"]);

    // Editing the catalog: add an issue, retire it, and validate input.
    let (st, mine) = post_json(
        c,
        "/issue-templates",
        &mgr,
        serde_json::json!({ "name": "Gate latch broken", "category": "structural",
            "checklist": ["Replace latch"], "parts": [{ "name": "Gate latch", "quantity": 1 }] }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{mine}");
    let (st, _) = post_json(
        c,
        "/issue-templates",
        &mgr,
        serde_json::json!({ "name": "Bad", "category": "nonsense" }),
    )
    .await;
    assert_eq!(st, Status::BadRequest);
    let (st, _) = send_json(
        c,
        Method::Delete,
        &format!("/issue-templates/{}", mine["id"].as_str().unwrap()),
        &mgr,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Ok);
    let (st, _) = post_json(
        c,
        &format!("/issue-templates/{}/generate", mine["id"].as_str().unwrap()),
        &mgr,
        serde_json::json!({ "property_id": pid }),
    )
    .await;
    assert_eq!(st, Status::Conflict);

    // Another workspace has its own catalog and cannot reach ours.
    let other = mint(
        c,
        Some(cascade),
        false,
        &["maintenance:manage", "maintenance:read"],
    );
    let (st, _) = post_json(
        c,
        &format!(
            "/issue-templates/{}/generate",
            faucet["id"].as_str().unwrap()
        ),
        &other,
        serde_json::json!({ "property_id": pid }),
    )
    .await;
    assert_eq!(st, Status::NotFound);

    // The generated ticket is on the property's history.
    let (st, hist) = get_json(c, &format!("/properties/{pid}/history?limit=100"), &mgr).await;
    assert_eq!(st, Status::Ok);
    assert!(hist["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["action"] == "issue.generate_ticket"));
}

/// Business profile and Google reviews: the client edits it, a Vantedge
/// employee edits the same record (flagged as support), the public site gets
/// the filtered reviews, and another workspace sees none of it.
async fn business_profile_and_google_reviews(c: &Ctx) {
    use rocket::http::Method;
    let nw = tenant_id(c, "northwind").await;
    let cascade = tenant_id(c, "cascade").await;
    let owner = mint(c, Some(nw), false, &["integrations:manage", "audit:read"]);
    let nobody = mint(c, Some(nw), false, &["property:read"]);

    // Nothing yet; no permission, no access.
    let (st, p) = get_json(c, "/business-profile", &owner).await;
    assert_eq!(st, Status::Ok, "{p}");
    assert!(p["google_place_id"].is_null());
    assert_eq!(p["min_rating"], 4);
    let (st, _) = get_json(c, "/business-profile", &nobody).await;
    assert_eq!(st, Status::Forbidden);

    // Save, with validation.
    let (st, _) = send_json(
        c,
        Method::Put,
        "/business-profile",
        &owner,
        serde_json::json!({ "website": "javascript:alert(1)" }),
    )
    .await;
    assert_eq!(st, Status::BadRequest);
    let (st, _) = send_json(
        c,
        Method::Put,
        "/business-profile",
        &owner,
        serde_json::json!({ "min_rating": 9 }),
    )
    .await;
    assert_eq!(st, Status::BadRequest);
    let (st, p) = send_json(
        c,
        Method::Put,
        "/business-profile",
        &owner,
        serde_json::json!({ "business_name": "Northwind Rentals", "phone": "555-0100",
            "website": "https://northwind.example" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{p}");
    assert_eq!(p["business_name"], "Northwind Rentals");

    // Find the business on Google (sample data without a live key), pick it.
    let (st, found) = get_json(
        c,
        "/business-profile/google/search?q=Northwind%20Rentals",
        &owner,
    )
    .await;
    assert_eq!(st, Status::Ok, "{found}");
    let place_id = found[0]["place_id"].as_str().unwrap().to_string();
    let (st, _) = get_json(c, "/business-profile/google/place", &owner).await;
    assert_eq!(st, Status::NotFound, "no place picked yet");
    let (st, p) = send_json(
        c,
        Method::Put,
        "/business-profile",
        &owner,
        serde_json::json!({ "google_place_id": place_id, "google_place_name": "Northwind Rentals" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{p}");
    assert!(p["review_link"].as_str().unwrap().contains(&place_id));
    let (st, place) = get_json(c, "/business-profile/google/place?refresh=true", &owner).await;
    assert_eq!(st, Status::Ok, "{place}");
    assert_eq!(place["reviews"].as_array().unwrap().len(), 3);

    // The public site: the 3-star review is under the 4-star floor.
    async fn public(c: &Ctx, slug: &'static str) -> (Status, serde_json::Value) {
        let resp = c
            .client
            .get("/public/reviews")
            .header(Header::new("X-Tenant", slug))
            .dispatch()
            .await;
        let st = resp.status();
        (st, resp.into_json::<serde_json::Value>().await.unwrap())
    }
    let (st, pr) = public(c, "northwind").await;
    assert_eq!(st, Status::Ok, "{pr}");
    assert_eq!(pr["reviews"].as_array().unwrap().len(), 2);
    assert!(pr["write_url"].as_str().unwrap().contains(&place_id));
    // Hidden when switched off.
    let (st, _) = send_json(
        c,
        Method::Put,
        "/business-profile",
        &owner,
        serde_json::json!({ "show_reviews": false }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    let (_, pr) = public(c, "northwind").await;
    assert!(pr["reviews"].as_array().unwrap().is_empty());
    // Another workspace has no profile.
    let (st, pr) = public(c, "cascade").await;
    assert_eq!(st, Status::Ok);
    assert!(pr["reviews"].as_array().unwrap().is_empty());
    let other = mint(c, Some(cascade), false, &["integrations:manage"]);
    let (_, p) = get_json(c, "/business-profile", &other).await;
    assert!(p["business_name"].is_null());

    // A Vantedge employee fixes the phone number on a call.
    let staff = mint(c, Some(nw), true, &["integrations:manage"]);
    let (st, p) = send_json(
        c,
        Method::Put,
        "/business-profile",
        &staff,
        serde_json::json!({ "phone": "555-0199" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{p}");
    let (st, trail) = get_json(
        c,
        "/audit/events?target_type=business_profile&limit=50",
        &owner,
    )
    .await;
    assert_eq!(st, Status::Ok, "{trail}");
    let events = trail["events"].as_array().unwrap();
    let call = events
        .iter()
        .find(|e| e["support"] == true)
        .expect("the support edit is flagged");
    assert_eq!(call["action"], "business_profile.save");
    assert!(call["changes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|ch| ch["field"] == "phone" && ch["to"] != "555-0199"));
    // Contact details are masked in the trail, never shown in full.
    assert!(events.iter().any(|e| e["support"] == false));
}

/// Site maps: an apartment layout with units linked to unit records, a
/// campground with sites, bulk saves, GeoJSON round trip with OSM tags,
/// publishing, and workspace isolation.
async fn site_maps_apartment_and_campground(c: &Ctx) {
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
            "audit:read",
        ],
    );
    let pid = property_ids(c, nw).await[0];
    let sq = |lng: f64, lat: f64| {
        serde_json::json!({ "type": "Polygon", "coordinates": [[
            [lng, lat], [lng + 0.0001, lat], [lng + 0.0001, lat + 0.0001],
            [lng, lat + 0.0001], [lng, lat]]] })
    };

    // An apartment map with two units drawn and linked.
    let (st, u1) = post_json(
        c,
        &format!("/properties/{pid}/units"),
        &mgr,
        serde_json::json!({ "unit_number": "MAP-1", "status": "vacant", "market_rent_cents": 120000 }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{u1}");
    let (_, u2) = post_json(
        c,
        &format!("/properties/{pid}/units"),
        &mgr,
        serde_json::json!({ "unit_number": "MAP-2", "status": "occupied" }),
    )
    .await;
    let (st, m) = post_json(
        c,
        "/site-maps",
        &mgr,
        serde_json::json!({ "property_id": pid, "name": "Complex layout", "kind": "apartment",
            "center_lng": -117.3, "center_lat": 34.4 }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{m}");
    let mid = m["id"].as_str().unwrap().to_string();
    let (st, _) = post_json(
        c,
        "/site-maps",
        &mgr,
        serde_json::json!({ "property_id": pid, "name": "x", "kind": "castle" }),
    )
    .await;
    assert_eq!(st, Status::BadRequest);

    let (st, m) = send_json(
        c,
        Method::Put,
        &format!("/site-maps/{mid}/features"),
        &mgr,
        serde_json::json!({ "features": [
            { "kind": "building", "name": "Building A", "geometry": sq(-117.3, 34.4) },
            { "kind": "unit", "name": "MAP-1", "geometry": sq(-117.3, 34.4001), "unit_id": u1["id"] },
            { "kind": "unit", "name": "MAP-2", "geometry": sq(-117.3, 34.4002), "unit_id": u2["id"] },
        ]}),
    )
    .await;
    assert_eq!(st, Status::Ok, "{m}");
    assert_eq!(m["stats"]["units"], 2);
    assert_eq!(m["stats"]["units_available"], 1);
    let feats = m["features"].as_array().unwrap();
    assert!(feats[1]["area_m2"].as_f64().unwrap() > 100.0);
    assert_eq!(feats[1]["unit"]["status"], "vacant");

    // Validation: a point cannot be a unit; a unit is drawn once; a foreign unit is refused.
    let bad = |features: serde_json::Value| {
        let mgr = mgr.clone();
        let mid = mid.clone();
        async move {
            send_json(
                c,
                Method::Put,
                &format!("/site-maps/{mid}/features"),
                &mgr,
                serde_json::json!({ "features": features }),
            )
            .await
            .0
        }
    };
    assert_eq!(
        bad(serde_json::json!([{ "kind": "unit", "geometry": { "type": "Point", "coordinates": [0, 0] } }])).await,
        Status::BadRequest
    );
    assert_eq!(
        bad(serde_json::json!([
            { "kind": "unit", "geometry": sq(0.0, 0.0), "unit_id": u1["id"] },
            { "kind": "unit", "geometry": sq(1.0, 1.0), "unit_id": u1["id"] }]))
        .await,
        Status::BadRequest
    );
    assert_eq!(
        bad(serde_json::json!([{ "kind": "unit", "geometry": sq(0.0, 0.0), "unit_id": uuid::Uuid::new_v4() }])).await,
        Status::BadRequest
    );

    // A campground: sites with attributes, a road and an amenity.
    let (st, camp) = post_json(
        c,
        "/site-maps",
        &mgr,
        serde_json::json!({ "property_id": pid, "name": "Pine Ridge", "kind": "campground", "base_layer": "grid" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{camp}");
    let cid = camp["id"].as_str().unwrap().to_string();
    let site = |name: &str, lng: f64| {
        serde_json::json!({ "kind": "site", "name": name,
            "geometry": { "type": "Point", "coordinates": [lng, 34.5] },
            "attrs": { "site_type": "rv", "power_amps": 50, "water": true, "max_length_ft": 40,
                       "pull_through": true, "rate_cents_night": 4500 } })
    };
    let (st, camp) = send_json(
        c,
        Method::Put,
        &format!("/site-maps/{cid}/features"),
        &mgr,
        serde_json::json!({ "features": [
            site("A1", -117.30), site("A2", -117.299),
            { "kind": "road", "name": "Loop", "geometry": { "type": "LineString", "coordinates": [[-117.3, 34.5], [-117.299, 34.5]] } },
            { "kind": "amenity", "name": "Showers", "geometry": { "type": "Point", "coordinates": [-117.3, 34.501] },
              "attrs": { "amenity": "bath_house" } },
        ]}),
    )
    .await;
    assert_eq!(st, Status::Ok, "{camp}");
    assert_eq!(camp["stats"]["sites"], 2);
    assert!(camp["features"][2]["length_m"].as_f64().unwrap() > 50.0);
    assert_eq!(
        bad_attrs(c, &mgr, &cid).await,
        Status::BadRequest,
        "site_type must be known"
    );

    // Export carries OSM tags; importing it into a fresh map reproduces the sites.
    let (st, gj) = get_json(c, &format!("/site-maps/{cid}/export.geojson"), &mgr).await;
    assert_eq!(st, Status::Ok);
    let first = &gj["features"][0]["properties"];
    assert_eq!(first["tourism"], "camp_pitch");
    assert_eq!(first["power_supply:amperage"], 50);
    let (_, copy) = post_json(
        c,
        "/site-maps",
        &mgr,
        serde_json::json!({ "property_id": pid, "name": "Copy", "kind": "campground" }),
    )
    .await;
    let (st, copy) = post_json(
        c,
        &format!("/site-maps/{}/import", copy["id"].as_str().unwrap()),
        &mgr,
        serde_json::json!({ "geojson": gj }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{copy}");
    assert_eq!(copy["stats"]["sites"], 2);
    assert_eq!(copy["features"].as_array().unwrap().len(), 4);
    // Raw OSM: a pitch is understood, a bakery is skipped.
    let (st, osm) = post_json(
        c,
        &format!("/site-maps/{}/import", copy["id"].as_str().unwrap()),
        &mgr,
        serde_json::json!({ "replace": true, "geojson": { "type": "FeatureCollection", "features": [
            { "type": "Feature", "properties": { "tourism": "camp_pitch", "name": "P1", "capacity": "4" },
              "geometry": { "type": "Point", "coordinates": [-117.3, 34.5] } },
            { "type": "Feature", "properties": { "shop": "bakery" },
              "geometry": { "type": "Point", "coordinates": [-117.3, 34.5] } } ] } }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{osm}");
    assert_eq!(osm["features"].as_array().unwrap().len(), 1);
    assert_eq!(osm["features"][0]["attrs"]["max_guests"], 4.0);

    // Publishing: private until published; the public view hides internal ids.
    let public = |path: String| async move {
        let resp = c
            .client
            .get(path)
            .header(Header::new("X-Tenant", "northwind"))
            .dispatch()
            .await;
        let st = resp.status();
        (
            st,
            resp.into_json::<serde_json::Value>()
                .await
                .unwrap_or_default(),
        )
    };
    let (st, _) = public(format!("/public/site-maps/{mid}")).await;
    assert_eq!(st, Status::NotFound);
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/site-maps/{mid}"),
        &mgr,
        serde_json::json!({ "published": true }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    let (st, pm) = public(format!("/public/site-maps/{mid}")).await;
    assert_eq!(st, Status::Ok, "{pm}");
    let pf = pm["features"].as_array().unwrap();
    assert!(pf.iter().all(|f| f["unit_id"].is_null()));
    let statuses: Vec<&str> = pf
        .iter()
        .filter_map(|f| f["unit"]["status"].as_str())
        .collect();
    assert!(
        statuses.contains(&"vacant") && statuses.contains(&"unavailable"),
        "{statuses:?}"
    );
    let (_, list) = public(format!("/public/site-maps?property_id={pid}")).await;
    assert_eq!(list.as_array().unwrap().len(), 1);

    // Another workspace cannot see or edit them.
    let other = mint(
        c,
        Some(cascade),
        false,
        &["property:read", "property:write"],
    );
    let (st, _) = get_json(c, &format!("/site-maps/{mid}"), &other).await;
    assert_eq!(st, Status::NotFound);
    let (st, l) = get_json(c, "/site-maps", &other).await;
    assert_eq!(st, Status::Ok);
    assert!(l.as_array().unwrap().is_empty());

    // The drawing is in the property's history.
    let (_, hist) = get_json(c, &format!("/properties/{pid}/history?limit=100"), &mgr).await;
    let h = hist["events"].as_array().unwrap();
    assert!(h.iter().any(
        |e| e["action"] == "site_map.draw" && e["summary"].as_str().unwrap().contains("added")
    ));
    assert!(h.iter().any(|e| e["action"] == "site_map.create"));

    // Delete.
    let (st, _) = send_json(
        c,
        Method::Delete,
        &format!("/site-maps/{cid}"),
        &mgr,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Ok);
    let (st, _) = get_json(c, &format!("/site-maps/{cid}"), &mgr).await;
    assert_eq!(st, Status::NotFound);
}

async fn bad_attrs(c: &Ctx, token: &str, map_id: &str) -> Status {
    send_json(
        c,
        rocket::http::Method::Put,
        &format!("/site-maps/{map_id}/features"),
        token,
        serde_json::json!({ "features": [{ "kind": "site",
            "geometry": { "type": "Point", "coordinates": [0, 0] },
            "attrs": { "site_type": "yurt" } }] }),
    )
    .await
    .0
}

/// Public search and tour requests, then autofill review on a property.
async fn public_search_tours_and_autofill(c: &Ctx) {
    use rocket::http::Method;
    let nw = tenant_id(c, "northwind").await;
    let cascade = tenant_id(c, "cascade").await;
    let get_pub = |path: String| async move {
        let resp = c
            .client
            .get(path)
            .header(Header::new("X-Tenant", "northwind"))
            .dispatch()
            .await;
        let st = resp.status();
        (
            st,
            resp.into_json::<serde_json::Value>()
                .await
                .unwrap_or_default(),
        )
    };

    // Search: the full list, then narrowed and sorted.
    let (st, all) = get_pub("/public/listings".into()).await;
    assert_eq!(st, Status::Ok);
    let all = all.as_array().unwrap().clone();
    assert!(!all.is_empty(), "the demo has public listings");
    let (_, cheap_first) = get_pub("/public/listings?sort=price_asc".into()).await;
    let rents: Vec<i64> = cheap_first
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["rent_cents"].as_i64().unwrap())
        .collect();
    assert!(rents.windows(2).all(|w| w[0] <= w[1]), "{rents:?}");
    let top = *rents.last().unwrap();
    let (_, under) = get_pub(format!("/public/listings?max_rent={}", top / 100 - 1)).await;
    assert!(under.as_array().unwrap().len() < all.len());
    let (_, none) = get_pub("/public/listings?q=zzzz-no-such-place".into()).await;
    assert!(none.as_array().unwrap().is_empty());

    // A tour request: validation, honeypot, then a real one.
    let listing_id = all[0]["id"].as_str().unwrap().to_string();
    let post_pub = |body: serde_json::Value| async move {
        let resp = c
            .client
            .post("/public/tour-requests")
            .header(ContentType::JSON)
            .header(Header::new("X-Tenant", "northwind"))
            .body(body.to_string())
            .dispatch()
            .await;
        resp.status()
    };
    let ok = serde_json::json!({ "listing_id": listing_id, "name": "Pat Prospect",
        "email": "pat@example.com", "phone": "555-0111",
        "preferred_times": "Sat morning", "consent": true });
    assert_eq!(
        post_pub(serde_json::json!({ "name": "x", "email": "nope", "consent": true })).await,
        Status::BadRequest
    );
    assert_eq!(
        post_pub(serde_json::json!({ "name": "x", "email": "a@b.co", "consent": false })).await,
        Status::BadRequest
    );
    let mut bot = ok.clone();
    bot["website"] = "http://spam.example".into();
    assert_eq!(post_pub(bot).await, Status::Ok);
    assert_eq!(post_pub(ok).await, Status::Ok);

    let lead = mint(
        c,
        Some(nw),
        false,
        &["application:read", "application:write"],
    );
    let (st, tours) = get_json(c, "/tour-requests", &lead).await;
    assert_eq!(st, Status::Ok, "{tours}");
    let tours = tours.as_array().unwrap();
    assert_eq!(tours.len(), 1, "the honeypot request was not stored");
    assert_eq!(tours[0]["name"], "Pat Prospect");
    assert_eq!(tours[0]["status"], "new");
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/tour-requests/{}", tours[0]["id"].as_str().unwrap()),
        &lead,
        serde_json::json!({ "status": "scheduled", "note": "Saturday 10am" }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    let (_, sched) = get_json(c, "/tour-requests?status=scheduled", &lead).await;
    assert_eq!(sched.as_array().unwrap().len(), 1);
    let other = mint(c, Some(cascade), false, &["application:read"]);
    let (_, theirs) = get_json(c, "/tour-requests", &other).await;
    assert!(theirs.as_array().unwrap().is_empty());
    let nobody = mint(c, Some(nw), false, &["property:read"]);
    let (st, _) = get_json(c, "/tour-requests", &nobody).await;
    assert_eq!(st, Status::Forbidden);

    // Autofill: a one-unit house whose record says something different.
    let mgr = mint(
        c,
        Some(nw),
        false,
        &[
            "property:read",
            "property:write",
            "lease:manage",
            "audit:read",
        ],
    );
    let (st, p) = post_json(
        c,
        "/properties",
        &mgr,
        serde_json::json!({ "name": "Autofill House", "address": "9 Fill St", "city": "Boise",
            "units": 1, "occupied_units": 0, "monthly_rent_cents": 0, "property_type": "multi_family" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{p}");
    let pid = p["id"].as_str().unwrap().to_string();
    let puid = uuid::Uuid::parse_str(&pid).unwrap();
    let (_, unit) = post_json(
        c,
        &format!("/properties/{pid}/units"),
        &mgr,
        serde_json::json!({ "unit_number": "1", "beds": 2, "status": "vacant" }),
    )
    .await;
    assert!(unit["id"].is_string(), "{unit}");

    // Before the record is fetched there is nothing to propose.
    let (st, none) = get_json(c, &format!("/properties/{pid}/autofill"), &mgr).await;
    assert_eq!(st, Status::Ok, "{none}");
    // Make sure the detail row says what we want (the job may or may not have run).
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
    let existing = entity::prelude::PropertyDetail::find()
        .filter(entity::property_detail::Column::PropertyId.eq(puid))
        .one(&c.db)
        .await
        .unwrap();
    let now = chrono::Utc::now();
    match existing {
        Some(d) => {
            let mut am: entity::property_detail::ActiveModel = d.into();
            am.property_type = Set(Some("Single Family Residence".into()));
            am.beds = Set(Some(3));
            am.baths = Set(Some(2.5));
            am.sqft = Set(Some(1400));
            am.last_enriched_at = Set(Some(now.into()));
            am.update(&c.db).await.unwrap();
        }
        None => {
            entity::property_detail::ActiveModel {
                property_id: Set(puid),
                tenant_id: Set(nw),
                property_type: Set(Some("Single Family Residence".into())),
                beds: Set(Some(3)),
                baths: Set(Some(2.5)),
                sqft: Set(Some(1400)),
                last_enriched_at: Set(Some(now.into())),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
                ..Default::default()
            }
            .insert(&c.db)
            .await
            .unwrap();
        }
    }
    let (st, ps) = get_json(c, &format!("/properties/{pid}/autofill"), &mgr).await;
    assert_eq!(st, Status::Ok, "{ps}");
    let fields: Vec<&str> = ps["proposals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["field"].as_str().unwrap())
        .collect();
    assert_eq!(
        fields,
        vec!["property_type", "unit_beds", "unit_baths", "unit_sqft"],
        "{ps}"
    );

    // Apply the type and square footage only; the rest stay proposed.
    let (st, after) = post_json(
        c,
        &format!("/properties/{pid}/autofill/apply"),
        &mgr,
        serde_json::json!({ "fields": ["property_type", "unit_sqft"] }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{after}");
    let left: Vec<&str> = after["proposals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["field"].as_str().unwrap())
        .collect();
    assert_eq!(left, vec!["unit_beds", "unit_baths"]);
    let (_, prop) = get_json(c, &format!("/properties/{pid}"), &mgr).await;
    assert_eq!(prop["property_type"], "single_family");
    let (st, _) = post_json(
        c,
        &format!("/properties/{pid}/autofill/apply"),
        &mgr,
        serde_json::json!({ "fields": ["nonsense"] }),
    )
    .await;
    assert_eq!(st, Status::BadRequest);
    let (_, hist) = get_json(c, &format!("/properties/{pid}/history?limit=50"), &mgr).await;
    assert!(hist["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["action"] == "property.autofill_apply"));
}

/// Alpha single sign-on: off by default, a signed assertion signs the right
/// person in once, and everything else is refused the same way.
async fn alpha_single_sign_on(c: &Ctx) {
    use rocket::http::Method;
    let nw = tenant_id(c, "northwind").await;
    let admin = mint(
        c,
        Some(nw),
        false,
        &["integrations:manage", "maintenance:read", "audit:read"],
    );
    let now = chrono::Utc::now().timestamp();
    let assertion = |secret: &str, email: &str, tenant: &str, ttl: i64| {
        let cl = crate::sso::claims("alpha", "vantedge", email, tenant, None, now, ttl);
        crate::sso::sign(secret, &cl).unwrap()
    };
    let try_in = |token: String| async move {
        let resp = c
            .client
            .post("/auth/sso/alpha")
            .header(ContentType::JSON)
            .body(serde_json::json!({ "token": token }).to_string())
            .dispatch()
            .await;
        let st = resp.status();
        (
            st,
            resp.into_json::<serde_json::Value>()
                .await
                .unwrap_or_default(),
        )
    };

    // Off by default.
    let (st, s) = get_json(c, "/sso/alpha", &admin).await;
    assert_eq!(st, Status::Ok, "{s}");
    assert_eq!(s["enabled"], false);
    let (st, _) = try_in(assertion("guess", "jordan@northwind.com", "northwind", 60)).await;
    assert_eq!(st, Status::Unauthorized);

    // Turn it on; the secret shows once.
    let nobody = mint(c, Some(nw), false, &["maintenance:read"]);
    let (st, _) = post_json(c, "/sso/alpha/enable", &nobody, serde_json::json!({})).await;
    assert_eq!(st, Status::Forbidden);
    let (st, on) = post_json(c, "/sso/alpha/enable", &admin, serde_json::json!({})).await;
    assert_eq!(st, Status::Ok, "{on}");
    let secret = on["secret"].as_str().unwrap().to_string();
    assert_eq!(on["enabled"], true);
    assert_eq!(on["tenant"], "northwind");
    let (st, _) = post_json(c, "/sso/alpha/enable", &admin, serde_json::json!({})).await;
    assert_eq!(st, Status::Conflict, "rotating is explicit");
    let (_, again) = get_json(c, "/sso/alpha", &admin).await;
    assert!(
        again.get("secret").is_none(),
        "the secret is not shown again"
    );

    // A good assertion signs Jordan in, once.
    let good = assertion(&secret, "Jordan@Northwind.com", "northwind", 60);
    let (st, r) = try_in(good.clone()).await;
    assert_eq!(st, Status::Ok, "{r}");
    assert_eq!(r["outcome"], "session", "{r}");
    assert!(r["session"]["access_token"].is_string() || r["session"].is_object());
    let (st, _) = try_in(good).await;
    assert_eq!(st, Status::Unauthorized, "a token works once");

    // Everything else is the same 401.
    for (label, t) in [
        (
            "wrong secret",
            assertion("wrong", "jordan@northwind.com", "northwind", 60),
        ),
        (
            "unknown person",
            assertion(&secret, "nobody@nowhere.example", "northwind", 60),
        ),
        (
            "a member of another workspace",
            assertion(&secret, "priya@cascade.com", "northwind", 60),
        ),
        (
            "another workspace",
            assertion(&secret, "jordan@northwind.com", "cascade", 60),
        ),
        ("junk", "not.a.token".to_string()),
    ] {
        let (st, _) = try_in(t).await;
        assert_eq!(st, Status::Unauthorized, "{label}");
    }
    let mut long = crate::sso::claims(
        "alpha",
        "vantedge",
        "jordan@northwind.com",
        "northwind",
        None,
        now,
        60,
    );
    long.exp = now + 3600;
    let (st, _) = try_in(crate::sso::sign(&secret, &long).unwrap()).await;
    assert_eq!(st, Status::Unauthorized, "too long-lived");

    // Launch: needs a linked vendor and the web address, and a real person.
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
    let jordan = entity::prelude::User::find()
        .filter(entity::user::Column::Email.eq("jordan@northwind.com"))
        .one(&c.db)
        .await
        .unwrap()
        .unwrap();
    let admin = crate::auth::issue_access_token(
        &c.config,
        jordan.id,
        Some(nw),
        false,
        vec![
            "integrations:manage".into(),
            "maintenance:read".into(),
            "audit:read".into(),
        ],
    )
    .unwrap();
    let cp = entity::prelude::Counterparty::find()
        .filter(entity::counterparty::Column::TenantId.eq(nw))
        .one(&c.db)
        .await
        .unwrap()
        .expect("a vendor exists");
    let (st, _) = post_json(
        c,
        "/sso/alpha/launch",
        &admin,
        serde_json::json!({ "counterparty_id": cp.id, "web_url": "https://alpha.example" }),
    )
    .await;
    assert_eq!(st, Status::Conflict, "not linked yet");
    let mut am: entity::counterparty::ActiveModel = cp.clone().into();
    am.partner_kind = Set(Some("alpha".into()));
    am.update(&c.db).await.unwrap();
    let (st, _) = post_json(
        c,
        "/sso/alpha/launch",
        &admin,
        serde_json::json!({ "counterparty_id": cp.id }),
    )
    .await;
    assert_eq!(st, Status::BadRequest, "needs the web address");
    let (st, _) = post_json(
        c,
        "/sso/alpha/launch",
        &admin,
        serde_json::json!({ "counterparty_id": cp.id, "web_url": "javascript:x" }),
    )
    .await;
    assert_eq!(st, Status::BadRequest);
    let (st, l) = post_json(
        c,
        "/sso/alpha/launch",
        &admin,
        serde_json::json!({ "counterparty_id": cp.id, "web_url": "https://alpha.example/", "next": "/portal/admin/jobs" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{l}");
    let url = l["url"].as_str().unwrap();
    assert!(
        url.starts_with("https://alpha.example/sso/vantedge?token="),
        "{url}"
    );
    assert!(url.ends_with("&next=/portal/admin/jobs"), "{url}");
    // The token in the link verifies with the shared secret, for Alpha.
    let tok = url
        .split("token=")
        .nth(1)
        .unwrap()
        .split('&')
        .next()
        .unwrap();
    let cl = crate::sso::verify(tok, &secret, "vantedge", "alpha", now).unwrap();
    assert_eq!(cl.tenant, "northwind");
    assert_eq!(cl.next.as_deref(), Some("/portal/admin/jobs"));
    // Remembered for next time.
    let (st, _) = post_json(
        c,
        "/sso/alpha/launch",
        &admin,
        serde_json::json!({ "counterparty_id": cp.id }),
    )
    .await;
    assert_eq!(st, Status::Ok);

    // Turn it off: the same kind of token stops working.
    let (st, off) = send_json(
        c,
        Method::Delete,
        "/sso/alpha",
        &admin,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Ok);
    assert_eq!(off["enabled"], false);
    let (st, _) = try_in(assertion(&secret, "jordan@northwind.com", "northwind", 60)).await;
    assert_eq!(st, Status::Unauthorized);

    let (_, trail) = get_json(c, "/audit/events?limit=100", &admin).await;
    let acts: Vec<&str> = trail["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["action"].as_str())
        .collect();
    for want in ["sso.enable", "auth.sso_login", "sso.launch", "sso.disable"] {
        assert!(acts.contains(&want), "missing {want}: {acts:?}");
    }
}

/// Website embeds: the allowed sites are normalised and validated, and the
/// public config says whether and where the widgets may be shown.
async fn embed_settings(c: &Ctx) {
    use rocket::http::Method;
    let nw = tenant_id(c, "northwind").await;
    let admin = mint(c, Some(nw), false, &["integrations:manage"]);
    let put = |body: serde_json::Value| {
        let admin = admin.clone();
        async move { send_json(c, Method::Put, "/business-profile", &admin, body).await }
    };
    async fn cfg(c: &Ctx, slug: &'static str) -> serde_json::Value {
        let resp = c
            .client
            .get("/public/embed-config")
            .header(Header::new("X-Tenant", slug))
            .dispatch()
            .await;
        resp.into_json::<serde_json::Value>().await.unwrap()
    }
    // Default: on, any site.
    let before = cfg(c, "northwind").await;
    assert_eq!(before["enabled"], true);

    for bad in [
        "*.example.com",
        "https://example.com/page",
        "http://example.com",
    ] {
        let (st, _) = put(serde_json::json!({ "embed_origins": bad })).await;
        assert_eq!(st, Status::BadRequest, "{bad}");
    }
    let (st, p) =
        put(serde_json::json!({ "embed_origins": "Example.com\nhttps://www.example.com/" })).await;
    assert_eq!(st, Status::Ok, "{p}");
    assert_eq!(
        p["embed_origins"],
        "https://example.com\nhttps://www.example.com"
    );
    let on = cfg(c, "northwind").await;
    assert_eq!(
        on["allowed_origins"],
        serde_json::json!(["https://example.com", "https://www.example.com"])
    );

    let (st, _) = put(serde_json::json!({ "embed_enabled": false })).await;
    assert_eq!(st, Status::Ok);
    assert_eq!(cfg(c, "northwind").await["enabled"], false);
    // Another workspace is unaffected.
    let other = cfg(c, "cascade").await;
    assert_eq!(other["enabled"], true);
    assert!(other["allowed_origins"].as_array().unwrap().is_empty());

    // Put it back for the tests that follow.
    let (st, _) = put(serde_json::json!({ "embed_enabled": true, "embed_origins": "" })).await;
    assert_eq!(st, Status::Ok);
}

/// Search appearance: the public site info carries the business, the SEO
/// fields validate, and an unknown host does not resolve.
async fn seo_site_info(c: &Ctx) {
    use rocket::http::Method;
    let nw = tenant_id(c, "northwind").await;
    let admin = mint(c, Some(nw), false, &["integrations:manage"]);
    let put = |body: serde_json::Value| {
        let admin = admin.clone();
        async move { send_json(c, Method::Put, "/business-profile", &admin, body).await }
    };
    let (st, _) = put(serde_json::json!({ "seo_title": "x".repeat(71) })).await;
    assert_eq!(st, Status::BadRequest);
    let (st, _) = put(serde_json::json!({ "google_site_verification": "<script>" })).await;
    assert_eq!(st, Status::BadRequest);
    let (st, p) = put(serde_json::json!({
        "business_name": "Northwind Rentals",
        "seo_title": "Homes for rent in Portland | Northwind",
        "seo_description": "Browse verified rentals managed by Northwind.",
        "google_site_verification": "abc-123_XYZ",
        "facebook_url": "https://facebook.com/northwind",
    }))
    .await;
    assert_eq!(st, Status::Ok, "{p}");

    let get = |path: String| async move {
        let resp = c
            .client
            .get(path)
            .header(Header::new("X-Tenant", "northwind"))
            .dispatch()
            .await;
        let st = resp.status();
        (
            st,
            resp.into_json::<serde_json::Value>()
                .await
                .unwrap_or_default(),
        )
    };
    let (st, site) = get("/public/site".into()).await;
    assert_eq!(st, Status::Ok, "{site}");
    assert_eq!(site["slug"], "northwind");
    assert_eq!(site["company_name"], "Northwind Rentals");
    assert_eq!(site["seo_title"], "Homes for rent in Portland | Northwind");
    assert_eq!(site["google_site_verification"], "abc-123_XYZ");
    assert_eq!(
        site["same_as"],
        serde_json::json!(["https://facebook.com/northwind"])
    );

    // Listings carry when they were listed (for sitemaps).
    let (_, ls) = get("/public/listings".into()).await;
    assert!(ls[0]["listed_at"].as_str().unwrap().contains('T'));

    let (st, _) = get("/public/resolve?host=nowhere.example".into()).await;
    assert_eq!(st, Status::NotFound);
}

/// Fix plan batch A: list limits, the job schedule, and reminders that run
/// themselves (each sent once), with the inspection calendar file.
async fn batch_a_limits_jobs_and_reminders(c: &Ctx) {
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
    let nw = tenant_id(c, "northwind").await;
    let cascade = tenant_id(c, "cascade").await;
    let admin = mint(
        c,
        Some(nw),
        false,
        &[
            "tenant:manage",
            "application:read",
            "property:read",
            "property:write",
            "lease:read",
            "lease:manage",
            "maintenance:manage",
            "maintenance:read",
        ],
    );

    // F1: limits and a cursor on the applications list.
    let (st, all) = get_json(c, "/applications", &admin).await;
    assert_eq!(st, Status::Ok);
    let all = all.as_array().unwrap().clone();
    if all.len() >= 2 {
        let (_, one) = get_json(c, "/applications?limit=1", &admin).await;
        assert_eq!(one.as_array().unwrap().len(), 1);
        let cursor = one[0]["created_at"].as_str().unwrap().to_string();
        let (st, next) = get_json(
            c,
            &format!("/applications?limit=1&before={}", urlencode(&cursor)),
            &admin,
        )
        .await;
        assert_eq!(st, Status::Ok, "{next}");
        assert_ne!(next[0]["id"], one[0]["id"]);
    }
    let (st, _) = get_json(c, "/applications?before=yesterday", &admin).await;
    assert_eq!(st, Status::BadRequest);

    // F2: the schedule shows this workspace's recurring jobs; run-now.
    crate::resident_reminders::ensure_job(&c.db, nw, crate::resident_reminders::KIND)
        .await
        .unwrap();
    crate::resident_reminders::ensure_job(&c.db, nw, crate::resident_reminders::KIND)
        .await
        .unwrap();
    let (st, sched) = get_json(c, "/admin/jobs/schedule", &admin).await;
    assert_eq!(st, Status::Ok, "{sched}");
    let job = sched
        .as_array()
        .unwrap()
        .iter()
        .find(|j| j["kind"] == "resident_reminders")
        .expect("the reminders job is scheduled")
        .clone();
    assert!(job["label"].as_str().unwrap().contains("Rent due"));
    let jid = job["id"].as_str().unwrap().to_string();
    let live = entity::prelude::BackgroundJob::find()
        .filter(entity::background_job::Column::TenantId.eq(nw))
        .filter(entity::background_job::Column::Kind.eq("resident_reminders"))
        .all(&c.db)
        .await
        .unwrap();
    assert_eq!(live.len(), 1, "ensuring twice keeps one job");
    let (st, ran) = post_json(
        c,
        &format!("/admin/jobs/{jid}/run-now"),
        &admin,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Ok, "{ran}");
    let other = mint(c, Some(cascade), false, &["tenant:manage"]);
    let (st, _) = post_json(
        c,
        &format!("/admin/jobs/{jid}/run-now"),
        &other,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::NotFound);
    let (st, theirs) = get_json(c, "/admin/jobs?kind=resident_reminders", &other).await;
    assert_eq!(st, Status::Ok);
    assert!(theirs.as_array().unwrap().is_empty());
    let nobody = mint(c, Some(nw), false, &["property:read"]);
    let (st, _) = get_json(c, "/admin/jobs/schedule", &nobody).await;
    assert_eq!(st, Status::Forbidden);

    // A lease to remind about.
    let pid = property_ids(c, nw).await[0];
    let today = chrono::Utc::now().date_naive();
    let end = today + chrono::Duration::days(85);
    let (st, lease) = post_json(
        c,
        &format!("/properties/{pid}/leases"),
        &admin,
        serde_json::json!({
            "tenant_name": "Rita Reminder",
            "tenant_email": "rita.reminder@example.com",
            "rent_cents": 150000,
            "start_date": (today - chrono::Duration::days(200)).to_string(),
            "end_date": end.to_string(),
            "status": "active",
        }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{lease}");
    let lid = uuid::Uuid::parse_str(lease["id"].as_str().unwrap()).unwrap();
    let emails = |template: &'static str| async move {
        entity::prelude::BackgroundJob::find()
            .filter(entity::background_job::Column::TenantId.eq(nw))
            .filter(entity::background_job::Column::Kind.eq("auto_email"))
            .all(&c.db)
            .await
            .unwrap()
            .into_iter()
            .filter(|j| {
                j.payload["template"] == template && j.payload["to"] == "rita.reminder@example.com"
            })
            .count()
    };

    // F4: rent due, two days before the rent day; once only.
    let due_day = crate::settings::get_i64(&c.db, nw, crate::settings::PAYMENTS_RENT_DUE_DAY).await;
    let next_due =
        crate::resident_reminders::next_due_date(today + chrono::Duration::days(1), due_day);
    let two_before = next_due - chrono::Duration::days(2);
    crate::resident_reminders::rent_due(&c.db, nw, two_before)
        .await
        .unwrap();
    assert_eq!(emails("rent_due").await, 1);
    crate::resident_reminders::rent_due(&c.db, nw, two_before)
        .await
        .unwrap();
    assert_eq!(emails("rent_due").await, 1, "never twice");

    // F4: rent past due, the day after; once only.
    let yesterday = today - chrono::Duration::days(1);
    entity::lease_payment::ActiveModel {
        id: Set(uuid::Uuid::new_v4()),
        tenant_id: Set(nw),
        lease_id: Set(lid),
        due_date: Set(yesterday.to_string()),
        amount_cents: Set(150000),
        paid_date: Set(None),
        status: Set("due".into()),
        method: Set(None),
        created_at: Set(chrono::Utc::now().into()),
        kind: Set("rent".into()),
        method_id: Set(None),
        provider: Set(None),
        external_id: Set(None),
        failure_reason: Set(None),
        receipt_number: Set(None),
        ledger_txn_id: Set(None),
    }
    .insert(&c.db)
    .await
    .unwrap();
    crate::resident_reminders::rent_past_due(&c.db, nw, today)
        .await
        .unwrap();
    crate::resident_reminders::rent_past_due(&c.db, nw, today)
        .await
        .unwrap();
    assert_eq!(emails("rent_past_due").await, 1);

    // F6: 85 days out → one proposed renewal at the current rent, once.
    crate::resident_reminders::lease_expiry(&c.db, nw, today)
        .await
        .unwrap();
    crate::resident_reminders::lease_expiry(&c.db, nw, today)
        .await
        .unwrap();
    let renewals = entity::prelude::LeaseRenewal::find()
        .filter(entity::lease_renewal::Column::LeaseId.eq(lid))
        .all(&c.db)
        .await
        .unwrap();
    assert_eq!(renewals.len(), 1);
    assert_eq!(renewals[0].status, "proposed", "drafted, not sent");
    assert_eq!(renewals[0].new_rent_cents, 150000);
    assert!(renewals[0].lease_document_id.is_some());
    assert!(
        emails("lease_renewal_sent").await == 0,
        "nothing reaches the resident from the reminder"
    );

    // F7: an inspection tomorrow → a reminder with a working calendar link.
    let (st, insp) = post_json(
        c,
        &format!("/leases/{lid}/inspections"),
        &admin,
        serde_json::json!({ "kind": "move_out", "scheduled_date": (today + chrono::Duration::days(1)).to_string() }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{insp}");
    let iid = insp["id"].as_str().unwrap().to_string();
    crate::resident_reminders::inspections(&c.db, nw, today)
        .await
        .unwrap();
    crate::resident_reminders::inspections(&c.db, nw, today)
        .await
        .unwrap();
    assert_eq!(emails("inspection_reminder").await, 1);
    let sig = crate::resident_reminders::calendar_sig(uuid::Uuid::parse_str(&iid).unwrap());
    let resp = c
        .client
        .get(format!("/public/inspections/{iid}/calendar.ics?sig={sig}"))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    assert!(resp
        .headers()
        .get_one("Content-Type")
        .unwrap_or("")
        .starts_with("text/calendar"));
    let body = resp.into_string().await.unwrap();
    assert!(
        body.contains("BEGIN:VEVENT") && body.contains("Move-out inspection"),
        "{body}"
    );
    let resp = c
        .client
        .get(format!(
            "/public/inspections/{iid}/calendar.ics?sig=00000000000000000000000000000000"
        ))
        .dispatch()
        .await;
    assert_eq!(
        resp.status(),
        Status::NotFound,
        "a guessed link does not work"
    );
    let resp = c
        .client
        .get(format!("/inspections/{iid}/calendar.ics"))
        .header(bearer(&admin))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);

    // F8: a warranty ending in 20 days notifies staff once.
    let (st, asset) = post_json(
        c,
        "/assets",
        &admin,
        serde_json::json!({ "property_id": pid, "name": "Reminder water heater", "kind": "plumbing",
            "warranty_expires": (today + chrono::Duration::days(20)).to_string() }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{asset}");
    let n1 = crate::resident_reminders::warranties(&c.db, nw, today)
        .await
        .unwrap();
    let n2 = crate::resident_reminders::warranties(&c.db, nw, today)
        .await
        .unwrap();
    assert!(n1 >= 1);
    assert_eq!(n2, 0);

    // F9: the digest counts what needs attention, and sends once a day.
    let d = crate::resident_reminders::compute_digest(&c.db, nw, today)
        .await
        .unwrap();
    assert!(!d.is_empty());
    let first = crate::resident_reminders::send_digest(&c.db, nw, today)
        .await
        .unwrap();
    assert_eq!(first["sent"], true, "{first}");
    let second = crate::resident_reminders::send_digest(&c.db, nw, today)
        .await
        .unwrap();
    assert_eq!(second["sent"], false);

    // The reminder-drafted renewal is in the audit trail as automatic.
    let logged = entity::prelude::AuditLog::find()
        .filter(entity::audit_log::Column::TenantId.eq(nw))
        .filter(entity::audit_log::Column::Action.eq("lease_renewal.propose"))
        .all(&c.db)
        .await
        .unwrap();
    assert!(logged
        .iter()
        .any(|r| r.metadata.as_ref().is_some_and(|m| m["automatic"] == true)));
}

fn urlencode(s: &str) -> String {
    s.replace('+', "%2B").replace(':', "%3A")
}

async fn batch_b_vendor_compliance(c: &Ctx) {
    use rocket::http::Method;
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
    let nw = tenant_id(c, "northwind").await;
    let cascade = tenant_id(c, "cascade").await;
    let admin = mint(
        c,
        Some(nw),
        false,
        &[
            "tenant:manage",
            "entity:read",
            "entity:manage",
            "maintenance:manage",
            "maintenance:read",
            "report:read",
            "property:read",
        ],
    );
    let today = chrono::Utc::now().date_naive();

    let (st, v) = post_json(
        c,
        "/entities",
        &admin,
        serde_json::json!({ "kind": "contractor", "name": "Compliance Plumbing",
            "email": "office@complianceplumbing.example" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{v}");
    let vid = v["id"].as_str().unwrap().to_string();

    // Nothing on file yet: both problems show, and the vendor is on the list.
    let (st, comp) = get_json(c, &format!("/entities/{vid}/compliance"), &admin).await;
    assert_eq!(st, Status::Ok, "{comp}");
    assert!(comp["w9"].is_null());
    assert_eq!(comp["coi_current"], false);
    assert_eq!(comp["problems"].as_array().unwrap().len(), 2);
    let (_, list) = get_json(c, "/compliance/vendors", &admin).await;
    assert!(list
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["counterparty_id"] == vid));

    // F11: the W-9. A bad TIN is refused; a good one is stored, masked.
    let w9 = |tin: &str| {
        serde_json::json!({ "legal_name": "Compliance Plumbing LLC", "classification": "llc_p",
            "tin_type": "ein", "tin": tin, "signed_on": today.to_string() })
    };
    let (st, _) = send_json(
        c,
        Method::Put,
        &format!("/entities/{vid}/w9"),
        &admin,
        w9("00-1234567"),
    )
    .await;
    assert_eq!(st, Status::BadRequest, "00 is not an IRS prefix");
    let (st, comp) = send_json(
        c,
        Method::Put,
        &format!("/entities/{vid}/w9"),
        &admin,
        w9("27-1234567"),
    )
    .await;
    assert_eq!(st, Status::Ok, "{comp}");
    assert_eq!(comp["w9"]["tin_masked"], "••-•••4567");
    let text = comp.to_string();
    assert!(!text.contains("271234567") && !text.contains("27-1234567"));
    let row = entity::prelude::VendorTaxProfile::find()
        .filter(entity::vendor_tax_profile::Column::TenantId.eq(nw))
        .one(&c.db)
        .await
        .unwrap()
        .unwrap();
    assert!(
        !row.tin_ciphertext.contains("271234567"),
        "stored encrypted"
    );
    let trail = entity::prelude::AuditLog::find()
        .filter(entity::audit_log::Column::Action.eq(crate::audit::actions::VENDOR_W9_SAVE))
        .all(&c.db)
        .await
        .unwrap();
    assert!(!trail.is_empty());
    assert!(trail.iter().all(|a| !serde_json::to_string(&a.metadata)
        .unwrap()
        .contains("271234567")));
    // Another workspace can't see or write it.
    let other = mint(c, Some(cascade), false, &["entity:read", "entity:manage"]);
    let (st, _) = get_json(c, &format!("/entities/{vid}/compliance"), &other).await;
    assert_eq!(st, Status::NotFound);
    let (st, _) = send_json(
        c,
        Method::Put,
        &format!("/entities/{vid}/w9"),
        &other,
        w9("27-1234567"),
    )
    .await;
    assert_eq!(st, Status::NotFound);

    // F11: the 1099 shows the masked TIN; the export has the full one.
    let llc = entity::prelude::Llc::find()
        .filter(entity::llc::Column::TenantId.eq(nw))
        .one(&c.db)
        .await
        .unwrap()
        .expect("a seeded entity");
    let now = chrono::Utc::now();
    entity::vendor_bill::ActiveModel {
        id: Set(uuid::Uuid::new_v4()),
        tenant_id: Set(nw),
        entity_id: Set(llc.id),
        counterparty_id: Set(uuid::Uuid::parse_str(&vid).unwrap()),
        property_id: Set(None),
        maintenance_ticket_id: Set(None),
        bill_number: Set(format!("BILL-COMP-{}", &vid[..8])),
        memo: Set("Repipe".into()),
        line_items: Set(serde_json::json!([])),
        amount_cents: Set(250_000),
        due_date: Set(None),
        status: Set("paid".into()),
        submitted_by: Set(None),
        submitted_at: Set(None),
        approved_by: Set(None),
        approved_at: Set(None),
        rejected_reason: Set(None),
        provider: Set(None),
        external_id: Set(None),
        accrual_txn_id: Set(None),
        payment_txn_id: Set(None),
        failure_reason: Set(None),
        paid_at: Set(Some(now.into())),
        created_by: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&c.db)
    .await
    .unwrap();
    use chrono::Datelike;
    let year = today.year();
    let (st, r) = get_json(c, &format!("/reports/1099?year={year}"), &admin).await;
    assert_eq!(st, Status::Ok, "{r}");
    let rec = r["nec"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["recipient_id"] == vid)
        .expect("the vendor is a 1099 recipient")
        .clone();
    assert_eq!(rec["tin"], "••-•••4567");
    assert_eq!(rec["name"], "Compliance Plumbing LLC", "the W-9 legal name");
    assert_eq!(rec["missing_tin"], false);
    let resp = c
        .client
        .get(format!("/reports/1099/export?year={year}&format=csv"))
        .header(bearer(&admin))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    let csv = resp.into_string().await.unwrap();
    assert!(csv.contains("27-1234567"), "{csv}");
    let exports = entity::prelude::AuditLog::find()
        .filter(entity::audit_log::Column::Action.eq(crate::audit::actions::TAX_1099_EXPORT))
        .filter(entity::audit_log::Column::TenantId.eq(nw))
        .all(&c.db)
        .await
        .unwrap();
    assert!(!exports.is_empty(), "who pulled full TINs is recorded");

    // F12: the dispatch gate is off by default.
    let pid = property_ids(c, nw).await[0];
    let (st, t) = post_json(
        c,
        &format!("/properties/{pid}/tickets"),
        &admin,
        serde_json::json!({ "title": "Leak under sink" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{t}");
    let tid = t["id"].as_str().unwrap().to_string();
    let (st, set) = send_json(
        c,
        Method::Put,
        "/settings/compliance.require_coi",
        &admin,
        serde_json::json!({ "value": true }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{set}");
    let assign = |reason: Option<&str>| serde_json::json!({ "assignee_entity_id": vid, "coi_override_reason": reason });
    let (st, err) = send_json(
        c,
        Method::Patch,
        &format!("/tickets/{tid}"),
        &admin,
        assign(None),
    )
    .await;
    assert_eq!(st, Status::Conflict, "{err}");
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/tickets/{tid}"),
        &admin,
        assign(Some("Emergency, the usual plumber is out")),
    )
    .await;
    assert_eq!(st, Status::Ok);
    let overrides = entity::prelude::AuditLog::find()
        .filter(entity::audit_log::Column::Action.eq(crate::audit::actions::VENDOR_COI_OVERRIDE))
        .filter(entity::audit_log::Column::TargetId.eq(tid.clone()))
        .all(&c.db)
        .await
        .unwrap();
    assert_eq!(overrides.len(), 1);

    // F12: insurance. A current certificate clears the gate.
    let (st, comp) = post_json(
        c,
        &format!("/entities/{vid}/insurance"),
        &admin,
        serde_json::json!({ "kind": "general_liability", "carrier": "Acme Mutual",
            "limit_cents": 100_000_000, "expires_on": (today + chrono::Duration::days(20)).to_string() }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{comp}");
    assert_eq!(comp["coi_current"], true);
    assert_eq!(comp["insurance"][0]["state"], "expiring");
    let (st, t2) = post_json(
        c,
        &format!("/properties/{pid}/tickets"),
        &admin,
        serde_json::json!({ "title": "Second leak", "assignee_entity_id": vid }),
    )
    .await;
    assert_eq!(
        st,
        Status::Ok,
        "covered vendors go out without a reason: {t2}"
    );

    // F12: the 30-day reminder goes to the vendor once.
    let vendor_emails = || async {
        entity::prelude::BackgroundJob::find()
            .filter(entity::background_job::Column::TenantId.eq(nw))
            .filter(entity::background_job::Column::Kind.eq("auto_email"))
            .all(&c.db)
            .await
            .unwrap()
            .into_iter()
            .filter(|j| {
                j.payload["template"] == "coi_expiring"
                    && j.payload["to"] == "office@complianceplumbing.example"
            })
            .count()
    };
    crate::resident_reminders::vendor_insurance(&c.db, nw, today)
        .await
        .unwrap();
    crate::resident_reminders::vendor_insurance(&c.db, nw, today)
        .await
        .unwrap();
    assert_eq!(vendor_emails().await, 1);
    // Then the 7-day one.
    crate::resident_reminders::vendor_insurance(&c.db, nw, today + chrono::Duration::days(15))
        .await
        .unwrap();
    assert_eq!(vendor_emails().await, 2);

    // Removing it puts the gate back.
    let cert = comp["insurance"][0]["id"].as_str().unwrap().to_string();
    let (st, comp) = send_json(
        c,
        Method::Delete,
        &format!("/vendor-insurance/{cert}"),
        &admin,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Ok, "{comp}");
    assert_eq!(comp["coi_current"], false);
    let (st, _) = post_json(
        c,
        &format!("/properties/{pid}/tickets"),
        &admin,
        serde_json::json!({ "title": "Third leak", "assignee_entity_id": vid }),
    )
    .await;
    assert_eq!(st, Status::Conflict);

    // Leave the workspace as the other scenarios expect it.
    send_json(
        c,
        Method::Put,
        "/settings/compliance.require_coi",
        &admin,
        serde_json::json!({ "value": false }),
    )
    .await;
}

async fn batch_c_texts(c: &Ctx) {
    use rocket::http::Method;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    let nw = tenant_id(c, "northwind").await;
    let cascade = tenant_id(c, "cascade").await;
    let admin = mint(
        c,
        Some(nw),
        false,
        &[
            "tenant:manage",
            "property:read",
            "property:write",
            "lease:read",
            "lease:manage",
            "maintenance:manage",
            "maintenance:read",
            "message:read",
            "message:manage",
        ],
    );
    let phone = "+17605550142";
    let sms_jobs = |template: &'static str| async move {
        entity::prelude::BackgroundJob::find()
            .filter(entity::background_job::Column::TenantId.eq(nw))
            .filter(entity::background_job::Column::Kind.eq("auto_sms"))
            .all(&c.db)
            .await
            .unwrap()
            .into_iter()
            .filter(|j| j.payload["template"] == template && j.payload["to"] == phone)
            .collect::<Vec<_>>()
    };

    // A resident with a number, and their repair.
    let pid = property_ids(c, nw).await[0];
    let today = chrono::Utc::now().date_naive();
    let (st, lease) = post_json(
        c,
        &format!("/properties/{pid}/leases"),
        &admin,
        serde_json::json!({
            "tenant_name": "Tess Texter",
            "tenant_email": "tess.texter@example.com",
            "tenant_phone": "(760) 555-0142",
            "rent_cents": 120000,
            "start_date": (today - chrono::Duration::days(30)).to_string(),
            "end_date": (today + chrono::Duration::days(300)).to_string(),
            "status": "active",
        }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{lease}");
    let lid = lease["id"].as_str().unwrap().to_string();
    let (st, t) = post_json(
        c,
        &format!("/properties/{pid}/tickets"),
        &admin,
        serde_json::json!({ "title": "Bedroom outlet dead", "lease_id": lid }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{t}");
    let tid = t["id"].as_str().unwrap().to_string();

    // F14: resolving asks for a rating by text, once.
    for status in ["resolved", "in_progress", "resolved"] {
        let (st, _) = send_json(
            c,
            Method::Patch,
            &format!("/tickets/{tid}"),
            &admin,
            serde_json::json!({ "status": status }),
        )
        .await;
        assert_eq!(st, Status::Ok);
    }
    assert_eq!(sms_jobs("ticket_rate_request").await.len(), 1);

    // A stranger's "5" is just a text.
    let (st, _) = post_json(
        c,
        "/texts/simulate",
        &admin,
        serde_json::json!({ "phone": "+17605550199", "body": "5" }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    // The resident's "4 stars" becomes the review; a second digit doesn't.
    let (st, th) = post_json(
        c,
        "/texts/simulate",
        &admin,
        serde_json::json!({ "phone": phone, "body": "4 stars" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{th}");
    post_json(
        c,
        "/texts/simulate",
        &admin,
        serde_json::json!({ "phone": phone, "body": "1" }),
    )
    .await;
    let (_, t) = get_json(c, &format!("/tickets/{tid}"), &admin).await;
    assert_eq!(t["rating"], 4, "{t}");
    assert_eq!(sms_jobs("ticket_rating_thanks").await.len(), 1);
    let thread_id = th["thread"]["id"].as_str().unwrap().to_string();

    // F15: a repair text gets a prefilled link, once a day.
    post_json(
        c,
        "/texts/simulate",
        &admin,
        serde_json::json!({ "phone": phone, "body": "The kitchen faucet is leaking again" }),
    )
    .await;
    post_json(
        c,
        "/texts/simulate",
        &admin,
        serde_json::json!({ "phone": phone, "body": "also the toilet is clogged" }),
    )
    .await;
    let links = sms_jobs("repair_link").await;
    assert_eq!(links.len(), 1);
    let url = links[0].payload["vars"]["url"].as_str().unwrap();
    assert!(
        url.contains(
            "/account/maintenance?new=1&title=The%20kitchen%20faucet%20is%20leaking%20again&category=plumbing"
        ),
        "{url}"
    );

    // F16: saved replies, isolated per workspace.
    let (st, r) = post_json(
        c,
        "/texts/replies",
        &admin,
        serde_json::json!({ "title": "On our way", "body": "Our tech is on the way." }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{r}");
    let rid = r["id"].as_str().unwrap().to_string();
    let (st, _) = post_json(
        c,
        "/texts/replies",
        &admin,
        serde_json::json!({ "title": " ", "body": "x" }),
    )
    .await;
    assert_eq!(st, Status::BadRequest);
    let (_, list) = get_json(c, "/texts/replies", &admin).await;
    assert!(list.as_array().unwrap().iter().any(|x| x["id"] == rid));
    let other = mint(c, Some(cascade), false, &["message:read", "message:manage"]);
    let (_, theirs) = get_json(c, "/texts/replies", &other).await;
    assert!(!theirs.as_array().unwrap().iter().any(|x| x["id"] == rid));
    let (st, _) = send_json(
        c,
        Method::Delete,
        &format!("/texts/replies/{rid}"),
        &other,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::NotFound);
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/texts/replies/{rid}"),
        &admin,
        serde_json::json!({ "title": "On our way", "body": "Our tech is about 20 minutes out." }),
    )
    .await;
    assert_eq!(st, Status::Ok);

    // F16: assign the conversation to a teammate; "mine" finds it.
    let jordan = entity::prelude::User::find()
        .filter(entity::user::Column::Email.eq("jordan@northwind.com"))
        .one(&c.db)
        .await
        .unwrap()
        .expect("seeded manager");
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/texts/{thread_id}"),
        &admin,
        serde_json::json!({ "assignee": uuid::Uuid::new_v4().to_string() }),
    )
    .await;
    assert_eq!(st, Status::BadRequest, "only teammates");
    let (st, upd) = send_json(
        c,
        Method::Patch,
        &format!("/texts/{thread_id}"),
        &admin,
        serde_json::json!({ "assignee": jordan.id.to_string() }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{upd}");
    assert_eq!(upd["assigned_user_id"], jordan.id.to_string());
    let jt = crate::auth::issue_access_token(
        &c.config,
        jordan.id,
        Some(nw),
        false,
        vec!["message:read".into()],
    )
    .unwrap();
    let (_, mine) = get_json(c, "/texts?mine=true", &jt).await;
    assert!(mine
        .as_array()
        .unwrap()
        .iter()
        .any(|x| x["id"] == thread_id));
    let (_, mine) = get_json(c, "/texts?mine=true", &admin).await;
    assert!(mine.as_array().unwrap().is_empty());

    // F16: photos queue a filing job; without live Twilio it skips cleanly.
    crate::texts::record_inbound(
        &c.db,
        nw,
        phone,
        "here is the leak",
        None,
        &[(
            "https://api.twilio.com/2010-04-01/Accounts/AC1/Messages/MM1/Media/ME1".into(),
            "image/jpeg".into(),
        )],
    )
    .await
    .unwrap();
    let media_jobs = entity::prelude::BackgroundJob::find()
        .filter(entity::background_job::Column::TenantId.eq(nw))
        .filter(entity::background_job::Column::Kind.eq("sms_media"))
        .all(&c.db)
        .await
        .unwrap();
    assert_eq!(media_jobs.len(), 1);
    let out = crate::text_auto::handle_media_job(&c.db, &media_jobs[0]).await;
    assert_eq!(out.status, "completed");

    // F17: marketing texts need consent.
    let run_now = |id: uuid::Uuid| async move {
        let job = entity::prelude::BackgroundJob::find_by_id(id)
            .one(&c.db)
            .await
            .unwrap()
            .unwrap();
        crate::notify::handle_job(&c.db, &job).await
    };
    let marketing = crate::scheduler::enqueue(
        &c.db,
        nw,
        "auto_sms",
        serde_json::json!({ "template": "test_notification", "to": phone, "marketing": true }),
        0,
    )
    .await
    .unwrap();
    let out = run_now(marketing).await;
    assert_eq!(
        out.result.as_ref().and_then(|r| r["reason"].as_str()),
        Some("no_marketing_consent")
    );
    let (st, upd) = send_json(
        c,
        Method::Patch,
        &format!("/texts/{thread_id}"),
        &admin,
        serde_json::json!({ "marketing_consent": true }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    assert_eq!(upd["marketing_consent"], true);

    // F17: quiet hours hold automatic texts but not typed replies.
    use chrono::Timelike;
    let hour = chrono::Utc::now().hour() as i64;
    for (key, value) in [
        (crate::settings::TEXTS_TIMEZONE, serde_json::json!("UTC")),
        (crate::settings::TEXTS_QUIET_START, serde_json::json!(hour)),
        (
            crate::settings::TEXTS_QUIET_END,
            serde_json::json!((hour + 2) % 24),
        ),
        (crate::settings::TEXTS_QUIET_HOURS, serde_json::json!(true)),
    ] {
        crate::settings::set_value(&c.db, nw, key, value)
            .await
            .unwrap();
    }
    let held = crate::scheduler::enqueue(
        &c.db,
        nw,
        "auto_sms",
        serde_json::json!({ "template": "rent_due", "to": phone, "vars": {} }),
        0,
    )
    .await
    .unwrap();
    let out = run_now(held).await;
    assert_eq!(out.status, "pending", "{:?}", out.result);
    assert!(out.run_at.unwrap() > chrono::Utc::now() + chrono::Duration::minutes(30));
    let typed = crate::scheduler::enqueue(
        &c.db,
        nw,
        "auto_sms",
        serde_json::json!({ "template": "direct_text", "to": phone,
            "vars": { "text": "See you tomorrow" }, "sms_message_id": uuid::Uuid::new_v4().to_string() }),
        0,
    )
    .await
    .unwrap();
    let out = run_now(typed).await;
    assert_ne!(out.status, "pending", "typed replies ignore quiet hours");
    crate::settings::set_value(
        &c.db,
        nw,
        crate::settings::TEXTS_QUIET_HOURS,
        serde_json::json!(false),
    )
    .await
    .unwrap();
}

async fn batch_d_listing_photos(c: &Ctx) {
    use rocket::http::{Header, Method};
    let nw = tenant_id(c, "northwind").await;
    let cascade = tenant_id(c, "cascade").await;
    let admin = mint(
        c,
        Some(nw),
        false,
        &[
            "listing:read",
            "listing:write",
            "document:read",
            "document:manage",
            "property:read",
        ],
    );
    let pid = property_ids(c, nw).await[0];
    let (st, l) = post_json(
        c,
        &format!("/properties/{pid}/listings"),
        &admin,
        serde_json::json!({ "title": "Photo test home", "rent_cents": 210000, "is_public": true }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{l}");
    let lid = l["id"].as_str().unwrap().to_string();

    // Upload through the documents flow, filed on the listing.
    let upload =
        |owner_type: &'static str, owner_id: String, name: &'static str, mime: &'static str| {
            let admin = admin.clone();
            async move {
                let (st, reg) = post_json(
                    c,
                    "/documents",
                    &admin,
                    serde_json::json!({ "owner_type": owner_type, "owner_id": owner_id,
                    "filename": name, "mime_type": mime, "size_bytes": 4 }),
                )
                .await;
                assert_eq!(st, Status::Ok, "{reg}");
                let url = reg["upload_url"].as_str().unwrap();
                let path = &url[url.find("/storage/local/").unwrap()..];
                let resp = c
                    .client
                    .put(path.to_string())
                    .body(vec![0xFF, 0xD8, 0xFF, 0xD9])
                    .dispatch()
                    .await;
                assert!(resp.status().code < 300, "{}", resp.status());
                reg["document"]["id"].as_str().unwrap().to_string()
            }
        };
    let kitchen = upload("listing", lid.clone(), "kitchen.jpg", "image/jpeg").await;
    let porch = upload("listing", lid.clone(), "porch.jpg", "image/jpeg").await;
    let notes = upload("listing", lid.clone(), "notes.txt", "text/plain").await;
    let elsewhere = upload("property", pid.to_string(), "roof.jpg", "image/jpeg").await;

    let add = |doc: String, alt: &'static str| {
        let admin = admin.clone();
        let lid = lid.clone();
        async move {
            post_json(
                c,
                &format!("/listings/{lid}/photos"),
                &admin,
                serde_json::json!({ "document_id": doc, "alt_text": alt }),
            )
            .await
        }
    };
    let (st, _) = add(kitchen.clone(), "  ").await;
    assert_eq!(st, Status::BadRequest, "alt text is required");
    let (st, _) = add(notes, "Notes").await;
    assert_eq!(st, Status::BadRequest, "only images");
    let (st, _) = add(elsewhere, "Roof").await;
    assert_eq!(st, Status::BadRequest, "only this listing's uploads");
    let (st, _) = add(kitchen.clone(), "Bright kitchen with a gas range").await;
    assert_eq!(st, Status::Ok);
    let (st, _) = add(kitchen.clone(), "again").await;
    assert_eq!(st, Status::Conflict);
    let (st, list) = add(porch, "Shaded front porch").await;
    assert_eq!(st, Status::Ok, "{list}");
    let ids: Vec<String> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(ids.len(), 2);
    assert!(list[0]["preview_url"]
        .as_str()
        .unwrap()
        .contains("/storage/local/"));

    // The porch becomes the hero.
    let (st, _) = send_json(
        c,
        Method::Put,
        &format!("/listings/{lid}/photos/order"),
        &admin,
        serde_json::json!({ "ids": [ids[1]] }),
    )
    .await;
    assert_eq!(st, Status::BadRequest, "every photo, once");
    let (st, list) = send_json(
        c,
        Method::Put,
        &format!("/listings/{lid}/photos/order"),
        &admin,
        serde_json::json!({ "ids": [ids[1], ids[0]] }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    assert_eq!(list[0]["alt_text"], "Shaded front porch");

    // The public page carries them in order, and the image link works.
    let resp = c
        .client
        .get(format!("/public/listings/{lid}"))
        .header(Header::new("X-Tenant", "northwind"))
        .dispatch()
        .await;
    let pl = resp.into_json::<serde_json::Value>().await.unwrap();
    assert_eq!(pl["photos"][0]["alt"], "Shaded front porch");
    let hero = pl["photos"][0]["url"].as_str().unwrap().to_string();
    assert!(hero.ends_with(&format!("/public/listing-photos/{}", ids[1])));
    let resp = c
        .client
        .get(format!("/public/listing-photos/{}", ids[1]))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::TemporaryRedirect);
    assert!(resp
        .headers()
        .get_one("Location")
        .unwrap_or("")
        .contains("/storage/local/"));
    let resp = c
        .client
        .get("/public/listings")
        .header(Header::new("X-Tenant", "northwind"))
        .dispatch()
        .await;
    let all = resp.into_json::<serde_json::Value>().await.unwrap();
    let card = all
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["id"] == lid.as_str())
        .unwrap()
        .clone();
    assert_eq!(card["photos"].as_array().unwrap().len(), 2);

    // Other workspaces see nothing; an unpublished listing's photos stop.
    let other = mint(c, Some(cascade), false, &["listing:read", "listing:write"]);
    let (st, _) = get_json(c, &format!("/listings/{lid}/photos"), &other).await;
    assert_eq!(st, Status::NotFound);
    let (st, _) = send_json(
        c,
        Method::Delete,
        &format!("/listing-photos/{}", ids[0]),
        &other,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::NotFound);
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/listings/{lid}"),
        &admin,
        serde_json::json!({ "is_public": false }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    let resp = c
        .client
        .get(format!("/public/listing-photos/{}", ids[1]))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::NotFound);

    // Edit and remove.
    let (st, list) = send_json(
        c,
        Method::Patch,
        &format!("/listing-photos/{}", ids[0]),
        &admin,
        serde_json::json!({ "caption": "Remodeled 2025" }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    assert_eq!(list[1]["caption"], "Remodeled 2025");
    let (st, list) = send_json(
        c,
        Method::Delete,
        &format!("/listing-photos/{}", ids[0]),
        &admin,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Ok);
    assert_eq!(list.as_array().unwrap().len(), 1);
}

/// A property manager sees only the properties assigned to them; the company
/// owner sees everything.
async fn property_reach(c: &Ctx) {
    use rocket::http::Method;
    let nw = tenant_id(c, "northwind").await;
    let owner = mint(
        c,
        Some(nw),
        false,
        &[
            "member:manage",
            "entity:manage",
            "property:read",
            "property:write",
            "maintenance:read",
            "maintenance:manage",
            "lease:read",
        ],
    );
    let props = property_ids(c, nw).await;
    assert!(props.len() >= 2, "seed has several properties");
    let (mine, other) = (props[0], props[1]);

    // A new property manager, assigned to one property.
    let (st, m) = post_json(
        c,
        "/members",
        &owner,
        serde_json::json!({ "email": "pat.reach@northwind.test", "name": "Pat Reach",
            "profile_type": "property_manager" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{m}");
    let pat = uuid::Uuid::parse_str(m["user_id"].as_str().unwrap()).unwrap();
    let (st, a) = post_json(
        c,
        &format!("/properties/{mine}/assignments"),
        &owner,
        serde_json::json!({ "user_id": pat, "relationship": "property_manager" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{a}");
    let perms: Vec<String> = [
        "property:read",
        "property:write",
        "maintenance:read",
        "maintenance:manage",
        "lease:read",
        "ledger:read",
        "member:manage",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let pm = crate::auth::issue_access_token(&c.config, pat, Some(nw), false, perms).unwrap();

    // /auth/me says so.
    let (st, me) = get_json(c, "/auth/me", &pm).await;
    assert_eq!(st, Status::Ok, "{me}");
    assert_eq!(me["reach"]["scope"], "properties");
    assert_eq!(me["reach"]["property_ids"], serde_json::json!([mine]));

    // Lists narrow to the assigned property.
    let (_, list) = get_json(c, "/properties", &pm).await;
    let ids: Vec<_> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].clone())
        .collect();
    assert_eq!(ids, vec![serde_json::json!(mine)]);
    let (_, all) = get_json(c, "/properties", &owner).await;
    assert!(
        all.as_array().unwrap().len() >= 2,
        "the owner sees the company"
    );
    let (_, s) = get_json(c, "/portfolio/summary", &pm).await;
    assert_eq!(s["properties"], 1);
    for path in ["/tickets", "/leases"] {
        let (st, rows) = get_json(c, path, &pm).await;
        assert_eq!(st, Status::Ok, "{path}");
        assert!(
            rows.as_array()
                .unwrap()
                .iter()
                .all(|r| r["property_id"] == serde_json::json!(mine)),
            "{path} stays on the assigned property"
        );
    }
    let (_, groups) = get_json(c, "/portfolio/llcs", &pm).await;
    for g in groups.as_array().unwrap() {
        for p in g["properties"].as_array().unwrap() {
            assert_eq!(p["id"], serde_json::json!(mine));
        }
    }

    // One property: theirs opens, another company property is not found.
    let (st, _) = get_json(c, &format!("/properties/{mine}"), &pm).await;
    assert_eq!(st, Status::Ok);
    let (st, _) = get_json(c, &format!("/properties/{other}"), &pm).await;
    assert_eq!(st, Status::NotFound);
    let (st, _) = get_json(c, &format!("/properties/{other}/tickets"), &pm).await;
    assert_eq!(st, Status::NotFound);
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/properties/{other}"),
        &pm,
        serde_json::json!({ "notes": "x" }),
    )
    .await;
    assert_eq!(st, Status::NotFound);

    // A ticket on someone else's property is out of reach too.
    let (st, t) = post_json(
        c,
        &format!("/properties/{other}/tickets"),
        &owner,
        serde_json::json!({ "title": "Out of reach" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{t}");
    let tid = t["id"].as_str().unwrap();
    let (st, _) = get_json(c, &format!("/tickets/{tid}"), &pm).await;
    assert_eq!(st, Status::NotFound);
    let (st, _) = get_json(c, &format!("/tickets/{tid}"), &owner).await;
    assert_eq!(st, Status::Ok);

    // Company-only actions and areas not yet reach-aware are closed.
    let (st, _) = post_json(
        c,
        &format!("/properties/{mine}/assignments"),
        &pm,
        serde_json::json!({ "user_id": pat, "relationship": "maintenance" }),
    )
    .await;
    assert_eq!(
        st,
        Status::Forbidden,
        "who is assigned where stays with the company"
    );
    let (st, _) = get_json(c, "/payments", &pm).await;
    assert_eq!(st, Status::Forbidden);
    let (st, _) = get_json(c, "/members", &pm).await;
    assert_eq!(st, Status::Forbidden);

    // Search only finds what's in reach.
    let other_name = entity::prelude::Property::find_by_id(other)
        .one(&c.db)
        .await
        .unwrap()
        .unwrap()
        .name;
    let (st, found) = get_json(
        c,
        &format!(
            "/search?q={}",
            other_name
                .bytes()
                .map(|b| if b.is_ascii_alphanumeric() {
                    (b as char).to_string()
                } else {
                    format!("%{b:02X}")
                })
                .collect::<String>()
        ),
        &pm,
    )
    .await;
    assert_eq!(st, Status::Ok, "{found}");
    assert!(
        !found["hits"]
            .as_array()
            .unwrap()
            .iter()
            .any(|h| h["id"] == serde_json::json!(other.to_string())),
        "{found}"
    );

    // An LLC assignment brings in that LLC's properties.
    if let Some(llc) = entity::prelude::Property::find_by_id(other)
        .one(&c.db)
        .await
        .unwrap()
        .and_then(|p| p.llc_id)
    {
        let (st, _) = post_json(
            c,
            &format!("/entities/{llc}/assignments"),
            &owner,
            serde_json::json!({ "user_id": pat, "relationship": "property_manager" }),
        )
        .await;
        assert_eq!(st, Status::Ok);
        let (st, _) = get_json(c, &format!("/properties/{other}"), &pm).await;
        assert_eq!(st, Status::Ok, "the LLC's property is now in reach");
    }
}

/// The service desk: a kit opens a work order with tasks by trade and parts
/// with costs; tasks go to vendors; photos ride on notes; receipts back up
/// expenses; costs compare to the estimate; routines start from a kit; and
/// a property manager can do all of it on their own properties only.
async fn service_desk(c: &Ctx) {
    use rocket::http::Method;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    let nw = tenant_id(c, "northwind").await;
    let staff = mint(
        c,
        Some(nw),
        false,
        &[
            "maintenance:read",
            "maintenance:manage",
            "entity:read",
            "entity:manage",
            "member:manage",
            "property:read",
            "property:write",
        ],
    );
    let props = property_ids(c, nw).await;
    let (pid, other) = (props[0], props[1]);

    // The catalog has the shower kit, with a plumber flagged.
    let (st, kits) = get_json(c, "/issue-templates", &staff).await;
    assert_eq!(st, Status::Ok, "{kits}");
    let shower = kits
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["name"] == "Replace shower")
        .expect("the shower kit")
        .clone();
    assert!(shower["tasks"].as_array().unwrap().len() >= 8);
    assert!(shower["trades"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("drywall")));
    assert_eq!(shower["contractor_trades"], serde_json::json!(["plumbing"]));
    assert!(shower["est_total_cents"].as_i64().unwrap() > 100_000);
    let kit_id = shower["id"].as_str().unwrap().to_string();

    // One click: the work order with its tasks and parts.
    let (st, g) = post_json(
        c,
        &format!("/issue-templates/{kit_id}/generate"),
        &staff,
        serde_json::json!({ "property_id": pid, "note": "Tile cracked, water behind the wall" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{g}");
    let tid = g["ticket"]["id"].as_str().unwrap().to_string();
    assert!(
        !g["ticket"]["description"]
            .as_str()
            .unwrap_or("")
            .contains("Checklist"),
        "tasks are line items, not text"
    );
    let (_, tasks) = get_json(c, &format!("/tickets/{tid}/tasks"), &staff).await;
    let tasks = tasks.as_array().unwrap().clone();
    assert_eq!(tasks.len(), shower["tasks"].as_array().unwrap().len());
    let valve = tasks
        .iter()
        .find(|t| t["trade"] == "plumbing" && t["needs_contractor"] == true)
        .unwrap()
        .clone();
    assert!(valve["est_cost_cents"].as_i64().unwrap() > 0);
    let (_, parts) = get_json(c, &format!("/tickets/{tid}/parts"), &staff).await;
    assert!(parts
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["name"].as_str().unwrap().contains("valve")));
    let (_, costs) = get_json(c, &format!("/tickets/{tid}/costs"), &staff).await;
    assert!(
        costs["est_parts_cents"].as_i64().unwrap() > 50_000,
        "{costs}"
    );
    assert_eq!(costs["trades_needed"][0]["trade"], "plumbing");
    assert_eq!(costs["trades_needed"][0]["covered"], false);

    // A plumber, found by trade, gets the valve task by email.
    let (st, v) = post_json(
        c,
        "/entities",
        &staff,
        serde_json::json!({ "kind": "contractor", "name": "Desk Plumbing Co",
            "email": "jobs@deskplumbing.example", "trades": ["plumbing", "nonsense"] }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{v}");
    assert_eq!(v["trades"], serde_json::json!(["plumbing"]));
    let vid = v["id"].as_str().unwrap().to_string();
    let (_, options) = get_json(c, &format!("/tickets/{tid}/vendors?trade=plumbing"), &staff).await;
    assert_eq!(options[0]["id"], vid.as_str(), "matching trade first");
    assert_eq!(options[0]["matches"], true);
    // A "vendor" that lists trades is offered too; a supplier with none isn't.
    let mut made = vec![];
    for (name, trades) in [
        ("Desert Wash Co", serde_json::json!(["exterior"])),
        ("Supply Barn", serde_json::json!([])),
    ] {
        let (st, v) = post_json(
            c,
            "/entities",
            &staff,
            serde_json::json!({ "kind": "vendor", "name": name, "trades": trades }),
        )
        .await;
        assert_eq!(st, Status::Ok, "{v}");
        made.push(v["id"].clone());
    }
    let (_, options) = get_json(c, &format!("/tickets/{tid}/vendors?trade=exterior"), &staff).await;
    assert_eq!(options[0]["id"], made[0]);
    assert!(!options
        .as_array()
        .unwrap()
        .iter()
        .any(|o| o["id"] == made[1]));
    let task_id = valve["id"].as_str().unwrap();
    let (st, after) = post_json(
        c,
        &format!("/tickets/{tid}/tasks/{task_id}/dispatch"),
        &staff,
        serde_json::json!({ "entity_id": vid, "note": "Moen valve preferred" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{after}");
    let sent = after
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == task_id)
        .unwrap()
        .clone();
    assert_eq!(sent["assignee_name"], "Desk Plumbing Co");
    assert!(sent["dispatched_at"].is_string());
    let emails = entity::prelude::BackgroundJob::find()
        .filter(entity::background_job::Column::TenantId.eq(nw))
        .filter(entity::background_job::Column::Kind.eq("auto_email"))
        .all(&c.db)
        .await
        .unwrap()
        .into_iter()
        .filter(|j| j.payload["to"] == "jobs@deskplumbing.example")
        .count();
    assert_eq!(emails, 1);
    let (_, costs) = get_json(c, &format!("/tickets/{tid}/costs"), &staff).await;
    assert_eq!(costs["trades_needed"][0]["open_tasks"], 3);
    assert_eq!(
        costs["trades_needed"][0]["covered"], false,
        "two plumbing tasks still open"
    );
    for t in tasks
        .iter()
        .filter(|t| t["trade"] == "plumbing" && t["needs_contractor"] == true && t["id"] != task_id)
    {
        let (st, _) = send_json(
            c,
            Method::Patch,
            &format!("/tickets/{tid}/tasks/{}", t["id"].as_str().unwrap()),
            &staff,
            serde_json::json!({ "assignee_entity_id": vid }),
        )
        .await;
        assert_eq!(st, Status::Ok);
    }
    let (_, costs) = get_json(c, &format!("/tickets/{tid}/costs"), &staff).await;
    assert_eq!(costs["trades_needed"][0]["covered"], true);

    // Tick a task off; add and remove one by hand.
    let first = tasks[0]["id"].as_str().unwrap();
    let (st, done) = send_json(
        c,
        Method::Patch,
        &format!("/tickets/{tid}/tasks/{first}"),
        &staff,
        serde_json::json!({ "status": "done" }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    assert!(done[0]["done_at"].is_string());
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/tickets/{tid}/tasks/{first}"),
        &staff,
        serde_json::json!({ "status": "finished" }),
    )
    .await;
    assert_eq!(st, Status::BadRequest);
    let (_, added) = post_json(
        c,
        &format!("/tickets/{tid}/tasks"),
        &staff,
        serde_json::json!({ "title": "Replace bathroom fan", "trade": "electrical",
            "est_minutes": 60, "needs_contractor": true }),
    )
    .await;
    let fan = added.as_array().unwrap().last().unwrap().clone();
    assert_eq!(fan["trade"], "electrical");
    let (st, left) = send_json(
        c,
        Method::Delete,
        &format!("/tickets/{tid}/tasks/{}", fan["id"].as_str().unwrap()),
        &staff,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Ok);
    assert_eq!(left.as_array().unwrap().len(), tasks.len());

    // A receipt, uploaded and attached to the purchase.
    let upload = |kind: &'static str, name: &'static str, mime: &'static str| {
        let staff = staff.clone();
        let tid = tid.clone();
        async move {
            let (st, up) = post_json(
                c,
                &format!("/tickets/{tid}/uploads"),
                &staff,
                serde_json::json!({ "filename": name, "mime_type": mime, "size_bytes": 4, "kind": kind }),
            )
            .await;
            assert_eq!(st, Status::Ok, "{up}");
            let url = up["upload_url"].as_str().unwrap();
            let path = &url[url.find("/storage/local/").unwrap()..];
            let resp = c
                .client
                .put(path.to_string())
                .body(vec![0xFF, 0xD8, 0xFF, 0xD9])
                .dispatch()
                .await;
            assert!(resp.status().code < 300);
            up["file"]["id"].as_str().unwrap().to_string()
        }
    };
    let receipt = upload("receipt", "home-depot.jpg", "image/jpeg").await;
    let (st, e) = post_json(
        c,
        &format!("/tickets/{tid}/expenses"),
        &staff,
        serde_json::json!({ "description": "Valve, cement board, screws", "vendor": "Home Depot",
            "amount_cents": 21_455, "receipt_document_ids": [receipt], "billable_to_owner": true }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{e}");
    assert_eq!(e["receipt_document_ids"][0], receipt.as_str());
    let (st, _) = post_json(
        c,
        &format!("/tickets/{tid}/expenses"),
        &staff,
        serde_json::json!({ "description": "x", "amount_cents": 0 }),
    )
    .await;
    assert_eq!(st, Status::BadRequest);
    let (_, costs) = get_json(c, &format!("/tickets/{tid}/costs"), &staff).await;
    assert_eq!(costs["expenses_cents"], 21_455);
    assert_eq!(costs["receipts"], 1);
    assert_eq!(costs["tasks_done"], 1);

    // A photo on a note.
    let photo = upload("photo", "wall-open.jpg", "image/jpeg").await;
    let (st, note) = post_json(
        c,
        &format!("/tickets/{tid}/comments"),
        &staff,
        serde_json::json!({ "body": "Wall open, studs are fine", "visibility": "internal",
            "document_ids": [photo] }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{note}");
    assert_eq!(note["document_ids"][0], photo.as_str());
    let (_, files) = get_json(c, &format!("/tickets/{tid}/files"), &staff).await;
    let kinds: Vec<_> = files
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["kind"].clone())
        .collect();
    assert!(
        kinds.contains(&serde_json::json!("photo"))
            && kinds.contains(&serde_json::json!("receipt"))
    );
    assert!(files[0]["url"]
        .as_str()
        .unwrap()
        .contains("/storage/local/"));
    let (st, _) = post_json(
        c,
        &format!("/tickets/{tid}/uploads"),
        &staff,
        serde_json::json!({ "filename": "notes.pdf", "mime_type": "application/pdf", "kind": "photo" }),
    )
    .await;
    assert_eq!(st, Status::BadRequest, "a photo has to be an image");

    // A kit added to a work order that's already open.
    let toilet = kits
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["name"] == "Replace toilet")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let (st, more) = post_json(
        c,
        &format!("/tickets/{tid}/kits"),
        &staff,
        serde_json::json!({ "issue_template_id": toilet }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    assert!(more.as_array().unwrap().len() > tasks.len());

    // A routine with a kit opens its work order with the kit's tasks.
    let hvac = kits
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["name"] == "Service HVAC (seasonal)")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let today = chrono::Utc::now().date_naive().to_string();
    let (st, plan) = post_json(
        c,
        "/maintenance-plans",
        &staff,
        serde_json::json!({ "property_id": pid, "title": "Spring HVAC service", "cadence_days": 182,
            "next_due_date": today, "issue_template_id": hvac }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{plan}");
    crate::helpdesk::run_due_plans(&c.db, nw).await.unwrap();
    let plan_row = entity::prelude::MaintenancePlan::find_by_id(
        uuid::Uuid::parse_str(plan["id"].as_str().unwrap()).unwrap(),
    )
    .one(&c.db)
    .await
    .unwrap()
    .unwrap();
    let routine = plan_row
        .last_ticket_id
        .expect("the routine opened a work order");
    let (_, rtasks) = get_json(c, &format!("/tickets/{routine}/tasks"), &staff).await;
    assert_eq!(rtasks.as_array().unwrap().len(), 5);

    // A property manager works their own property, and only that.
    let (_, m) = post_json(
        c,
        "/members",
        &staff,
        serde_json::json!({ "email": "desk.pm@northwind.test", "name": "Desk PM",
            "profile_type": "property_manager" }),
    )
    .await;
    let pm_id = uuid::Uuid::parse_str(m["user_id"].as_str().unwrap()).unwrap();
    post_json(
        c,
        &format!("/properties/{pid}/assignments"),
        &staff,
        serde_json::json!({ "user_id": pm_id, "relationship": "property_manager" }),
    )
    .await;
    let pm = crate::auth::issue_access_token(
        &c.config,
        pm_id,
        Some(nw),
        false,
        vec!["maintenance:read".into(), "maintenance:manage".into()],
    )
    .unwrap();
    let (st, _) = get_json(c, "/issue-templates", &pm).await;
    assert_eq!(st, Status::Ok);
    let (st, _) = post_json(
        c,
        &format!("/issue-templates/{kit_id}/generate"),
        &pm,
        serde_json::json!({ "property_id": other }),
    )
    .await;
    assert_eq!(st, Status::NotFound, "not their property");
    let (st, mine) = post_json(
        c,
        &format!("/issue-templates/{kit_id}/generate"),
        &pm,
        serde_json::json!({ "property_id": pid }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{mine}");
    let (st, _) = get_json(c, &format!("/tickets/{tid}/costs"), &pm).await;
    assert_eq!(st, Status::Ok);
    let (_, plans) = get_json(c, "/maintenance-plans", &pm).await;
    assert!(plans
        .as_array()
        .unwrap()
        .iter()
        .all(|p| p["property_id"] == serde_json::json!(pid)));
    let some_part = parts[0]["id"].as_str().unwrap();
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/parts/{some_part}"),
        &pm,
        serde_json::json!({ "note": "picked up" }),
    )
    .await;
    assert_ne!(
        st,
        Status::Forbidden,
        "parts on their work order are theirs"
    );
    // The catalog is the company's to edit. A renamed starter kit stays
    // renamed; it doesn't come back under its old name.
    let mut edited = shower.clone();
    edited["name"] = serde_json::json!("Shower remodel");
    let (st, _) = send_json(
        c,
        Method::Put,
        &format!("/issue-templates/{kit_id}"),
        &pm,
        edited.clone(),
    )
    .await;
    assert_eq!(st, Status::Forbidden, "a property manager uses the catalog");
    let (st, saved) = send_json(
        c,
        Method::Put,
        &format!("/issue-templates/{kit_id}"),
        &staff,
        edited,
    )
    .await;
    assert_eq!(st, Status::Ok, "{saved}");
    assert_eq!(saved["tasks"], shower["tasks"]);
    let (_, kits) = get_json(c, "/issue-templates", &staff).await;
    let names: Vec<&str> = kits
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"Shower remodel"));
    assert!(!names.contains(&"Replace shower"), "{names:?}");

    // Their own kit, priced at their rates, then retired.
    let (st, mine) = post_json(
        c,
        "/issue-templates",
        &staff,
        serde_json::json!({
            "name": "Garbage disposal swap",
            "category": "appliance",
            "tasks": [
                { "title": "Pull the old unit", "trade": "plumbing", "est_minutes": 30 },
                { "title": "Wire and mount", "trade": "electrical", "est_minutes": 45,
                  "needs_contractor": true }
            ],
            "parts": [{ "name": "Disposal 1/2 HP", "quantity": 1, "unit_cost_cents": 12900 }]
        }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{mine}");
    assert_eq!(mine["contractor_trades"], serde_json::json!(["electrical"]));
    assert!(mine["est_labor_cents"].as_i64().unwrap() > 0);
    assert_eq!(mine["est_parts_cents"], 12900);
    let mine_id = mine["id"].as_str().unwrap();
    let (st, _) = send_json(
        c,
        Method::Delete,
        &format!("/issue-templates/{mine_id}"),
        &staff,
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(st, Status::Ok);
    let (_, kits) = get_json(c, "/issue-templates", &staff).await;
    assert!(!kits
        .as_array()
        .unwrap()
        .iter()
        .any(|k| k["id"] == mine["id"]));
}

/// Moving in from another tool: an AppFolio rent roll is recognised, mapped,
/// previewed (nothing written), committed (one bad row reported, the rest in),
/// matched rather than duplicated the second time, exported in a shape that
/// goes straight back in, and undone, keeping what was used since.
async fn imports_and_exports(c: &Ctx) {
    use rocket::http::Method;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    let nw = tenant_id(c, "northwind").await;
    let staff = mint(
        c,
        Some(nw),
        false,
        &[
            "data:import",
            "data:export",
            "lease:read",
            "lease:manage",
            "property:read",
        ],
    );
    let upload = |path: String, body: &'static str| {
        let staff = staff.clone();
        async move {
            let resp = c
                .client
                .post(path)
                .header(bearer(&staff))
                .header(ContentType::CSV)
                .body(body)
                .dispatch()
                .await;
            let st = resp.status();
            (
                st,
                resp.into_json::<serde_json::Value>()
                    .await
                    .unwrap_or_default(),
            )
        }
    };
    const ROLL: &str = "Rent Roll\nAs of 10/01/2026\n\n\
Property,Unit,Tenant,Status,BD/BA,Sqft,Rent,Deposit,Lease From,Lease To,Past Due\n\
Juniper Flats - 410 Juniper St,101,\"Okafor, Ada\",Current,2/1,850,\"$1,450.00\",\"1,450.00\",01/01/2026,12/31/2026,0.00\n\
Juniper Flats - 410 Juniper St,102,Ben Ruiz,Notice,1/1,610,\"1,150.00\",500,3/1/25,2/28/26,\"$75.00\"\n\
Juniper Flats - 410 Juniper St,103,VACANT,,Studio/1,420,,,,,\n\
Juniper Flats - 410 Juniper St,104,Cy Park,Current,1/1,610,,,6/1/2026,,\n\
Total,,,,,,\"2,600.00\",,,,\n";

    let (st, p) = upload("/imports?kind=tenants&filename=rent_roll.csv".into(), ROLL).await;
    assert_eq!(st, Status::Ok, "{p}");
    let bid = p["batch"]["id"].as_str().unwrap().to_string();
    assert_eq!(p["batch"]["source"], "appfolio", "{p}");
    assert_eq!(p["batch"]["mapping"]["lease_start"], "Lease From");
    assert_eq!(
        p["counts"]["rows"], 4,
        "the title and total rows are skipped"
    );
    assert_eq!(p["counts"]["properties"], 1, "{p}");
    assert_eq!(
        p["counts"]["units"], 3,
        "a failed row leaves nothing behind, its unit included"
    );
    assert_eq!(p["counts"]["leases"], 2);
    assert_eq!(p["counts"]["errors"], 1, "Cy Park has no rent");
    let bad = &p["rows"][0];
    assert_eq!(bad["action"], "error");
    assert!(
        bad["message"].as_str().unwrap().contains("no rent"),
        "{bad}"
    );
    // A preview writes nothing.
    let none = Property::find()
        .filter(entity::property::Column::TenantId.eq(nw))
        .filter(entity::property::Column::Name.eq("Juniper Flats - 410 Juniper St"))
        .one(&c.db)
        .await
        .unwrap();
    assert!(none.is_none());

    // Commit.
    let (st, done) = post_json(
        c,
        &format!("/imports/{bid}/commit"),
        &staff,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Ok, "{done}");
    assert_eq!(done["status"], "done");
    assert_eq!(done["summary"]["leases"], 2);
    let prop = Property::find()
        .filter(entity::property::Column::TenantId.eq(nw))
        .filter(entity::property::Column::Name.eq("Juniper Flats - 410 Juniper St"))
        .one(&c.db)
        .await
        .unwrap()
        .expect("the property came in");
    assert_eq!(prop.units, 3);
    assert_eq!(
        prop.occupied_units, 1,
        "the notice lease isn't counted as occupied"
    );
    let leases = entity::prelude::Lease::find()
        .filter(entity::lease::Column::PropertyId.eq(prop.id))
        .all(&c.db)
        .await
        .unwrap();
    let ada = leases
        .iter()
        .find(|l| l.tenant_name == "Ada Okafor")
        .expect("name flipped");
    assert_eq!(ada.rent_cents, 145_000);
    assert_eq!(ada.start_date, "2026-01-01");
    let ben = leases.iter().find(|l| l.tenant_name == "Ben Ruiz").unwrap();
    assert_eq!(ben.status, "notice");
    assert_eq!(ben.balance_cents, 7_500);
    assert_eq!(ben.payment_status, "late");
    assert_eq!(ben.start_date, "2025-03-01");

    // The same file again: matched, nothing new.
    let (_, again) = upload("/imports?kind=tenants".into(), ROLL).await;
    assert_eq!(again["counts"]["leases"], 0, "{again}");
    assert_eq!(again["counts"]["properties"], 0);
    assert_eq!(again["counts"]["matched"], 2);
    let again_id = again["batch"]["id"].as_str().unwrap().to_string();
    // Fix the mapping instead: point rent at a column that isn't there.
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/imports/{again_id}"),
        &staff,
        serde_json::json!({ "mapping": { "rent": "Nope" } }),
    )
    .await;
    assert_eq!(st, Status::BadRequest);
    let (st, _) = send_json(
        c,
        Method::Delete,
        &format!("/imports/{again_id}"),
        &staff,
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(st, Status::Ok);

    // Export: the tenants file goes back in as a Vantedge file, and matches.
    let resp = c
        .client
        .get("/exports/tenants")
        .header(bearer(&staff))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    let csv = resp.into_string().await.unwrap();
    assert!(
        csv.contains("Ada Okafor") && csv.contains("1450.00"),
        "{csv}"
    );
    let resp = c
        .client
        .post("/imports?kind=tenants")
        .header(bearer(&staff))
        .body(csv)
        .dispatch()
        .await;
    let back: serde_json::Value = resp.into_json().await.unwrap();
    assert_eq!(back["batch"]["source"], "vantedge", "{back}");
    assert_eq!(back["counts"]["leases"], 0, "everything matches: {back}");
    assert_eq!(back["counts"]["errors"], 0, "{back}");
    let back_id = back["batch"]["id"].as_str().unwrap();
    send_json(
        c,
        Method::Delete,
        &format!("/imports/{back_id}"),
        &staff,
        serde_json::Value::Null,
    )
    .await;
    let resp = c
        .client
        .get("/exports/all")
        .header(bearer(&staff))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    assert_eq!(resp.content_type(), Some(ContentType::ZIP));
    let zip = resp.into_bytes().await.unwrap();
    assert!(zip.starts_with(b"PK"), "a zip");

    // Vendors: trades come through, an existing vendor is filled in, not doubled.
    let (st, v) = upload(
        "/imports?kind=vendors".into(),
        "Vendor Name,Category,Email,Phone\nRapid Rooter Plumbing,Plumbing,,(503) 555-0199\nHigh Desert Pest,Pest Control,bugs@hdp.example,\n",
    )
    .await;
    assert_eq!(st, Status::Ok, "{v}");
    assert_eq!(v["counts"]["vendors"], 1, "{v}");
    let vid = v["batch"]["id"].as_str().unwrap();
    let (_, vdone) = post_json(
        c,
        &format!("/imports/{vid}/commit"),
        &staff,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(vdone["summary"]["vendors"], 1);
    let pest = entity::prelude::Counterparty::find()
        .filter(entity::counterparty::Column::TenantId.eq(nw))
        .filter(entity::counterparty::Column::Name.eq("High Desert Pest"))
        .one(&c.db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pest.trades, serde_json::json!(["pest"]));
    assert_eq!(pest.kind, "contractor");

    // Someone takes a payment on Ada's lease; then the import is undone.
    let (st, _) = post_json(
        c,
        &format!("/leases/{}/payments", ada.id),
        &staff,
        serde_json::json!({ "due_date": "2026-10-01", "amount_cents": 145000, "paid_date": "2026-10-01", "status": "paid" }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    let (st, u) = post_json(
        c,
        &format!("/imports/{bid}/undo"),
        &staff,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Ok, "{u}");
    assert_eq!(u["batch"]["status"], "undone");
    assert_eq!(u["report"]["removed"]["lease"], 1, "Ben's lease goes: {u}");
    let kept: Vec<&str> = u["report"]["kept"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k["t"].as_str().unwrap())
        .collect();
    assert!(
        kept.contains(&"lease") && kept.contains(&"unit") && kept.contains(&"property"),
        "{u}"
    );
    assert!(
        u["report"]["kept"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("payments"),
        "{u}"
    );
    assert_eq!(u["report"]["removed"]["unit"], 2, "the other two units go");
    let (st, _) = post_json(
        c,
        &format!("/imports/{bid}/undo"),
        &staff,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Conflict, "once");
    let (_, vu) = post_json(
        c,
        &format!("/imports/{vid}/undo"),
        &staff,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(vu["report"]["removed"]["vendor"], 1, "{vu}");

    // A property manager scoped to their properties can't import.
    let pat = entity::prelude::User::find()
        .filter(entity::user::Column::Email.eq("pat.reach@northwind.test"))
        .one(&c.db)
        .await
        .unwrap()
        .expect("property_reach made Pat, a property manager on one property");
    let pm = crate::auth::issue_access_token(
        &c.config,
        pat.id,
        Some(nw),
        false,
        vec!["data:import".into(), "data:export".into()],
    )
    .unwrap();
    let resp = c
        .client
        .post("/imports?kind=vendors")
        .header(bearer(&pm))
        .body("Name\nX\n")
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Forbidden);

    // Leave Northwind as it was.
    use sea_orm::ConnectionTrait;
    for sql in [
        format!("DELETE FROM lease_payment WHERE lease_id = '{}'", ada.id),
        format!("DELETE FROM lease WHERE property_id = '{}'", prop.id),
        format!("DELETE FROM unit WHERE property_id = '{}'", prop.id),
        format!(
            "DELETE FROM property_detail WHERE property_id = '{}'",
            prop.id
        ),
        format!("DELETE FROM property WHERE id = '{}'", prop.id),
    ] {
        c.db.execute_unprepared(&sql).await.unwrap();
    }
}

/// Listing syndication: each portal reads a feed from a secret URL. A channel
/// is off until turned on (with someone for renters to reach), only listings
/// complete enough for the portals go out, every pull is logged, and a new
/// URL retires the old one.
async fn listing_syndication(c: &Ctx) {
    use rocket::http::Method;
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
    let nw = tenant_id(c, "northwind").await;
    let staff = mint(c, Some(nw), false, &["listing:read", "listing:write"]);
    let (st, s) = get_json(c, "/syndication", &staff).await;
    assert_eq!(st, Status::Ok, "{s}");
    let keys: Vec<&str> = s["channels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["key"].as_str().unwrap())
        .collect();
    assert_eq!(keys, vec!["zillow", "mits"]);
    assert_eq!(s["channels"][0]["enabled"], false);
    let maple = s["listings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["title"] == "The Maple Court")
        .expect("seeded listing")
        .clone();
    assert_eq!(maple["ready"], false);
    assert_eq!(maple["state"], "OR");
    assert_eq!(maple["city"], "Portland", "the state comes off the city");
    assert!(
        maple["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["message"].as_str().unwrap().contains("photo")),
        "{maple}"
    );
    let url = s["channels"][0]["feed_url"].as_str().unwrap().to_string();
    let path = url
        .split_once("/feeds/")
        .map(|(_, p)| format!("/feeds/{p}"))
        .unwrap();
    let pull = |p: String| async move {
        let r = c
            .client
            .get(p)
            .header(Header::new("User-Agent", "ZillowFeedBot/1.0"))
            .dispatch()
            .await;
        let st = r.status();
        (st, r.into_string().await.unwrap_or_default())
    };
    assert_eq!(
        pull(path.clone()).await.0,
        Status::NotFound,
        "off until turned on"
    );

    // A photo makes Maple Court ready.
    let lid = uuid::Uuid::parse_str(maple["id"].as_str().unwrap()).unwrap();
    let photo = entity::listing_photo::ActiveModel {
        id: Set(uuid::Uuid::new_v4()),
        tenant_id: Set(nw),
        listing_id: Set(lid),
        document_id: Set(uuid::Uuid::new_v4()),
        alt_text: Set("Living room".into()),
        caption: Set(None),
        position: Set(0),
        created_by: Set(None),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    }
    .insert(&c.db)
    .await
    .unwrap();

    // Turning it on needs someone to reach.
    let (st, on) = send_json(
        c,
        Method::Patch,
        "/syndication/zillow",
        &staff,
        serde_json::json!({ "enabled": true, "contact_email": "leasing@northwind.example", "contact_phone": "(503) 555-0100" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{on}");
    assert_eq!(on["enabled"], true);
    assert!(on["listings"].as_u64().unwrap() >= 1, "{on}");
    let (st, xml) = pull(path.clone()).await;
    assert_eq!(st, Status::Ok, "{xml}");
    assert!(xml.contains("<hotPadsItems version=\"2.1\">"), "{xml}");
    assert!(xml.contains("<name>The Maple Court</name>"));
    assert!(xml.contains("<price>1850</price>"));
    assert!(xml.contains("<zip>97214</zip>") && xml.contains("<state>OR</state>"));
    assert!(xml.contains("<contactEmail>leasing@northwind.example</contactEmail>"));
    assert!(xml.contains(&format!("/public/listing-photos/{}", photo.id)));
    assert!(!xml.contains("Leased"), "only ready listings");
    let (_, s) = get_json(c, "/syndication", &staff).await;
    assert_eq!(s["channels"][0]["pull_count"], 1);
    assert_eq!(s["channels"][0]["last_pull_agent"], "ZillowFeedBot/1.0");
    assert_eq!(
        s["channels"][0]["pulls"][0]["listings"],
        s["channels"][0]["listings"]
    );

    // Turned off for the portals: gone from the feed.
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/listings/{lid}"),
        &staff,
        serde_json::json!({ "syndicate": false }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    let (_, xml) = pull(path.clone()).await;
    assert!(!xml.contains("The Maple Court"));
    send_json(
        c,
        Method::Patch,
        &format!("/listings/{lid}"),
        &staff,
        serde_json::json!({ "syndicate": true }),
    )
    .await;

    // The MITS feed, previewed while it's off.
    let r = c
        .client
        .get("/syndication/mits/preview")
        .header(bearer(&staff))
        .dispatch()
        .await;
    assert_eq!(r.status(), Status::Ok);
    let mits = r.into_string().await.unwrap();
    assert!(
        mits.contains("<PhysicalProperty") && mits.contains("The Maple Court"),
        "{mits}"
    );
    let wrong = path.replace("/feeds/zillow/", "/feeds/mits/");
    assert_eq!(
        pull(wrong).await.0,
        Status::NotFound,
        "a token belongs to its channel"
    );

    // A new URL retires the old one.
    let (st, rot) = post_json(
        c,
        "/syndication/zillow/rotate",
        &staff,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Ok);
    let fresh = rot["feed_url"].as_str().unwrap();
    assert_ne!(fresh, url);
    assert_eq!(pull(path).await.0, Status::NotFound);
    let fresh_path = fresh
        .split_once("/feeds/")
        .map(|(_, p)| format!("/feeds/{p}"))
        .unwrap();
    assert_eq!(pull(fresh_path).await.0, Status::Ok);

    // Leasing agents can't turn channels on; scoped people don't see it.
    let reader = mint(c, Some(nw), false, &["listing:read"]);
    let (st, _) = send_json(
        c,
        Method::Patch,
        "/syndication/zillow",
        &reader,
        serde_json::json!({ "enabled": false }),
    )
    .await;
    assert_eq!(st, Status::Forbidden);

    // Leave Northwind as it was.
    let (st, _) = send_json(
        c,
        Method::Patch,
        "/syndication/zillow",
        &staff,
        serde_json::json!({ "enabled": false }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    entity::prelude::ListingPhoto::delete_many()
        .filter(entity::listing_photo::Column::Id.eq(photo.id))
        .exec(&c.db)
        .await
        .unwrap();
}

/// The resident reports a problem with a video and a comment; staff see it
/// marked as theirs, put an action kit on it, link a part to a store, and
/// work it with the buttons. The resident follows along: the public lines,
/// their own files, never staff receipts or internal steps.
async fn maintenance_actions_and_resident(c: &Ctx) {
    use rocket::http::Method;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    let nw = tenant_id(c, "northwind").await;
    let staff = mint(
        c,
        Some(nw),
        false,
        &["maintenance:read", "maintenance:manage", "property:read"],
    );
    let taylor = entity::prelude::User::find()
        .filter(entity::user::Column::Email.eq("taylor@example.com"))
        .one(&c.db)
        .await
        .unwrap()
        .expect("the seeded resident");
    let res =
        crate::auth::issue_access_token(&c.config, taylor.id, Some(nw), false, vec![]).unwrap();

    // The resident reports it, with a video.
    let (st, t) = post_json(
        c,
        "/my/tickets",
        &res,
        serde_json::json!({ "title": "Dishwasher leaking", "category": "appliance", "location": "Kitchen" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{t}");
    let tid = t["id"].as_str().unwrap().to_string();
    let (st, v) = post_json(
        c,
        &format!("/my/tickets/{tid}/photos"),
        &res,
        serde_json::json!({ "filename": "leak.mp4", "mime_type": "video/mp4", "size_bytes": 40_000_000 }),
    )
    .await;
    assert_eq!(st, Status::Ok, "a 40 MB video is fine: {v}");
    let vid = v["document"]["id"].as_str().unwrap().to_string();
    // The bytes land (a small stand-in for the video).
    let put = |url: &str| {
        let path = url
            .split_once("/storage/")
            .map(|(_, p)| format!("/storage/{p}"))
            .unwrap();
        async move {
            c.client
                .put(path)
                .body(vec![0u8; 64])
                .dispatch()
                .await
                .status()
        }
    };
    assert_eq!(put(v["upload_url"].as_str().unwrap()).await, Status::Ok);
    let (st, _) = post_json(
        c,
        &format!("/my/tickets/{tid}/photos"),
        &res,
        serde_json::json!({ "filename": "lease.pdf", "mime_type": "application/pdf", "size_bytes": 1000 }),
    )
    .await;
    assert_eq!(st, Status::BadRequest, "photos and videos only");
    let (st, _) = post_json(
        c,
        &format!("/my/tickets/{tid}/photos"),
        &res,
        serde_json::json!({ "filename": "long.mp4", "mime_type": "video/mp4", "size_bytes": 150_000_000 }),
    )
    .await;
    assert_eq!(st, Status::BadRequest, "too large");
    let (st, cmt) = post_json(
        c,
        &format!("/my/tickets/{tid}/comments"),
        &res,
        serde_json::json!({ "body": "It pools under the door", "document_ids": [vid] }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{cmt}");

    // Staff see it as the resident's, with the video.
    let (_, detail) = get_json(c, &format!("/tickets/{tid}"), &staff).await;
    let theirs = detail["comments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["action"] == "resident_comment")
        .expect("the resident's comment")
        .clone();
    assert_eq!(theirs["document_ids"][0], vid.as_str());
    let (_, files) = get_json(c, &format!("/tickets/{tid}/files"), &staff).await;
    assert!(
        files
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["kind"] == "video"),
        "{files}"
    );

    // An action kit goes on, and a part gets a store link.
    let (_, kits) = get_json(c, "/issue-templates", &staff).await;
    let kit = kits
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["name"] == "Replace dishwasher")
        .expect("an action kit")
        .clone();
    let (st, _) = post_json(
        c,
        &format!("/tickets/{tid}/kits"),
        &staff,
        serde_json::json!({ "issue_template_id": kit["id"] }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    let (_, detail) = get_json(c, &format!("/tickets/{tid}"), &staff).await;
    let part = detail["parts"][0]["id"].as_str().unwrap().to_string();
    let (st, p) = send_json(
        c,
        Method::Patch,
        &format!("/parts/{part}"),
        &staff,
        serde_json::json!({ "url": "https://www.homedepot.com/p/Whirlpool-24-in-Dishwasher/123" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{p}");
    assert_eq!(p["store"], "Home Depot");
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/parts/{part}"),
        &staff,
        serde_json::json!({ "url": "javascript:alert(1)" }),
    )
    .await;
    assert_eq!(st, Status::BadRequest);
    // A staff receipt on the work order.
    let (st, r) = post_json(
        c,
        &format!("/tickets/{tid}/uploads"),
        &staff,
        serde_json::json!({ "filename": "receipt.jpg", "mime_type": "image/jpeg", "size_bytes": 2000, "kind": "receipt" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{r}");
    assert_eq!(put(r["upload_url"].as_str().unwrap()).await, Status::Ok);

    // The buttons.
    let (_, actions) = get_json(c, "/ticket-actions", &staff).await;
    assert!(actions
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["key"] == "work_done"));
    let press = |action: &'static str, note: Option<&'static str>| {
        let staff = staff.clone();
        let tid = tid.clone();
        async move {
            post_json(
                c,
                &format!("/tickets/{tid}/actions"),
                &staff,
                serde_json::json!({ "action": action, "note": note }),
            )
            .await
        }
    };
    let (st, after) = press("on_my_way", None).await;
    assert_eq!(st, Status::Ok, "{after}");
    assert_eq!(after["status"], "open", "no status change");
    assert_eq!(
        press("diagnosed", None).await.0,
        Status::BadRequest,
        "say what you found"
    );
    assert_eq!(
        press("diagnosed", Some("Door gasket torn; replacing the unit."))
            .await
            .0,
        Status::Ok
    );
    let (_, held) = press("waiting_parts", None).await;
    assert_eq!(held["status"], "on_hold", "{held}");
    assert_eq!(held["waiting_on"], "parts");
    let (_, back) = press("parts_in", None).await;
    assert_eq!(back["status"], "in_progress");
    assert!(back["waiting_on"].is_null());
    let (_, tasks) = get_json(c, &format!("/tickets/{tid}/tasks"), &staff).await;
    let first = tasks[0]["id"].as_str().unwrap();
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/tickets/{tid}/tasks/{first}"),
        &staff,
        serde_json::json!({ "status": "done" }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    let (_, done) = press("work_done", None).await;
    assert_eq!(done["status"], "resolved");
    let (_, detail) = get_json(c, &format!("/tickets/{tid}"), &staff).await;
    let bodies: Vec<&str> = detail["comments"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kind"] == "action")
        .map(|c| c["body"].as_str().unwrap())
        .collect();
    assert!(bodies.contains(&"On my way."), "{bodies:?}");
    assert!(bodies.iter().any(|b| b.starts_with("Done: ")), "{bodies:?}");

    // What the resident sees.
    let (st, mine) = get_json(c, &format!("/my/tickets/{tid}"), &res).await;
    assert_eq!(st, Status::Ok, "{mine}");
    let lines: Vec<&str> = mine["comments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["body"].as_str().unwrap())
        .collect();
    assert!(lines.contains(&"On my way."), "{lines:?}");
    assert!(lines.contains(&"Diagnosed: Door gasket torn; replacing the unit."));
    assert!(lines.contains(&"Work complete."));
    assert!(
        !lines.iter().any(|l| l.starts_with("Done: ")),
        "internal steps stay internal"
    );
    let names: Vec<&str> = mine["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["filename"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["leak.mp4"], "their video, not staff receipts");
    assert_eq!(mine["files"][0]["kind"], "video");
}

/// The full property profile: permits, insurance, schools with their zone,
/// and action items fed by the "needs attention" rules.
async fn property_profile_records(c: &Ctx) {
    use rocket::http::Method;
    let nw = tenant_id(c, "northwind").await;
    let pid = property_ids(c, nw).await[0];
    let other = property_ids(c, nw).await[1];
    let staff = mint(c, Some(nw), false, &["property:read", "property:write"]);
    let reader = mint(c, Some(nw), false, &["property:read"]);
    let today = chrono::Utc::now().date_naive();
    let day = |n: i64| (today + chrono::Duration::days(n)).to_string();
    let base = format!("/properties/{pid}");

    // A permit about to lapse, with an inspection next week.
    let (st, p) = post_json(
        c,
        &format!("{base}/permits"),
        &staff,
        serde_json::json!({
            "description": "Rebuild rear deck", "kind": "building", "status": "issued",
            "permit_number": "B-2026-0412", "jurisdiction": "City of Portland",
            "expires_on": day(20), "inspection_on": day(5), "fee_cents": 41000
        }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{p}");
    assert_eq!(p["open"], true);
    assert_eq!(p["fee_label"], "$410");
    let permit = p["id"].as_str().unwrap().to_string();
    let (st, e) = post_json(
        c,
        &format!("{base}/permits"),
        &staff,
        serde_json::json!({ "description": "x", "expires_on": "next week" }),
    )
    .await;
    assert_eq!(st, Status::BadRequest, "{e}");
    let (st, _) = post_json(
        c,
        &format!("{base}/permits"),
        &staff,
        serde_json::json!({ "description": "x", "kind": "spaceship" }),
    )
    .await;
    assert_eq!(st, Status::BadRequest);
    let (st, _) = post_json(
        c,
        &format!("{base}/permits"),
        &staff,
        serde_json::json!({ "description": "x", "document_ids": [Uuid::new_v4()] }),
    )
    .await;
    assert_eq!(st, Status::BadRequest, "files must be this property's");
    let (st, _) = post_json(
        c,
        &format!("{base}/permits"),
        &reader,
        serde_json::json!({ "description": "x" }),
    )
    .await;
    assert_eq!(st, Status::Forbidden, "reading isn't writing");
    // A permit on one property isn't reachable through another.
    let (st, _) = send_json(
        c,
        Method::Delete,
        &format!("/properties/{other}/permits/{permit}"),
        &staff,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::NotFound);

    // Insurance renewing next month; a school the team says the home is zoned for.
    let (st, pol) = post_json(
        c,
        &format!("{base}/insurance"),
        &staff,
        serde_json::json!({
            "carrier": "Cascade Mutual", "kind": "property", "policy_number": "HO-88213",
            "effective_on": day(-335), "expires_on": day(30),
            "premium_cents": 184000, "coverage_cents": 65000000, "agent_name": "Dana Ruiz"
        }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{pol}");
    assert_eq!(pol["coverage_label"], "$650,000");
    let (st, _) = post_json(
        c,
        &format!("{base}/insurance"),
        &staff,
        serde_json::json!({ "carrier": "X", "effective_on": day(10), "expires_on": day(1) }),
    )
    .await;
    assert_eq!(st, Status::BadRequest, "renewal before start");
    let (st, sch) = post_json(
        c,
        &format!("{base}/schools"),
        &staff,
        serde_json::json!({
            "name": "Sunnyside Elementary", "level": "elementary", "district": "Portland SD 1J",
            "assigned": true, "rating": 8, "website": "https://sunnyside.pps.net"
        }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{sch}");
    assert_eq!(sch["source"], "manual");
    let school = sch["id"].as_str().unwrap().to_string();
    let (st, _) = post_json(
        c,
        &format!("{base}/schools"),
        &staff,
        serde_json::json!({ "name": "X", "level": "elementary", "rating": 11 }),
    )
    .await;
    assert_eq!(st, Status::BadRequest);

    // What needs attention.
    let (st, att) = get_json(c, &format!("{base}/attention"), &reader).await;
    assert_eq!(st, Status::Ok, "{att}");
    let keys: Vec<String> = att
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["key"].as_str().unwrap().to_string())
        .collect();
    let has = |k: &str| keys.iter().any(|x| x.starts_with(k));
    assert!(has(&format!("permit-expiring:{permit}")), "{keys:?}");
    assert!(has(&format!("permit-inspection:{permit}")), "{keys:?}");
    assert!(has("policy-renew:"), "{keys:?}");
    assert!(has(&format!("school-zone:{school}")), "{keys:?}");
    assert!(!has("policy-none"), "a property policy is on file");

    // Turn one into an action item; it stops being suggested, and twice is refused.
    let renew = att
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["key"].as_str().unwrap().starts_with("policy-renew:"))
        .unwrap()
        .clone();
    let (st, item) = post_json(
        c,
        &format!("{base}/action-items"),
        &staff,
        serde_json::json!({
            "title": renew["title"], "subject_type": "insurance", "subject_id": renew["subject_id"],
            "due_on": renew["due_on"], "priority": renew["priority"], "suggestion_key": renew["key"]
        }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{item}");
    assert_eq!(item["status"], "open");
    let item_id = item["id"].as_str().unwrap().to_string();
    let (st, _) = post_json(
        c,
        &format!("{base}/action-items"),
        &staff,
        serde_json::json!({ "title": "again", "suggestion_key": renew["key"] }),
    )
    .await;
    assert_eq!(st, Status::Conflict);
    let (_, att) = get_json(c, &format!("{base}/attention"), &reader).await;
    assert!(!att
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["key"] == renew["key"]));

    // A to-do of the team's own, overdue.
    let (st, mine) = post_json(
        c,
        &format!("{base}/action-items"),
        &staff,
        serde_json::json!({ "title": "Walk the roof after the storm", "due_on": day(-2), "priority": "high" }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    assert_eq!(mine["overdue"], true);
    let (_, open) = get_json(c, &format!("{base}/action-items"), &reader).await;
    assert_eq!(
        open[0]["title"], "Walk the roof after the storm",
        "high first"
    );

    // Tick the renewal off.
    let (st, done) = send_json(
        c,
        Method::Patch,
        &format!("{base}/action-items/{item_id}"),
        &staff,
        serde_json::json!({ "status": "done" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{done}");
    assert!(done["completed_at"].is_string());
    let (_, open) = get_json(c, &format!("{base}/action-items"), &reader).await;
    assert!(!open
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["id"] == item_id.as_str()));
    let (_, fin) = get_json(c, &format!("{base}/action-items?status=done"), &reader).await;
    assert!(fin
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["id"] == item_id.as_str()));

    // Confirming the school zone clears that suggestion; finaling the permit
    // clears both permit suggestions.
    let (st, _) = send_json(
        c,
        Method::Put,
        &format!("{base}/schools/{school}"),
        &staff,
        serde_json::json!({
            "name": "Sunnyside Elementary", "level": "elementary", "assigned": true,
            "district": "Portland SD 1J", "zone_verified_on": day(0)
        }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    let (st, fp) = send_json(
        c,
        Method::Put,
        &format!("{base}/permits/{permit}"),
        &staff,
        serde_json::json!({ "description": "Rebuild rear deck", "status": "finaled", "permit_number": "B-2026-0412" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{fp}");
    assert_eq!(fp["open"], false);
    assert_eq!(
        fp["finaled_on"],
        day(0).as_str(),
        "finaled today by default"
    );
    let (_, att) = get_json(c, &format!("{base}/attention"), &reader).await;
    let keys: Vec<&str> = att
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["key"].as_str().unwrap())
        .collect();
    assert!(!keys.iter().any(|k| k.starts_with("permit-")), "{keys:?}");
    assert!(
        !keys.contains(&format!("school-zone:{school}").as_str()),
        "{keys:?}"
    );

    // The schools list puts the zoned school first among its level.
    let (_, list) = get_json(c, &format!("{base}/schools"), &reader).await;
    assert!(list
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["name"] == "Sunnyside Elementary" && s["assigned"] == true));
}

/// Assigned techs and queues: who can take work, a task given to a person,
/// their own queue, and several tasks sent to a vendor as one job.
async fn desk_queues_and_vendor_batches(c: &Ctx) {
    use rocket::http::Method;
    let nw = tenant_id(c, "northwind").await;
    let pid = property_ids(c, nw).await[0];
    let staff = mint(
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

    // Who can take work on this property.
    let (st, techs) = get_json(c, &format!("/ticket-techs?property_id={pid}"), &staff).await;
    assert_eq!(st, Status::Ok, "{techs}");
    let techs = techs.as_array().unwrap().clone();
    assert!(!techs.is_empty(), "somebody on the team can take it");
    assert!(
        techs.iter().all(|t| t["role"] != "renter"),
        "residents aren't offered"
    );
    let load = |t: &serde_json::Value| {
        t["open_tickets"].as_i64().unwrap() + t["open_tasks"].as_i64().unwrap()
    };
    let tech = techs[0].clone();
    let tech_id = tech["user_id"].as_str().unwrap().to_string();
    let tech_token = crate::auth::issue_access_token(
        &c.config,
        Uuid::parse_str(&tech_id).unwrap(),
        Some(nw),
        false,
        vec!["maintenance:read".into(), "maintenance:manage".into()],
    )
    .unwrap();
    let before = load(&tech);

    // A work order from a kit, with its tasks.
    let (_, kits) = get_json(c, "/issue-templates", &staff).await;
    let kit = kits
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["name"] == "Replace dishwasher")
        .expect("the dishwasher kit")
        .clone();
    assert_eq!(kit["kit_key"], "replace-dishwasher");
    let (st, g) = post_json(
        c,
        &format!("/issue-templates/{}/generate", kit["id"].as_str().unwrap()),
        &staff,
        serde_json::json!({ "property_id": pid }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{g}");
    let tid = g["ticket"]["id"].as_str().unwrap().to_string();
    let (_, tasks) = get_json(c, &format!("/tickets/{tid}/tasks"), &staff).await;
    let tasks = tasks.as_array().unwrap().clone();
    assert!(tasks.len() >= 3);
    let (t1, t2, t3) = (
        tasks[0]["id"].as_str().unwrap().to_string(),
        tasks[1]["id"].as_str().unwrap().to_string(),
        tasks[2]["id"].as_str().unwrap().to_string(),
    );

    // Give a task to a teammate; a stranger and a resident are refused.
    let (st, out) = send_json(
        c,
        Method::Patch,
        &format!("/tickets/{tid}/tasks/{t1}"),
        &staff,
        serde_json::json!({ "assignee_user_id": tech_id }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{out}");
    assert_eq!(out[0]["assignee_user_id"], tech_id.as_str());
    assert_eq!(out[0]["assignee_user_name"], tech["name"]);
    for who in [
        Uuid::new_v4(),
        entity::prelude::User::find()
            .filter(entity::user::Column::Email.eq("taylor@example.com"))
            .one(&c.db)
            .await
            .unwrap()
            .unwrap()
            .id,
    ] {
        let (st, _) = send_json(
            c,
            Method::Patch,
            &format!("/tickets/{tid}/tasks/{t2}"),
            &staff,
            serde_json::json!({ "assignee_user_id": who.to_string() }),
        )
        .await;
        assert_eq!(st, Status::BadRequest, "not on the team: {who}");
    }

    // It's in their queue, not anyone else's, and their load went up.
    let (st, q) = get_json(c, "/ticket-queue", &tech_token).await;
    assert_eq!(st, Status::Ok, "{q}");
    let mine = q["tasks"].as_array().unwrap();
    let row = mine
        .iter()
        .find(|t| t["task_id"] == t1.as_str())
        .expect("in my queue");
    assert_eq!(row["ticket_id"], tid.as_str());
    assert_eq!(row["property_id"], pid.to_string());
    let (_, other) = get_json(c, "/ticket-queue", &staff).await;
    assert!(!other["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["task_id"] == t1.as_str()));
    let (_, techs2) = get_json(c, &format!("/ticket-techs?property_id={pid}"), &staff).await;
    let after = techs2
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["user_id"] == tech_id.as_str())
        .unwrap();
    assert_eq!(load(after), before + 1);

    // Starting it keeps it in the queue; finishing takes it out.
    send_json(
        c,
        Method::Patch,
        &format!("/tickets/{tid}/tasks/{t1}"),
        &tech_token,
        serde_json::json!({ "status": "doing" }),
    )
    .await;
    let (_, q) = get_json(c, "/ticket-queue", &tech_token).await;
    let row = q["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["task_id"] == t1.as_str())
        .unwrap();
    assert_eq!(row["status"], "doing");
    send_json(
        c,
        Method::Patch,
        &format!("/tickets/{tid}/tasks/{t1}"),
        &tech_token,
        serde_json::json!({ "status": "done" }),
    )
    .await;
    let (_, q) = get_json(c, "/ticket-queue", &tech_token).await;
    assert!(!q["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["task_id"] == t1.as_str()));

    // The work order list shows who has it and how far along it is.
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/tickets/{tid}"),
        &staff,
        serde_json::json!({ "assignee_user_id": tech_id }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    let find = |list: &serde_json::Value| {
        list.as_array()
            .unwrap()
            .iter()
            .find(|t| t["id"] == tid.as_str())
            .unwrap()
            .clone()
    };
    let (_, list) = get_json(c, "/tickets", &staff).await;
    let row = find(&list);
    assert_eq!(row["assignee_kind"], "tech");
    assert_eq!(row["assignee_name"], tech["name"]);
    assert_eq!(row["tasks_done"], 1);
    assert_eq!(row["tasks_total"].as_i64().unwrap(), tasks.len() as i64);
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/tickets/{tid}"),
        &staff,
        serde_json::json!({ "clear_assignee_user": true }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    let (_, list) = get_json(c, "/tickets", &staff).await;
    assert!(find(&list)["assignee_name"].is_null());

    // Two tasks go to one vendor as one job (one email listing both); a
    // finished task can't be sent; a vendor with no email is refused.
    let (st, v) = post_json(
        c,
        "/entities",
        &staff,
        serde_json::json!({ "kind": "contractor", "name": "Batch Appliance Co",
            "email": "jobs@batchappliance.example", "trades": ["appliance"] }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{v}");
    let vid = v["id"].as_str().unwrap().to_string();
    let (st, e) = post_json(
        c,
        &format!("/tickets/{tid}/dispatch-tasks"),
        &staff,
        serde_json::json!({ "task_ids": [t1], "entity_id": vid }),
    )
    .await;
    assert_eq!(st, Status::Conflict, "a finished task isn't sent: {e}");
    let (st, _) = post_json(
        c,
        &format!("/tickets/{tid}/dispatch-tasks"),
        &staff,
        serde_json::json!({ "task_ids": [], "entity_id": vid }),
    )
    .await;
    assert_eq!(st, Status::BadRequest);
    let (st, sent) = post_json(
        c,
        &format!("/tickets/{tid}/dispatch-tasks"),
        &staff,
        serde_json::json!({ "task_ids": [t2, t3], "entity_id": vid, "note": "Gate code 4411" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{sent}");
    for id in [&t2, &t3] {
        let row = sent
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["id"] == id.as_str())
            .unwrap();
        assert_eq!(row["assignee_name"], "Batch Appliance Co");
        assert_eq!(row["dispatch_via"], "email");
        assert_eq!(row["dispatch_note"], "Gate code 4411");
        assert!(row["dispatched_at"].is_string());
    }
    let mails: Vec<_> = entity::prelude::BackgroundJob::find()
        .filter(entity::background_job::Column::TenantId.eq(nw))
        .filter(entity::background_job::Column::Kind.eq("auto_email"))
        .all(&c.db)
        .await
        .unwrap()
        .into_iter()
        .filter(|j| j.payload["to"] == "jobs@batchappliance.example")
        .collect();
    assert_eq!(mails.len(), 1, "one job, one email");
    let body = mails[0].payload["vars"]["description"].as_str().unwrap();
    assert!(body.starts_with("Tasks:"), "{body}");
    assert!(body.contains(tasks[1]["title"].as_str().unwrap()));
    assert!(body.contains(tasks[2]["title"].as_str().unwrap()));
    assert!(body.contains("Gate code 4411"));
    let (_, detail) = get_json(c, &format!("/tickets/{tid}"), &staff).await;
    assert!(detail["comments"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["action"] == "task_sent"
            && m["body"]
                .as_str()
                .unwrap()
                .starts_with("Sent to Batch Appliance Co")));
    // The vendor and the work order are on a shared list.
    let (st, none) = post_json(
        c,
        "/entities",
        &staff,
        serde_json::json!({ "kind": "contractor", "name": "No Email Co" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{none}");
    let (_, fresh) = get_json(c, &format!("/tickets/{tid}/tasks"), &staff).await;
    let spare = fresh
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["dispatched_at"].is_null() && t["status"] == "todo");
    if let Some(spare) = spare {
        let (st, _) = post_json(
            c,
            &format!("/tickets/{tid}/dispatch-tasks"),
            &staff,
            serde_json::json!({ "task_ids": [spare["id"]], "entity_id": none["id"] }),
        )
        .await;
        assert_eq!(st, Status::BadRequest, "no email on file");
    }
}

/// The description and features, and the history list.
async fn property_story_and_timeline(c: &Ctx) {
    use rocket::http::Method;
    let nw = tenant_id(c, "northwind").await;
    let pid = property_ids(c, nw).await[0];
    let staff = mint(c, Some(nw), false, &["property:read", "property:write"]);
    let reader = mint(c, Some(nw), false, &["property:read"]);
    let base = format!("/properties/{pid}");

    let (st, s) = send_json(
        c,
        Method::Put,
        &format!("{base}/story"),
        &staff,
        serde_json::json!({
            "description": "  Corner fourplex, a block from the light rail.  ",
            "features": {
                "Interior": ["Flooring: Hardwood", " flooring: hardwood ", "", "Gas fireplace"],
                "exterior": ["Fenced yard"]
            }
        }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{s}");
    assert_eq!(
        s["description"],
        "Corner fourplex, a block from the light rail."
    );
    assert_eq!(
        s["features"]["interior"],
        serde_json::json!(["Flooring: Hardwood", "Gas fireplace"])
    );
    // It comes back with the property's details.
    let (_, intel) = get_json(c, &format!("{base}/intel"), &reader).await;
    assert_eq!(intel["detail"]["features"]["exterior"][0], "Fenced yard");
    assert!(intel["detail"]["description"]
        .as_str()
        .unwrap()
        .starts_with("Corner"));
    // A group left out is cleared; an unknown group, or reading-only access, is refused.
    let (_, s2) = send_json(
        c,
        Method::Put,
        &format!("{base}/story"),
        &staff,
        serde_json::json!({ "features": { "interior": ["Gas fireplace"] } }),
    )
    .await;
    assert!(s2["features"]["exterior"].is_null());
    assert_eq!(
        s2["description"], s["description"],
        "the description stays when not sent"
    );
    let (st, _) = send_json(
        c,
        Method::Put,
        &format!("{base}/story"),
        &staff,
        serde_json::json!({ "features": { "garage": ["x"] } }),
    )
    .await;
    assert_eq!(st, Status::BadRequest);
    let (st, _) = send_json(
        c,
        Method::Put,
        &format!("{base}/story"),
        &reader,
        serde_json::json!({ "description": "x" }),
    )
    .await;
    assert_eq!(st, Status::Forbidden);

    // The history is newest first and mixes the sources.
    let (st, tl) = get_json(c, &format!("{base}/timeline"), &reader).await;
    assert_eq!(st, Status::Ok, "{tl}");
    let events = tl.as_array().unwrap();
    assert!(!events.is_empty());
    let dates: Vec<&str> = events.iter().map(|e| e["date"].as_str().unwrap()).collect();
    let mut sorted = dates.clone();
    sorted.sort_by(|a, b| b.cmp(a));
    assert_eq!(dates, sorted, "newest first");
    let kinds: std::collections::BTreeSet<&str> =
        events.iter().map(|e| e["kind"].as_str().unwrap()).collect();
    for k in ["built", "estimate", "tax", "lease"] {
        assert!(kinds.contains(k), "{k} in {kinds:?}");
    }
    assert!(events
        .iter()
        .filter(|e| e["kind"] == "lease")
        .all(|e| e["monthly"] == true && e["amount_label"].as_str().unwrap().ends_with("/mo")));
}

/// Appointments: offered windows reach the resident, who picks one in the
/// portal; the work order follows; reminders go once; the public link lets
/// anyone pick or decline with another time; staff confirm by phone.
/// A vendor without an account answers from the link in their dispatch email:
/// accepts with a time (which books the visit), sends a photo and an invoice,
/// marks it done. Another batch is declined and goes back to unassigned. The
/// office invites a vendor to Alpha.
async fn vendor_link_flow(c: &Ctx) {
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    let nw = tenant_id(c, "northwind").await;
    let pid = property_ids(c, nw).await[0];
    let staff = mint(
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
    let (st, vendor) = post_json(
        c,
        "/entities",
        &staff,
        serde_json::json!({ "kind": "contractor", "name": "Ray's Plumbing", "contact_name": "Ray",
            "email": "ray@plumbing.example", "trades": ["plumbing"] }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{vendor}");
    let vid = vendor["id"].as_str().unwrap().to_string();

    let (st, t) = post_json(
        c,
        &format!("/properties/{pid}/tickets"),
        &staff,
        serde_json::json!({ "title": "Kitchen sink backs up", "category": "plumbing", "priority": "high" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{t}");
    let tid = t["id"].as_str().unwrap().to_string();
    let mut ids = vec![];
    for title in ["Snake the drain", "Replace P-trap", "Caulk the sink"] {
        let (st, tasks) = post_json(
            c,
            &format!("/tickets/{tid}/tasks"),
            &staff,
            serde_json::json!({ "title": title, "trade": "plumbing", "needs_contractor": true }),
        )
        .await;
        assert_eq!(st, Status::Ok, "{tasks}");
        ids.push(
            tasks
                .as_array()
                .unwrap()
                .iter()
                .find(|x| x["title"] == title)
                .unwrap()["id"]
                .as_str()
                .unwrap()
                .to_string(),
        );
    }

    // Two tasks go to Ray by email; the email carries one link for both.
    let (st, sent) = post_json(
        c,
        &format!("/tickets/{tid}/dispatch-tasks"),
        &staff,
        serde_json::json!({ "task_ids": [ids[0], ids[1]], "entity_id": vid, "note": "Resident home after 4" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{sent}");
    let job = entity::prelude::BackgroundJob::find()
        .filter(entity::background_job::Column::TenantId.eq(nw))
        .all(&c.db)
        .await
        .unwrap()
        .into_iter()
        .filter(|j| j.payload["template"] == "ticket_dispatch")
        .max_by_key(|j| j.created_at)
        .expect("the dispatch email");
    let link = job.payload["vars"]["vendor_link"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(link.contains("/vendor/"), "{link}");
    let token = link.rsplit('/').next().unwrap().to_string();
    let public = |path: String| async move {
        let r = c.client.get(path).dispatch().await;
        let st = r.status();
        (
            st,
            r.into_json::<serde_json::Value>()
                .await
                .unwrap_or(serde_json::Value::Null),
        )
    };
    let post_public = |path: String, body: serde_json::Value| async move {
        let r = c
            .client
            .post(path)
            .header(ContentType::JSON)
            .body(body.to_string())
            .dispatch()
            .await;
        let st = r.status();
        (
            st,
            r.into_json::<serde_json::Value>()
                .await
                .unwrap_or(serde_json::Value::Null),
        )
    };
    let (st, view) = public(format!("/public/vendor/{token}")).await;
    assert_eq!(st, Status::Ok, "{view}");
    assert_eq!(view["title"], "Kitchen sink backs up");
    assert_eq!(view["vendor"], "Ray's Plumbing");
    assert_eq!(view["tasks"].as_array().unwrap().len(), 2);
    assert_eq!(view["note"], "Resident home after 4");
    assert!(view["response"].is_null());
    assert!(view["company"].as_str().unwrap().contains("Northwind"));
    let (st, _) = public("/public/vendor/not-a-real-token-at-all".into()).await;
    assert_eq!(st, Status::NotFound);

    // Ray accepts and says when: the visit is on the calendar, the work order
    // is scheduled, and the office hears.
    let when = (chrono::Utc::now() + chrono::Duration::days(2))
        .date_naive()
        .and_hms_opt(15, 0, 0)
        .unwrap()
        .and_utc()
        .to_rfc3339();
    let (st, acc) = post_public(
        format!("/public/vendor/{token}/accept"),
        serde_json::json!({ "start": when, "note": "Bringing a 50ft auger" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{acc}");
    assert_eq!(acc["response"], "accepted");
    assert_eq!(acc["response_note"], "Bringing a 50ft auger");
    assert!(acc["when_words"].as_str().unwrap().contains(" to "));
    let (_, detail) = get_json(c, &format!("/tickets/{tid}"), &staff).await;
    assert_eq!(detail["status"], "scheduled");
    assert!(detail["comments"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["action"] == "vendor_accepted" && m["author_name"] == "Ray's Plumbing"));
    let (_, tasks) = get_json(c, &format!("/tickets/{tid}/tasks"), &staff).await;
    let snake = tasks
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["id"] == ids[0].as_str())
        .unwrap();
    assert_eq!(snake["vendor_response"], "accepted");
    assert_eq!(snake["status"], "doing");
    let (_, cal) = get_json(
        c,
        &format!("/appointments?subject_type=ticket&subject_id={tid}"),
        &staff,
    )
    .await;
    let visit = cal
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["status"] == "confirmed")
        .expect("the vendor's visit");
    assert_eq!(visit["vendor_entity_id"], vid.as_str());
    assert_eq!(visit["confirmed_by"], "vendor");
    let heard = entity::prelude::BackgroundJob::find()
        .filter(entity::background_job::Column::TenantId.eq(nw))
        .all(&c.db)
        .await
        .unwrap()
        .into_iter()
        .filter(|j| j.payload["template"] == "vendor_task_accepted")
        .count();
    assert!(heard >= 1, "staff were told");

    // A photo and the invoice, then done. The invoice is an expense billable
    // to the owner; the resident sees the done line.
    let (st, up) = post_public(
        format!("/public/vendor/{token}/uploads"),
        serde_json::json!({ "filename": "after.jpg", "mime_type": "image/jpeg", "size_bytes": 4 }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{up}");
    assert_eq!(up["file"]["kind"], "photo");
    let url = up["upload_url"].as_str().unwrap();
    let path = &url[url.find("/storage/local/").unwrap()..];
    let resp = c
        .client
        .put(path.to_string())
        .body(vec![0xFF, 0xD8, 0xFF, 0xD9])
        .dispatch()
        .await;
    assert!(resp.status().code < 300);
    let (st, inv) = post_public(
        format!("/public/vendor/{token}/uploads"),
        serde_json::json!({ "filename": "invoice.pdf", "mime_type": "application/pdf", "size_bytes": 10, "kind": "invoice" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{inv}");
    assert_eq!(inv["file"]["kind"], "receipt");
    let inv_id = inv["file"]["id"].as_str().unwrap().to_string();
    let (st, _) = post_public(
        format!("/public/vendor/{token}/invoice"),
        serde_json::json!({ "amount_cents": 0 }),
    )
    .await;
    assert_eq!(st, Status::BadRequest);
    let (st, billed) = post_public(
        format!("/public/vendor/{token}/invoice"),
        serde_json::json!({ "amount_cents": 38500, "description": "Drain cleared, new P-trap", "document_id": inv_id }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{billed}");
    assert_eq!(billed["invoices"][0]["amount_label"], "$385.00");
    let (st, fin) = post_public(
        format!("/public/vendor/{token}/done"),
        serde_json::json!({ "note": "Flowing clear" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{fin}");
    assert_eq!(fin["response"], "done");
    assert!(fin["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .all(|x| x["status"] == "done"));
    assert_eq!(
        fin["files"].as_array().unwrap().len(),
        1,
        "the stored photo"
    );
    let (_, expenses) = get_json(c, &format!("/tickets/{tid}/expenses"), &staff).await;
    let e = expenses
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["vendor"] == "Ray's Plumbing")
        .expect("the vendor's invoice as an expense");
    assert_eq!(e["amount_cents"], 38500);
    assert_eq!(e["billable_to_owner"], true);
    let (_, detail) = get_json(c, &format!("/tickets/{tid}"), &staff).await;
    assert!(detail["comments"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["action"] == "vendor_done" && m["visibility"] == "public"));
    // Once done, declining is refused.
    let (st, _) = post_public(
        format!("/public/vendor/{token}/decline"),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Conflict);

    // The third task goes to Ray too, and Ray declines: it's unassigned again.
    let (st, _) = post_json(
        c,
        &format!("/tickets/{tid}/dispatch-tasks"),
        &staff,
        serde_json::json!({ "task_ids": [ids[2]], "entity_id": vid }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    let job2 = entity::prelude::BackgroundJob::find()
        .filter(entity::background_job::Column::TenantId.eq(nw))
        .all(&c.db)
        .await
        .unwrap()
        .into_iter()
        .filter(|j| j.payload["template"] == "ticket_dispatch")
        .max_by_key(|j| j.created_at)
        .unwrap();
    let token2 = job2.payload["vars"]["vendor_link"]
        .as_str()
        .unwrap()
        .rsplit('/')
        .next()
        .unwrap()
        .to_string();
    assert_ne!(token, token2, "each batch has its own link");
    let (st, dec) = post_public(
        format!("/public/vendor/{token2}/decline"),
        serde_json::json!({ "reason": "Booked solid this month" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{dec}");
    assert_eq!(dec["response"], "declined");
    assert_eq!(
        dec["vendor"], "Ray's Plumbing",
        "the page still knows who declined"
    );
    let (_, tasks) = get_json(c, &format!("/tickets/{tid}/tasks"), &staff).await;
    let caulk = tasks
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["id"] == ids[2].as_str())
        .unwrap();
    assert!(caulk["assignee_entity_id"].is_null());
    assert!(caulk["dispatched_at"].is_null());
    assert_eq!(caulk["vendor_response"], "declined");
    assert_eq!(
        caulk["vendor_note"].as_str().unwrap(),
        "Booked solid this month"
    );
    let told = entity::prelude::BackgroundJob::find()
        .filter(entity::background_job::Column::TenantId.eq(nw))
        .all(&c.db)
        .await
        .unwrap()
        .into_iter()
        .filter(|j| j.payload["template"] == "vendor_task_declined")
        .count();
    assert!(told >= 1);
    let (st, _) = post_public(
        format!("/public/vendor/{token2}/done"),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Conflict);

    // Invite Ray to Alpha: an email with a prefilled sign-up link, remembered.
    let (st, inv) = post_json(
        c,
        &format!("/entities/{vid}/alpha-invite"),
        &staff,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Ok, "{inv}");
    let join = inv["join_url"].as_str().unwrap();
    assert!(join.contains("/partners/join?"), "{join}");
    assert!(join.contains("email=ray%40plumbing.example"), "{join}");
    assert!(join.contains("from=Northwind"), "{join}");
    let invited = entity::prelude::BackgroundJob::find()
        .filter(entity::background_job::Column::TenantId.eq(nw))
        .all(&c.db)
        .await
        .unwrap()
        .into_iter()
        .filter(|j| j.payload["template"] == "alpha_invite")
        .count();
    assert_eq!(invited, 1);
    let (_, options) = get_json(c, &format!("/tickets/{tid}/vendors"), &staff).await;
    let ray = options
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["id"] == vid.as_str())
        .unwrap();
    assert!(ray["alpha_invited_at"].is_string());
    let (st, _) = post_json(
        c,
        &format!("/entities/{}/alpha-invite", Uuid::new_v4()),
        &staff,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::NotFound);
}

/// A landlord on a phone: a walk-in lead gets a showing booked (the prospect's
/// details come from the lead), the showing is marked done (the lead is
/// toured), the application link goes out, and the public form filed from
/// that link attaches to the lead.
async fn showings_flow(c: &Ctx) {
    use rocket::http::{Header, Method};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    let nw = tenant_id(c, "northwind").await;
    let pid = property_ids(c, nw).await[0];
    let staff = mint(
        c,
        Some(nw),
        false,
        &[
            "maintenance:read",
            "maintenance:manage",
            "application:read",
            "application:write",
            "property:read",
        ],
    );
    let (st, lead) = post_json(
        c,
        "/leads",
        &staff,
        serde_json::json!({ "name": "Priya Natarajan", "email": "priya@example.com", "phone": "503-555-0142", "source": "walk_in" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{lead}");
    let lid = lead["id"].as_str().unwrap().to_string();

    // Book the showing: no title or contact given; the lead supplies them.
    let start = (chrono::Utc::now() + chrono::Duration::days(1))
        .date_naive()
        .and_hms_opt(17, 0, 0)
        .unwrap()
        .and_utc()
        .to_rfc3339();
    let (st, a) = post_json(
        c,
        "/appointments",
        &staff,
        serde_json::json!({ "property_id": pid, "lead_id": lid, "windows": [{ "start": start }] }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{a}");
    assert_eq!(a["kind"], "showing");
    assert_eq!(a["subject_type"], "lead");
    assert_eq!(a["with_role"], "prospect");
    assert_eq!(a["with_name"], "Priya Natarajan");
    assert_eq!(a["with_email"], "priya@example.com");
    assert!(a["title"].as_str().unwrap().starts_with("Showing: "));
    let aid = a["id"].as_str().unwrap().to_string();
    let (st, a) = send_json(
        c,
        Method::Patch,
        &format!("/appointments/{aid}"),
        &staff,
        serde_json::json!({ "confirm": { "start": start } }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{a}");
    assert_eq!(a["status"], "confirmed");
    // It's in the showings list.
    let (_, list) = get_json(c, "/appointments?status=confirmed", &staff).await;
    assert!(list
        .as_array()
        .unwrap()
        .iter()
        .any(|x| x["id"] == aid.as_str() && x["kind"] == "showing"));

    // They came: the lead is toured.
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/appointments/{aid}"),
        &staff,
        serde_json::json!({ "status": "done" }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    let (_, leads) = get_json(c, "/leads", &staff).await;
    let l = leads["leads"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["id"] == lid.as_str())
        .unwrap()
        .clone();
    assert_eq!(l["status"], "toured");

    // Send the application: email and text, with a prefilled link.
    let (st, inv) = post_json(
        c,
        &format!("/leads/{lid}/invite"),
        &staff,
        serde_json::json!({ "message": "Great meeting you" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{inv}");
    let url = inv["apply_url"].as_str().unwrap().to_string();
    assert!(url.contains("/apply?tenant=northwind&lead="), "{url}");
    assert!(url.contains("email=priya%40example.com"), "{url}");
    let sent = entity::prelude::BackgroundJob::find()
        .filter(entity::background_job::Column::TenantId.eq(nw))
        .all(&c.db)
        .await
        .unwrap()
        .into_iter()
        .filter(|j| j.payload["template"] == "application_invite")
        .count();
    assert!(sent >= 2, "email and text went out, got {sent}");

    // Priya applies from the link: the application attaches to the lead.
    let resp = c
        .client
        .post("/public/applications")
        .header(Header::new("X-Tenant", "northwind"))
        .header(ContentType::JSON)
        .body(
            serde_json::json!({
                "lead_id": lid,
                "applicant_name": "Priya Natarajan",
                "email": "priya@example.com",
                "phone": "503-555-0142",
                "annual_income_cents": 9_600_000,
                "move_in": "2026-11-01",
                "screening_consent": true
            })
            .to_string(),
        )
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    let applied: serde_json::Value = resp.into_json().await.unwrap();
    let app_id = applied["application_id"].as_str().unwrap().to_string();
    let (_, leads) = get_json(c, "/leads", &staff).await;
    let l = leads["leads"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["id"] == lid.as_str())
        .unwrap()
        .clone();
    assert_eq!(l["status"], "applied");
    assert_eq!(l["application_id"], app_id.as_str());
    // Inviting again is refused: they've applied.
    let (st, _) = post_json(
        c,
        &format!("/leads/{lid}/invite"),
        &staff,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Conflict);
}

/// Property data settings and the crime source: the workspace's provider
/// choices show in settings and the live status, the crime source files a row
/// (simulated here, the FBI when live), the profile returns it, and an area
/// well above the state's rate becomes an attention suggestion.
async fn property_data_sources(c: &Ctx) {
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
    let nw = tenant_id(c, "northwind").await;
    let pid = property_ids(c, nw).await[0];
    let staff = mint(
        c,
        Some(nw),
        false,
        &["property:read", "property:write", "tenant:manage"],
    );
    let (st, settings) = get_json(c, "/settings", &staff).await;
    assert_eq!(st, Status::Ok, "{settings}");
    let keys: Vec<&str> = settings
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s["key"].as_str())
        .collect();
    for k in [
        "property_data.crime_provider",
        "property_data.records_provider",
        "property_data.refresh_days",
    ] {
        assert!(keys.contains(&k), "setting {k} is in the catalog");
    }
    let (st, live) = get_json(c, "/property-data/live", &staff).await;
    assert_eq!(st, Status::Ok, "{live}");
    assert_eq!(live["crime_provider"], "fbi");
    assert_eq!(live["records_provider"], "simulated");
    assert_eq!(live["records_live"], false);
    assert_eq!(live["crime_key_set"], false);

    // The crime source runs (no key and no LIVE_PROVIDERS here: simulated).
    let property = entity::prelude::Property::find_by_id(pid)
        .one(&c.db)
        .await
        .unwrap()
        .unwrap();
    let out =
        crate::enrichment::runner::run_source(&c.db, &property, crate::enrichment::Source::Crime)
            .await
            .unwrap();
    assert_eq!(out.provider, "simulated");
    assert!(!out.fell_back);
    let (st, intel) = get_json(c, &format!("/properties/{pid}/intel"), &staff).await;
    assert_eq!(st, Status::Ok, "{intel}");
    let crime = &intel["crime"];
    assert_eq!(crime["simulated"], true);
    assert_eq!(crime["offenses"].as_array().unwrap().len(), 4);
    assert!(crime["agency_name"].as_str().unwrap().contains("Police"));
    assert!(crime["verdict_words"].as_str().unwrap().len() > 5);
    // Running again replaces the row rather than adding one.
    crate::enrichment::runner::run_source(&c.db, &property, crate::enrichment::Source::Crime)
        .await
        .unwrap();
    let rows = entity::prelude::PropertyCrime::find()
        .filter(entity::property_crime::Column::PropertyId.eq(pid))
        .all(&c.db)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);

    // An area well above the state's rate is flagged on the profile.
    let mut am: entity::property_crime::ActiveModel = rows[0].clone().into();
    am.verdict = Set("well_above".into());
    am.update(&c.db).await.unwrap();
    let (st, att) = get_json(c, &format!("/properties/{pid}/attention"), &staff).await;
    assert_eq!(st, Status::Ok, "{att}");
    assert!(
        att.as_array()
            .unwrap()
            .iter()
            .any(|s| s["key"] == "crime-high"),
        "{att}"
    );

    // The nightly refresh picks up stale properties, a few at a time, and
    // skips a fresh one.
    let due = crate::enrichment::refresh::due(&c.db, nw, 30, 3)
        .await
        .unwrap();
    assert!(due.len() <= 3);
    // Turning the refresh off makes the job a no-op.
    let (st, _) = send_json(
        c,
        rocket::http::Method::Put,
        "/settings/property_data.refresh_days",
        &staff,
        serde_json::json!({ "value": 0 }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    let (_, live) = get_json(c, "/property-data/live", &staff).await;
    assert_eq!(live["crime_provider"], "fbi");
}

/// Needs attention everywhere: the code-required items catalog seeds routines
/// per property, routines coming due and work orders with no date show under
/// "To schedule", a routine can be opened early, and the portfolio rollup
/// carries it all.
async fn attention_and_mandates(c: &Ctx) {
    use rocket::http::Method;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    let nw = tenant_id(c, "northwind").await;
    let staff = mint(
        c,
        Some(nw),
        false,
        &["maintenance:read", "maintenance:manage", "property:read"],
    );
    // A Northwind property made into an eight-unit Portland, OR building from
    // 2016, so the catalog's rules have something to bite on.
    let maple = {
        use sea_orm::ActiveModelTrait;
        let p = entity::prelude::Property::find()
            .filter(entity::property::Column::TenantId.eq(nw))
            .one(&c.db)
            .await
            .unwrap()
            .expect("a seeded property");
        let mut am: entity::property::ActiveModel = p.into();
        am.city = sea_orm::Set("Portland, OR".into());
        am.state = sea_orm::Set("OR".into());
        am.units = sea_orm::Set(8);
        am.year_built = sea_orm::Set(2016);
        am.update(&c.db).await.unwrap()
    };
    let pid = maple.id;
    let pname = maple.name.clone();

    // The catalog, narrowed to what applies.
    let (st, items) = get_json(c, &format!("/mandates?property_id={pid}"), &staff).await;
    assert_eq!(st, Status::Ok, "{items}");
    let keys: Vec<&str> = items
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["key"].as_str().unwrap())
        .collect();
    assert!(keys.contains(&"smoke-co-test"), "{keys:?}");
    assert!(
        keys.contains(&"water-heater-straps"),
        "Oregon is seismic: {keys:?}"
    );
    assert!(
        keys.contains(&"fire-extinguishers"),
        "eight units: {keys:?}"
    );
    assert!(!keys.contains(&"lead-paint-visual"), "built 2016: {keys:?}");
    assert!(!keys.contains(&"repaint-cycle"), "not New York: {keys:?}");
    assert!(items
        .as_array()
        .unwrap()
        .iter()
        .all(|m| m["plan_id"].is_null()));
    let wanted = items
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["conditional"] == false)
        .count();

    // Add all: one routine per item, the conditional ones aside; again adds none.
    let (st, r) = send_json(
        c,
        Method::Post,
        &format!("/properties/{pid}/mandates"),
        &staff,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Ok, "{r}");
    assert_eq!(r["created"].as_u64().unwrap() as usize, wanted);
    let on = r["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| !m["plan_id"].is_null())
        .count();
    assert_eq!(on, wanted);
    let (_, r) = send_json(
        c,
        Method::Post,
        &format!("/properties/{pid}/mandates"),
        &staff,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(r["created"], 0);
    // A conditional item, by key.
    let (st, r) = send_json(
        c,
        Method::Post,
        &format!("/properties/{pid}/mandates"),
        &staff,
        serde_json::json!({ "keys": ["boiler-inspection"] }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{r}");
    assert_eq!(r["created"], 1);
    let (st, r) = send_json(
        c,
        Method::Post,
        &format!("/properties/{pid}/mandates"),
        &staff,
        serde_json::json!({ "keys": ["no-such-item"] }),
    )
    .await;
    assert_eq!(st, Status::BadRequest, "{r}");

    // The routines carry the key and a kit where the catalog names one.
    let (_, plans) = get_json(c, "/maintenance-plans", &staff).await;
    let smoke = plans
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["property_id"] == pid.to_string() && p["mandate_key"] == "smoke-co-test")
        .cloned()
        .expect("the alarm routine");
    let hvac = plans
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["property_id"] == pid.to_string() && p["mandate_key"] == "hvac-service")
        .cloned()
        .expect("the HVAC routine");
    assert!(
        !hvac["issue_template_id"].is_null(),
        "service-hvac kit: {hvac}"
    );

    // First due 30 days out, so nothing is "to schedule" yet with a 14-day lead.
    let today = chrono::Utc::now().date_naive();
    let (st, ts) = get_json(c, "/to-schedule", &staff).await;
    assert_eq!(st, Status::Ok, "{ts}");
    assert_eq!(ts["lead_days"], 14);
    let listed = |ts: &serde_json::Value, id: &str| {
        ts["plans"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["plan"]["id"] == id)
    };
    assert!(!listed(&ts, smoke["id"].as_str().unwrap()));
    // Move the alarm check to three days out: now it needs scheduling.
    let soon = (today + chrono::Duration::days(3)).to_string();
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/maintenance-plans/{}", smoke["id"].as_str().unwrap()),
        &staff,
        serde_json::json!({ "next_due_date": soon }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    let (_, ts) = get_json(c, "/to-schedule", &staff).await;
    let row = ts["plans"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["plan"]["id"] == smoke["id"])
        .cloned()
        .expect("the alarm routine is due soon");
    assert_eq!(row["days"], 3);
    assert_eq!(row["mandate"], true);
    assert_eq!(row["property_name"], pname);

    // A work order with no date and no visit shows too.
    let (st, tk) = send_json(
        c,
        Method::Post,
        &format!("/properties/{pid}/tickets"),
        &staff,
        serde_json::json!({ "title": "Hall light out", "category": "electrical", "priority": "low" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{tk}");
    let (_, ts) = get_json(c, "/to-schedule", &staff).await;
    assert!(ts["tickets"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["id"] == tk["id"]));

    // Open the routine early: a work order with its due date, then the plan
    // drops off the list and can't be opened twice while it's open.
    let (st, opened) = send_json(
        c,
        Method::Post,
        &format!(
            "/maintenance-plans/{}/run-now",
            smoke["id"].as_str().unwrap()
        ),
        &staff,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Ok, "{opened}");
    assert_eq!(
        opened["title"],
        "Test smoke and CO alarms, replace batteries"
    );
    assert_eq!(opened["due_date"], soon);
    let (_, ts) = get_json(c, "/to-schedule", &staff).await;
    assert!(!listed(&ts, smoke["id"].as_str().unwrap()));
    let (st, again) = send_json(
        c,
        Method::Post,
        &format!(
            "/maintenance-plans/{}/run-now",
            smoke["id"].as_str().unwrap()
        ),
        &staff,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Conflict, "{again}");

    // The portfolio rollup carries the undated work order.
    let (st, all) = get_json(c, "/attention", &staff).await;
    assert_eq!(st, Status::Ok, "{all}");
    let undated = all["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["kind"] == "unscheduled" && i["key"] == tk["id"])
        .cloned()
        .expect("the undated work order");
    assert_eq!(undated["property_name"], pname);
    assert_eq!(
        undated["href"],
        format!("/console/maintenance/{}", tk["id"].as_str().unwrap())
    );
    assert!(all["counts"]["unscheduled"].as_u64().unwrap() >= 1);

    // Reading needs maintenance:read; a stranger gets nothing.
    let nobody = mint(c, Some(nw), false, &["property:read"]);
    let (st, _) = get_json(c, "/to-schedule", &nobody).await;
    assert_eq!(st, Status::Forbidden);
}

/// Plan the day: a route proposed from the work due, with times from the
/// tasks and the supply run first; accepting books the visits, assigns the
/// work, settles parts and sets their need-by; the shopping list groups
/// what's left to buy by day and store.
async fn routes_and_shopping(c: &Ctx) {
    use rocket::http::Method;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    let nw = tenant_id(c, "northwind").await;
    let staff = mint(
        c,
        Some(nw),
        false,
        &["maintenance:read", "maintenance:manage", "property:read"],
    );
    let jordan = entity::prelude::User::find()
        .filter(entity::user::Column::Email.eq("jordan@northwind.com"))
        .one(&c.db)
        .await
        .unwrap()
        .expect("the seeded admin");
    let props = entity::prelude::Property::find()
        .filter(entity::property::Column::TenantId.eq(nw))
        .all(&c.db)
        .await
        .unwrap();
    let pid = props[0].id;
    let pid2 = props.get(1).map(|p| p.id).unwrap_or(pid);
    // A day well ahead of anything else in the suite.
    let day = (chrono::Utc::now() + chrono::Duration::days(40)).date_naive();

    // Two work orders due that day: one with tasks (90 minutes) and a part to
    // buy, one unassigned with no tasks (the default length).
    let (st, a) = send_json(
        c,
        Method::Post,
        &format!("/properties/{pid}/tickets"),
        &staff,
        serde_json::json!({
            "title": "Replace kitchen faucet",
            "category": "plumbing",
            "priority": "high",
            "due_date": day.to_string(),
            "assignee_user_id": jordan.id,
        }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{a}");
    let a_id = a["id"].as_str().unwrap().to_string();
    for (title, minutes) in [("Pull the old faucet", 30), ("Fit the new one", 60)] {
        let (st, r) = send_json(
            c,
            Method::Post,
            &format!("/tickets/{a_id}/tasks"),
            &staff,
            serde_json::json!({ "title": title, "trade": "plumbing", "est_minutes": minutes }),
        )
        .await;
        assert_eq!(st, Status::Ok, "{r}");
    }
    let (st, part) = send_json(
        c,
        Method::Post,
        &format!("/tickets/{a_id}/parts"),
        &staff,
        serde_json::json!({ "name": "Moen Adler faucet", "quantity": 1 }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{part}");
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/parts/{}", part["id"].as_str().unwrap()),
        &staff,
        serde_json::json!({ "url": "https://www.homedepot.com/p/Moen-Adler/1001" }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    let (st, b) = send_json(
        c,
        Method::Post,
        &format!("/properties/{pid2}/tickets"),
        &staff,
        serde_json::json!({ "title": "Patch hallway drywall", "category": "general", "due_date": day.to_string() }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{b}");
    let b_id = b["id"].as_str().unwrap().to_string();

    // Propose Jordan's day: the supply run first, the faucet job 90 minutes,
    // the unassigned drywall job offered too.
    let (st, route) = send_json(
        c,
        Method::Post,
        "/routes/propose",
        &staff,
        serde_json::json!({ "date": day.to_string(), "assignee_user_id": jordan.id, "start": "08:00" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{route}");
    let stops = route["stops"].as_array().unwrap();
    assert_eq!(stops[0]["kind"], "store", "{route}");
    // Overdue unassigned work from earlier scenarios is offered too, so the
    // day can fill; what doesn't fit is listed as unplaced.
    let all: Vec<serde_json::Value> = stops
        .iter()
        .chain(route["unplaced"].as_array().unwrap().iter())
        .cloned()
        .collect();
    let faucet = all
        .iter()
        .find(|s| s["ticket_id"] == a_id)
        .expect("the faucet job is on the route");
    assert_eq!(faucet["minutes"], 90);
    assert_eq!(faucet["to_buy"], 1);
    assert!(
        all.iter().any(|s| s["ticket_id"] == b_id),
        "unassigned work is offered: {route}"
    );
    let hd = route["stores"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["store"] == "Home Depot")
        .expect("a Home Depot run");
    assert!(hd["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["name"] == "Moen Adler faucet"));
    assert!(faucet["start"].as_str().unwrap() >= stops[0]["end"].as_str().unwrap());

    // Reading needs maintenance:read; booking needs manage.
    let reader = mint(c, Some(nw), false, &["maintenance:read"]);
    let (st, _) = send_json(
        c,
        Method::Post,
        "/routes/accept",
        &reader,
        serde_json::json!({ "date": day.to_string(), "assignee_user_id": jordan.id, "stops": [] }),
    )
    .await;
    assert_eq!(st, Status::Forbidden);

    // Accept: visits booked, the drywall job now Jordan's, parts settled.
    let accept_stops: Vec<serde_json::Value> = all
        .iter()
        .filter(|s| s["ticket_id"] == a_id || s["ticket_id"] == b_id)
        .map(|s| serde_json::json!({ "ticket_id": s["ticket_id"], "start": s["start"], "end": s["end"] }))
        .collect();
    assert_eq!(accept_stops.len(), 2);
    let (st, done) = send_json(
        c,
        Method::Post,
        "/routes/accept",
        &staff,
        serde_json::json!({ "date": day.to_string(), "assignee_user_id": jordan.id, "stops": accept_stops }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{done}");
    assert_eq!(
        done["booked"].as_u64().unwrap() as usize,
        accept_stops.len()
    );
    assert!(done["to_order"]
        .as_array()
        .unwrap()
        .iter()
        .any(|g| g["store"] == "Home Depot"));
    let (_, t) = get_json(c, &format!("/tickets/{b_id}"), &staff).await;
    assert_eq!(t["assignee_user_id"], jordan.id.to_string());
    assert_eq!(t["status"], "scheduled");
    let (_, t) = get_json(c, &format!("/tickets/{a_id}"), &staff).await;
    assert_eq!(t["status"], "scheduled");
    assert_eq!(t["due_date"], day.to_string());
    let (_, visits) = get_json(
        c,
        &format!("/appointments?from={day}&to={day}&assignee={}", jordan.id),
        &staff,
    )
    .await;
    let mine: Vec<_> = visits
        .as_array()
        .unwrap()
        .iter()
        .filter(|v| {
            v["status"] == "confirmed" && (v["subject_id"] == a_id || v["subject_id"] == b_id)
        })
        .collect();
    assert_eq!(mine.len(), 2, "{visits}");
    let (_, parts) = get_json(c, &format!("/tickets/{a_id}/parts"), &staff).await;
    let p = &parts.as_array().unwrap()[0];
    assert_eq!(p["status"], "to_order");
    assert_eq!(p["need_by"], (day - chrono::Duration::days(1)).to_string());

    // Accepting again keeps the visits that match instead of rebooking.
    let (st, again) = send_json(
        c,
        Method::Post,
        "/routes/accept",
        &staff,
        serde_json::json!({ "date": day.to_string(), "assignee_user_id": jordan.id, "stops": [{ "ticket_id": a_id, "start": faucet["start"], "end": faucet["end"] }] }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{again}");
    assert_eq!(again["kept"], 1);
    assert_eq!(again["booked"], 0);

    // The shopping list for that day: Home Depot, the faucet, on that day.
    let (st, shop) = get_json(c, &format!("/shopping?from={day}&to={day}"), &staff).await;
    assert_eq!(st, Status::Ok, "{shop}");
    let d = shop["days"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["day"] == day.to_string())
        .expect("that day on the list");
    let hd = d["stores"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["store"] == "Home Depot")
        .expect("Home Depot that day");
    assert!(hd["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["ticket_id"] == a_id));
    let (st, _) = get_json(c, "/shopping?from=2026-02-02&to=2026-01-01", &staff).await;
    assert_eq!(st, Status::BadRequest);
}

/// Follow-ups the scheduler sends on its own: the resident after finished
/// work (rate it, then still fixed?), a quiet vendor, an unanswered offer of
/// times, and a prospect who toured. Each once, each only while due.
async fn follow_ups_go_out(c: &Ctx) {
    use rocket::http::Method;
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter};
    let nw = tenant_id(c, "northwind").await;
    let staff = mint(
        c,
        Some(nw),
        false,
        &[
            "maintenance:read",
            "maintenance:manage",
            "entity:read",
            "entity:manage",
            "property:read",
            "application:read",
            "application:write",
        ],
    );
    let taylor = entity::prelude::User::find()
        .filter(entity::user::Column::Email.eq("taylor@example.com"))
        .one(&c.db)
        .await
        .unwrap()
        .expect("the seeded resident");
    let res =
        crate::auth::issue_access_token(&c.config, taylor.id, Some(nw), false, vec![]).unwrap();
    let now = chrono::Utc::now();
    let ago = |h: i64| -> sea_orm::prelude::DateTimeWithTimeZone {
        (now - chrono::Duration::hours(h)).into()
    };

    // Nothing is due yet: the first run sends nothing for these.
    let (st, t) = post_json(
        c,
        "/my/tickets",
        &res,
        serde_json::json!({ "title": "Porch light flickers", "category": "electrical" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{t}");
    let tid = t["id"].as_str().unwrap().to_string();
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/tickets/{tid}"),
        &staff,
        serde_json::json!({ "status": "resolved" }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    let _ = crate::followups::run(&c.db, nw).await.unwrap();
    let rating_key = format!("followup:rating:{tid}");
    assert!(!crate::notices::claimed(&c.db, nw, &rating_key)
        .await
        .unwrap());

    // Two days on: the rating ask goes out, once; the check-in waits for a week.
    {
        let t =
            entity::prelude::MaintenanceTicket::find_by_id(uuid::Uuid::parse_str(&tid).unwrap())
                .one(&c.db)
                .await
                .unwrap()
                .unwrap();
        let mut am: entity::maintenance_ticket::ActiveModel = t.into();
        am.resolved_at = sea_orm::Set(Some(ago(48)));
        am.update(&c.db).await.unwrap();
    }
    let out = crate::followups::run(&c.db, nw).await.unwrap();
    assert!(out["rating_requests"].as_u64().unwrap() >= 1, "{out}");
    assert!(crate::notices::claimed(&c.db, nw, &rating_key)
        .await
        .unwrap());
    assert!(
        !crate::notices::claimed(&c.db, nw, &format!("followup:checkin:{tid}"))
            .await
            .unwrap()
    );
    let sms = entity::prelude::BackgroundJob::find()
        .filter(entity::background_job::Column::TenantId.eq(nw))
        .filter(entity::background_job::Column::Kind.eq("auto_sms"))
        .all(&c.db)
        .await
        .unwrap()
        .into_iter()
        .filter(|j| {
            j.payload["template"] == "ticket_rating_request" && j.payload["owner_id"] == tid
        })
        .count();
    let email = entity::prelude::BackgroundJob::find()
        .filter(entity::background_job::Column::TenantId.eq(nw))
        .filter(entity::background_job::Column::Kind.eq("auto_email"))
        .all(&c.db)
        .await
        .unwrap()
        .into_iter()
        .any(|j| j.payload["template"] == "ticket_rating_request" && j.payload["owner_id"] == tid);
    assert!(sms >= 1 || email, "the resident was asked by text or email");
    let out2 = crate::followups::run(&c.db, nw).await.unwrap();
    let again = entity::prelude::BackgroundJob::find()
        .filter(entity::background_job::Column::TenantId.eq(nw))
        .filter(entity::background_job::Column::Kind.eq("auto_sms"))
        .all(&c.db)
        .await
        .unwrap()
        .into_iter()
        .filter(|j| {
            j.payload["template"] == "ticket_rating_request" && j.payload["owner_id"] == tid
        })
        .count();
    assert_eq!(again, sms, "not asked twice: {out2}");
    // Eight days on: the check-in.
    {
        let t =
            entity::prelude::MaintenanceTicket::find_by_id(uuid::Uuid::parse_str(&tid).unwrap())
                .one(&c.db)
                .await
                .unwrap()
                .unwrap();
        let mut am: entity::maintenance_ticket::ActiveModel = t.into();
        am.resolved_at = sea_orm::Set(Some(ago(24 * 8)));
        am.update(&c.db).await.unwrap();
    }
    let out = crate::followups::run(&c.db, nw).await.unwrap();
    assert!(out["checkins"].as_u64().unwrap() >= 1, "{out}");
    assert!(
        crate::notices::claimed(&c.db, nw, &format!("followup:checkin:{tid}"))
            .await
            .unwrap()
    );

    // A vendor sent two tasks by email three days ago with no answer: one
    // nudge for the batch, a fresh link, and a note on the work order.
    let (st, vendor) = post_json(
        c,
        "/entities",
        &staff,
        serde_json::json!({ "kind": "contractor", "name": "Quiet Electric", "contact_name": "Q",
            "email": "quiet@electric.example", "trades": ["electrical"] }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{vendor}");
    let vid = vendor["id"].as_str().unwrap().to_string();
    let (st, t2) = post_json(
        c,
        &format!("/properties/{}/tickets", t["property_id"].as_str().unwrap()),
        &staff,
        serde_json::json!({ "title": "Panel buzzes", "category": "electrical", "priority": "high" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{t2}");
    let tid2 = t2["id"].as_str().unwrap().to_string();
    let mut ids = vec![];
    for title in ["Open the panel", "Torque the breakers"] {
        let (_, tasks) = post_json(
            c,
            &format!("/tickets/{tid2}/tasks"),
            &staff,
            serde_json::json!({ "title": title, "trade": "electrical", "needs_contractor": true }),
        )
        .await;
        ids.push(
            tasks
                .as_array()
                .unwrap()
                .iter()
                .find(|x| x["title"] == title)
                .unwrap()["id"]
                .as_str()
                .unwrap()
                .to_string(),
        );
    }
    let (st, sent) = post_json(
        c,
        &format!("/tickets/{tid2}/dispatch-tasks"),
        &staff,
        serde_json::json!({ "task_ids": ids, "entity_id": vid }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{sent}");
    let before: Vec<entity::ticket_task::Model> = entity::prelude::TicketTask::find()
        .filter(entity::ticket_task::Column::TicketId.eq(uuid::Uuid::parse_str(&tid2).unwrap()))
        .all(&c.db)
        .await
        .unwrap();
    let old_hash = before[0]
        .vendor_token_hash
        .clone()
        .expect("a link was minted");
    for t in before {
        let mut am: entity::ticket_task::ActiveModel = t.into();
        am.dispatched_at = sea_orm::Set(Some(ago(72)));
        am.update(&c.db).await.unwrap();
    }
    let out = crate::followups::run(&c.db, nw).await.unwrap();
    assert_eq!(out["vendor_nudges"], 1, "{out}");
    let after: Vec<entity::ticket_task::Model> = entity::prelude::TicketTask::find()
        .filter(entity::ticket_task::Column::TicketId.eq(uuid::Uuid::parse_str(&tid2).unwrap()))
        .all(&c.db)
        .await
        .unwrap();
    assert!(after
        .iter()
        .all(|t| t.vendor_token_hash.as_deref() != Some(old_hash.as_str())));
    assert_eq!(
        after[0].vendor_token_hash, after[1].vendor_token_hash,
        "one link for the batch"
    );
    let nudge = entity::prelude::BackgroundJob::find()
        .filter(entity::background_job::Column::TenantId.eq(nw))
        .filter(entity::background_job::Column::Kind.eq("auto_email"))
        .all(&c.db)
        .await
        .unwrap()
        .into_iter()
        .find(|j| j.payload["template"] == "vendor_task_nudge")
        .expect("the vendor nudge email");
    assert_eq!(nudge.payload["to"], "quiet@electric.example");
    let link = nudge.payload["vars"]["vendor_link"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(link.contains("/vendor/"));
    let out = crate::followups::run(&c.db, nw).await.unwrap();
    assert_eq!(out["vendor_nudges"], 0, "once per batch");
    let (_, full) = get_json(c, &format!("/tickets/{tid2}"), &staff).await;
    assert!(
        full["comments"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["body"]
                .as_str()
                .unwrap_or("")
                .starts_with("Nudged Quiet Electric")),
        "{full}"
    );

    // Times offered two days ago for a visit next week, not picked: the link
    // goes again, once; an offer whose times have passed is left alone.
    let (st, t3) = post_json(
        c,
        "/my/tickets",
        &res,
        serde_json::json!({ "title": "Closet door off its track", "category": "general" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{t3}");
    let tid3 = t3["id"].as_str().unwrap().to_string();
    let next_week = (now + chrono::Duration::days(7)).to_rfc3339();
    let (st, offer) = post_json(
        c,
        "/appointments",
        &staff,
        serde_json::json!({ "ticket_id": tid3, "windows": [{ "start": next_week }] }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{offer}");
    let aid = uuid::Uuid::parse_str(offer["id"].as_str().unwrap()).unwrap();
    {
        let a = entity::prelude::Appointment::find_by_id(aid)
            .one(&c.db)
            .await
            .unwrap()
            .unwrap();
        let old = a.token_hash.clone();
        let mut am: entity::appointment::ActiveModel = a.into();
        am.created_at = sea_orm::Set(ago(48));
        am.update(&c.db).await.unwrap();
        let out = crate::followups::run(&c.db, nw).await.unwrap();
        assert_eq!(out["offer_reminders"], 1, "{out}");
        let a = entity::prelude::Appointment::find_by_id(aid)
            .one(&c.db)
            .await
            .unwrap()
            .unwrap();
        assert_ne!(a.token_hash, old, "a fresh link");
        assert_eq!(a.status, "proposed");
        let out = crate::followups::run(&c.db, nw).await.unwrap();
        assert_eq!(out["offer_reminders"], 0);
    }

    // A prospect who toured three days ago and hasn't applied hears once.
    let (st, lead) = post_json(
        c,
        "/leads",
        &staff,
        serde_json::json!({ "name": "Omar Haddad", "email": "omar@example.com", "phone": "503-555-0199", "source": "walk_in" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{lead}");
    let lid = uuid::Uuid::parse_str(lead["id"].as_str().unwrap()).unwrap();
    {
        let l = entity::prelude::Lead::find_by_id(lid)
            .one(&c.db)
            .await
            .unwrap()
            .unwrap();
        let mut am: entity::lead::ActiveModel = l.into();
        am.status = sea_orm::Set("toured".into());
        am.updated_at = sea_orm::Set(ago(72));
        am.update(&c.db).await.unwrap();
    }
    let out = crate::followups::run(&c.db, nw).await.unwrap();
    assert_eq!(out["prospect_nudges"], 1, "{out}");
    let nudge = entity::prelude::BackgroundJob::find()
        .filter(entity::background_job::Column::TenantId.eq(nw))
        .filter(entity::background_job::Column::Kind.eq("auto_email"))
        .all(&c.db)
        .await
        .unwrap()
        .into_iter()
        .find(|j| j.payload["template"] == "lead_after_showing")
        .expect("the prospect email");
    assert_eq!(nudge.payload["to"], "omar@example.com");
    assert!(nudge.payload["vars"]["link"]
        .as_str()
        .unwrap()
        .contains("/apply?tenant=northwind&lead="));
    let out = crate::followups::run(&c.db, nw).await.unwrap();
    assert_eq!(out["prospect_nudges"], 0, "once");

    // Turned off, nothing goes: a second finished work order with the hours at 0.
    crate::settings::set_value(
        &c.db,
        nw,
        crate::settings::FOLLOWUPS_RATING_HOURS,
        serde_json::json!(0),
    )
    .await
    .unwrap();
    let (st, t4) = post_json(
        c,
        "/my/tickets",
        &res,
        serde_json::json!({ "title": "Doorbell dead", "category": "electrical" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{t4}");
    let tid4 = t4["id"].as_str().unwrap().to_string();
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/tickets/{tid4}"),
        &staff,
        serde_json::json!({ "status": "resolved" }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    {
        let t =
            entity::prelude::MaintenanceTicket::find_by_id(uuid::Uuid::parse_str(&tid4).unwrap())
                .one(&c.db)
                .await
                .unwrap()
                .unwrap();
        let mut am: entity::maintenance_ticket::ActiveModel = t.into();
        am.resolved_at = sea_orm::Set(Some(ago(48)));
        am.update(&c.db).await.unwrap();
    }
    let _ = crate::followups::run(&c.db, nw).await.unwrap();
    assert!(
        !crate::notices::claimed(&c.db, nw, &format!("followup:rating:{tid4}"))
            .await
            .unwrap()
    );
    crate::settings::set_value(
        &c.db,
        nw,
        crate::settings::FOLLOWUPS_RATING_HOURS,
        serde_json::json!(24),
    )
    .await
    .unwrap();
}

/// Operations analytics and the portfolio map: three repairs on one water
/// heater that cost more than half its price flag it to replace and make a
/// repeat issue; the leasing funnel and the map answer; a reader without
/// report:read is refused.
async fn analytics_and_map(c: &Ctx) {
    use sea_orm::{ActiveModelTrait, EntityTrait};
    let nw = tenant_id(c, "northwind").await;
    let pid = property_ids(c, nw).await[0];
    let staff = mint(
        c,
        Some(nw),
        false,
        &[
            "maintenance:read",
            "maintenance:manage",
            "property:read",
            "report:read",
        ],
    );
    let (st, heater) = post_json(
        c,
        "/assets",
        &staff,
        serde_json::json!({ "property_id": pid, "kind": "plumbing", "name": "Analytics water heater", "purchase_price_cents": 100000 }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{heater}");
    let aid = uuid::Uuid::parse_str(heater["id"].as_str().unwrap()).unwrap();
    for (i, cost) in [20000i64, 20000, 15000].iter().enumerate() {
        let (st, t) = post_json(
            c,
            &format!("/properties/{pid}/tickets"),
            &staff,
            serde_json::json!({ "title": format!("Heater out again {i}"), "category": "water_heater_repeat" }),
        )
        .await;
        assert_eq!(st, Status::Ok, "{t}");
        let row = entity::prelude::MaintenanceTicket::find_by_id(
            uuid::Uuid::parse_str(t["id"].as_str().unwrap()).unwrap(),
        )
        .one(&c.db)
        .await
        .unwrap()
        .unwrap();
        let mut am: entity::maintenance_ticket::ActiveModel = row.into();
        am.cost_cents = sea_orm::Set(Some(*cost));
        am.asset_id = sea_orm::Set(Some(aid));
        am.update(&c.db).await.unwrap();
    }

    let (st, ops) = get_json(
        c,
        &format!("/analytics/operations?months=3&property_id={pid}"),
        &staff,
    )
    .await;
    assert_eq!(st, Status::Ok, "{ops}");
    assert_eq!(ops["months"].as_array().unwrap().len(), 3);
    assert_eq!(ops["properties"].as_array().unwrap().len(), 1);
    assert!(ops["totals"]["tickets"]["opened"].as_u64().unwrap() >= 3);
    let rep = ops["repeats"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["category"] == "water_heater_repeat")
        .expect("the repeat issue");
    assert_eq!(rep["count"], 3);
    assert_eq!(rep["spend_cents"], 55000);
    let a = ops["appliances"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["asset_id"] == aid.to_string())
        .expect("the heater");
    assert_eq!(a["replace"], true);
    assert_eq!(a["share_pct"], 55);
    assert_eq!(ops["replace_share_pct"], 50);
    let this_month = ops["months"].as_array().unwrap().last().unwrap();
    assert!(this_month["tickets"]["opened"].as_u64().unwrap() >= 3);
    let (st, _) = get_json(c, "/analytics/operations?property_id=nope", &staff).await;
    assert_eq!(st, Status::BadRequest);

    let (st, funnel) = get_json(c, "/analytics/leasing?months=12", &staff).await;
    assert_eq!(st, Status::Ok, "{funnel}");
    assert!(funnel["listings"].is_array());
    assert!(funnel["tours"].as_u64().is_some());

    let (st, map) = get_json(c, "/portfolio/map", &staff).await;
    assert_eq!(st, Status::Ok, "{map}");
    let pin = map["pins"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["property_id"] == pid.to_string())
        .expect("the property on the map");
    assert!(pin["open_tickets"].as_u64().unwrap() >= 3);

    let reader = mint(c, Some(nw), false, &["property:read"]);
    let (st, _) = get_json(c, "/analytics/operations", &reader).await;
    assert_eq!(st, Status::Forbidden);
}

/// Texts, round two: a prospect's number names the conversation; a missed
/// call is filed and texted back once per window (not after STOP, not when
/// turned off); staff link an unknown number to a vendor or to nobody.
async fn texts_round_two(c: &Ctx) {
    use rocket::http::Method;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    let nw = tenant_id(c, "northwind").await;
    let staff = mint(
        c,
        Some(nw),
        false,
        &[
            "message:read",
            "message:manage",
            "application:read",
            "application:write",
            "entity:read",
            "entity:manage",
        ],
    );
    // A prospect texts: the conversation is theirs.
    let (st, lead) = post_json(
        c,
        "/leads",
        &staff,
        serde_json::json!({ "name": "Nadia Okafor", "email": "nadia@example.com", "phone": "(503) 555-0177", "source": "website" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{lead}");
    let (st, t) = post_json(
        c,
        "/texts/simulate",
        &staff,
        serde_json::json!({ "phone": "503-555-0177", "body": "Is the 2 bed still available?" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{t}");
    assert_eq!(t["thread"]["display_name"], "Nadia Okafor", "{t}");
    assert_eq!(t["thread"]["lead_id"], lead["id"]);

    // A stranger calls and nobody answers: filed, texted back, once.
    let (st, call) = post_json(
        c,
        "/texts/simulate-call",
        &staff,
        serde_json::json!({ "phone": "503-555-0188" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{call}");
    assert_eq!(call["texted_back"], true, "{call}");
    let msgs = call["thread"]["messages"].as_array().unwrap();
    assert!(msgs
        .iter()
        .any(|m| m["body"] == "Missed call" && m["direction"] == "in"));
    let back = msgs
        .iter()
        .find(|m| m["direction"] == "out")
        .expect("the text-back is filed");
    assert!(
        back["body"].as_str().unwrap().contains("Northwind"),
        "{back}"
    );
    let tid = call["thread"]["thread"]["id"].as_str().unwrap().to_string();
    let queued = entity::prelude::BackgroundJob::find()
        .filter(entity::background_job::Column::TenantId.eq(nw))
        .filter(entity::background_job::Column::Kind.eq("auto_sms"))
        .all(&c.db)
        .await
        .unwrap()
        .into_iter()
        .any(|j| j.payload["trigger"] == "missed_call" && j.payload["to"] == "+15035550188");
    assert!(queued, "the text-back is queued");
    let (_, again) = post_json(
        c,
        "/texts/simulate-call",
        &staff,
        serde_json::json!({ "phone": "503-555-0188" }),
    )
    .await;
    assert_eq!(again["texted_back"], false, "once per window");
    assert!(again["thread"]["thread"]["unread_count"].as_i64().unwrap() >= 2);

    // Someone who texted STOP isn't texted back.
    let (_, _) = post_json(
        c,
        "/texts/simulate",
        &staff,
        serde_json::json!({ "phone": "503-555-0199", "body": "STOP" }),
    )
    .await;
    let (_, stopped) = post_json(
        c,
        "/texts/simulate-call",
        &staff,
        serde_json::json!({ "phone": "503-555-0199" }),
    )
    .await;
    assert_eq!(stopped["texted_back"], false);

    // Turned off: filed, no text.
    crate::settings::set_value(
        &c.db,
        nw,
        crate::settings::TEXTS_MISSED_CALL_REPLY_ON,
        serde_json::json!(false),
    )
    .await
    .unwrap();
    let (_, off) = post_json(
        c,
        "/texts/simulate-call",
        &staff,
        serde_json::json!({ "phone": "503-555-0166" }),
    )
    .await;
    assert_eq!(off["texted_back"], false);
    assert!(off["thread"]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["body"] == "Missed call"));
    crate::settings::set_value(
        &c.db,
        nw,
        crate::settings::TEXTS_MISSED_CALL_REPLY_ON,
        serde_json::json!(true),
    )
    .await
    .unwrap();

    // Staff say whose number it is.
    let (st, vendor) = post_json(
        c,
        "/entities",
        &staff,
        serde_json::json!({ "kind": "contractor", "name": "Ruiz Roofing", "email": "ruiz@roof.example" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{vendor}");
    let (st, linked) = send_json(
        c,
        Method::Patch,
        &format!("/texts/{tid}"),
        &staff,
        serde_json::json!({ "link": { "kind": "vendor", "id": vendor["id"] } }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{linked}");
    assert_eq!(linked["counterparty_id"], vendor["id"]);
    assert_eq!(linked["display_name"], "Ruiz Roofing");
    let (st, renamed) = send_json(
        c,
        Method::Patch,
        &format!("/texts/{tid}"),
        &staff,
        serde_json::json!({ "link": { "kind": "none" }, "display_name": "Unknown roofer" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{renamed}");
    assert!(renamed["counterparty_id"].is_null());
    assert_eq!(renamed["display_name"], "Unknown roofer");
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/texts/{tid}"),
        &staff,
        serde_json::json!({ "link": { "kind": "landlord", "id": vendor["id"] } }),
    )
    .await;
    assert_eq!(st, Status::BadRequest);
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/texts/{tid}"),
        &staff,
        serde_json::json!({ "link": { "kind": "resident", "id": uuid::Uuid::new_v4() } }),
    )
    .await;
    assert_eq!(st, Status::NotFound);
}

/// The go-live page: every provider listed with its needs, a vault key
/// counting as present, backups and drills recorded by the scripts showing up,
/// and only integrations managers allowed in.
async fn go_live_page(c: &Ctx) {
    use sea_orm::{ActiveModelTrait, Set};
    let nw = tenant_id(c, "northwind").await;
    let admin = mint(c, Some(nw), false, &["integrations:manage"]);
    crate::secrets::store(&c.db, Some(nw), "checkr.api_key", "test-key-1234", None)
        .await
        .unwrap();
    let (st, page) = get_json(c, "/go-live", &admin).await;
    assert_eq!(st, Status::Ok, "{page}");
    let providers = page["providers"].as_array().unwrap();
    assert_eq!(providers.len(), crate::routes::go_live::PROVIDERS.len());
    let checkr = providers.iter().find(|p| p["key"] == "checkr").unwrap();
    assert_eq!(checkr["readiness"], "simulated");
    assert!(checkr["requirements"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["label"] == "checkr.api_key in the vault" && r["present"] == true));
    assert!(checkr["requirements"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["label"]
            .as_str()
            .unwrap()
            .starts_with("webhook.checkr.secret")
            && r["present"] == false));
    let backup = |ok: bool, hours: i64| entity::backup_run::ActiveModel {
        id: Set(uuid::Uuid::new_v4()),
        kind: Set(if hours < 0 {
            "restore_drill".into()
        } else {
            "backup".into()
        }),
        started_at: Set((chrono::Utc::now() - chrono::Duration::hours(hours.abs())).into()),
        finished_at: Set(Some(chrono::Utc::now().into())),
        ok: Set(ok),
        bytes: Set(Some(123_456)),
        location: Set(Some("/var/backups/acre/acre-test.dump.age".into())),
        detail: Set(None),
        created_at: Set(chrono::Utc::now().into()),
    };
    backup(true, 5).insert(&c.db).await.unwrap();
    backup(false, 1).insert(&c.db).await.unwrap();
    backup(true, -2).insert(&c.db).await.unwrap();
    let (_, page) = get_json(c, "/go-live", &admin).await;
    assert_eq!(page["last_backup"]["ok"], false, "the newest run failed");
    assert!(page["last_good_backup_at"].is_string());
    let check = |k: &str| {
        page["platform"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["key"] == k)
            .unwrap()["ok"]
            .clone()
    };
    assert_eq!(check("backup"), true);
    assert_eq!(check("drill"), true);
    assert_eq!(check("production"), false);
    let reader = mint(c, Some(nw), false, &["property:read"]);
    let (st, _) = get_json(c, "/go-live", &reader).await;
    assert_eq!(st, Status::Forbidden);
}

/// Spanish for residents: staff set a resident's language on the lease, the
/// resident sets their own from the portal, an applicant picks it on the
/// public form, and messages to each come out in Spanish (English for anyone
/// else, and for messages with no Spanish version).
async fn spanish_messages(c: &Ctx) {
    use rocket::http::Method;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
    let nw = tenant_id(c, "northwind").await;
    let staff = mint(c, Some(nw), false, &["lease:read", "lease:manage"]);
    let leases = entity::prelude::Lease::find()
        .filter(entity::lease::Column::TenantId.eq(nw))
        .filter(entity::lease::Column::TenantEmail.is_not_null())
        .order_by_asc(entity::lease::Column::CreatedAt)
        .all(&c.db)
        .await
        .unwrap();
    let (es_lease, en_lease) = (&leases[0], &leases[1]);
    let (st, set) = send_json(
        c,
        Method::Put,
        &format!("/leases/{}/language", es_lease.id),
        &staff,
        serde_json::json!({ "language": "es" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{set}");
    assert_eq!(set["language"], "es");
    let (_, got) = get_json(c, &format!("/leases/{}/language", es_lease.id), &staff).await;
    assert_eq!(got["language"], "es");
    let (st, _) = send_json(
        c,
        Method::Put,
        &format!("/leases/{}/language", es_lease.id),
        &staff,
        serde_json::json!({ "language": "fr" }),
    )
    .await;
    assert_eq!(st, Status::BadRequest);
    let reader = mint(c, Some(nw), false, &["lease:read"]);
    let (st, _) = send_json(
        c,
        Method::Put,
        &format!("/leases/{}/language", es_lease.id),
        &reader,
        serde_json::json!({ "language": "es" }),
    )
    .await;
    assert_eq!(st, Status::Forbidden);

    // Render a real rent reminder to each.
    async fn rendered(
        c: &Ctx,
        tenant: uuid::Uuid,
        to: &str,
        template: &str,
    ) -> entity::notification::Model {
        let job_id = crate::scheduler::enqueue(
            &c.db,
            tenant,
            "auto_email",
            serde_json::json!({
                "template": template,
                "to": to,
                "vars": { "amount": "$1,850", "due_date": "2026-11-01", "pay_url": "https://x/pay" },
                "trigger": format!("es-test:{}", uuid::Uuid::new_v4()),
            }),
            0,
        )
        .await
        .unwrap();
        let job = entity::prelude::BackgroundJob::find_by_id(job_id)
            .one(&c.db)
            .await
            .unwrap()
            .unwrap();
        let _ = crate::notify::handle_job(&c.db, &job).await;
        entity::prelude::Notification::find()
            .filter(entity::notification::Column::BackgroundJobId.eq(job_id))
            .one(&c.db)
            .await
            .unwrap()
            .expect("rendered and filed")
    }
    let es = rendered(c, nw, es_lease.tenant_email.as_deref().unwrap(), "rent_due").await;
    assert!(
        es.subject
            .as_deref()
            .unwrap()
            .starts_with("El alquiler de $1,850 vence"),
        "{:?}",
        es.subject
    );
    assert!(es.body.as_deref().unwrap().contains("Pague en línea"));
    let en = rendered(c, nw, en_lease.tenant_email.as_deref().unwrap(), "rent_due").await;
    assert!(
        en.subject
            .as_deref()
            .unwrap()
            .starts_with("Rent of $1,850 is due"),
        "{:?}",
        en.subject
    );
    // No Spanish version of a staff message: English, even to a Spanish reader.
    let staffish = rendered(
        c,
        nw,
        es_lease.tenant_email.as_deref().unwrap(),
        "test_notification",
    )
    .await;
    assert!(!staffish.body.as_deref().unwrap_or("").contains("Hola"));

    // The resident chooses for themselves.
    let taylor = entity::prelude::User::find()
        .filter(entity::user::Column::Email.eq("taylor@example.com"))
        .one(&c.db)
        .await
        .unwrap()
        .unwrap();
    let res =
        crate::auth::issue_access_token(&c.config, taylor.id, Some(nw), false, vec![]).unwrap();
    let (st, mine) = send_json(
        c,
        Method::Put,
        "/my/language",
        &res,
        serde_json::json!({ "language": "es" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{mine}");
    assert!(mine["addresses"].as_u64().unwrap() >= 1);
    let (_, mine) = get_json(c, "/my/language", &res).await;
    assert_eq!(mine["language"], "es");
    let t = rendered(c, nw, "taylor@example.com", "payment_receipt").await;
    assert!(
        t.subject.as_deref().unwrap().starts_with("Pago recibido"),
        "{:?}",
        t.subject
    );
    let (_, _) = send_json(
        c,
        Method::Put,
        "/my/language",
        &res,
        serde_json::json!({ "language": "en" }),
    )
    .await;
    let t = rendered(c, nw, "taylor@example.com", "payment_receipt").await;
    assert!(t
        .subject
        .as_deref()
        .unwrap()
        .starts_with("Payment received"));

    // An applicant picks Spanish on the public form.
    assert_eq!(
        crate::language::for_contact(&c.db, nw, "lucia@example.com").await,
        "en"
    );
    let resp = c
        .client
        .post("/public/applications?tenant=northwind")
        .header(rocket::http::ContentType::JSON)
        .body(
            serde_json::json!({
                "applicant_name": "Lucía Romero",
                "email": "Lucia@Example.com",
                "phone": "503-555-0144",
                "screening_consent": true,
                "language": "es-MX",
            })
            .to_string(),
        )
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok, "{:?}", resp.into_string().await);
    assert_eq!(
        crate::language::for_contact(&c.db, nw, "lucia@example.com").await,
        "es"
    );
    assert_eq!(
        crate::language::for_contact(&c.db, nw, "+15035550144").await,
        "es"
    );
}

/// House onboarding: a new property's checklist starts empty and fills in
/// from the data; the record's year built and the rent estimate are proposed
/// and applied only when chosen.
async fn onboarding_checklist(c: &Ctx) {
    use rocket::http::Method;
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
    let nw = tenant_id(c, "northwind").await;
    let staff = mint(c, Some(nw), false, &["property:read", "property:write"]);
    let (st, prop) = send_json(
        c,
        Method::Post,
        "/properties",
        &staff,
        serde_json::json!({ "name": "Checklist Cottage", "address": "9 Fern Ln", "city": "Portland", "state": "OR", "postal_code": "97202", "units": 1, "occupied_units": 0, "monthly_rent_cents": 0, "property_type": "single_family" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{prop}");
    let pid = prop["id"].as_str().unwrap().to_string();
    let (st, list) = get_json(c, &format!("/properties/{pid}/checklist"), &staff).await;
    assert_eq!(st, Status::Ok, "{list}");
    assert_eq!(list["steps"].as_array().unwrap().len(), 11);
    assert_eq!(list["next"], "address");
    assert_eq!(list["required"], 10);
    let step = |l: &serde_json::Value, k: &str| -> serde_json::Value {
        l["steps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["key"] == k)
            .unwrap()
            .clone()
    };
    assert_eq!(step(&list, "owner")["done"], false);

    // The record arrives with a year built; a rent estimate is on file.
    let puid = uuid::Uuid::parse_str(&pid).unwrap();
    let now = chrono::Utc::now();
    let existing = entity::prelude::PropertyDetail::find()
        .filter(entity::property_detail::Column::PropertyId.eq(puid))
        .one(&c.db)
        .await
        .unwrap();
    match existing {
        Some(d) => {
            let mut am: entity::property_detail::ActiveModel = d.into();
            am.year_built = Set(Some(1948));
            am.latitude = Set(Some(45.48));
            am.longitude = Set(Some(-122.65));
            am.last_enriched_at = Set(Some(now.into()));
            am.update(&c.db).await.unwrap();
        }
        None => {
            entity::property_detail::ActiveModel {
                property_id: Set(puid),
                tenant_id: Set(nw),
                year_built: Set(Some(1948)),
                latitude: Set(Some(45.48)),
                longitude: Set(Some(-122.65)),
                last_enriched_at: Set(Some(now.into())),
                features: Set(serde_json::json!([])),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
                ..Default::default()
            }
            .insert(&c.db)
            .await
            .unwrap();
        }
    }
    entity::property_valuation::ActiveModel {
        id: Set(uuid::Uuid::new_v4()),
        tenant_id: Set(nw),
        property_id: Set(puid),
        as_of: Set(now.date_naive().to_string()),
        estimated_value_cents: Set(Some(52_000_000)),
        value_low_cents: Set(None),
        value_high_cents: Set(None),
        estimated_rent_cents: Set(Some(214_960)),
        confidence: Set(Some(80)),
        source: Set("test".into()),
        created_at: Set(now.into()),
    }
    .insert(&c.db)
    .await
    .unwrap();
    let (_, auto) = get_json(c, &format!("/properties/{pid}/autofill"), &staff).await;
    let fields: Vec<&str> = auto["proposals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["field"].as_str().unwrap())
        .collect();
    assert!(fields.contains(&"year_built"), "{auto}");
    let has_unit = entity::prelude::Unit::find()
        .filter(entity::unit::Column::PropertyId.eq(puid))
        .all(&c.db)
        .await
        .unwrap()
        .len()
        == 1;
    if has_unit {
        assert!(fields.contains(&"unit_market_rent"), "{auto}");
    }
    let (_, list) = get_json(c, &format!("/properties/{pid}/checklist"), &staff).await;
    assert_eq!(step(&list, "address")["done"], true);
    assert_eq!(
        step(&list, "record")["done"],
        false,
        "suggestions to review"
    );
    let (st, applied) = post_json(
        c,
        &format!("/properties/{pid}/autofill/apply"),
        &staff,
        serde_json::json!({ "fields": fields }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{applied}");
    assert!(
        applied["proposals"].as_array().unwrap().is_empty(),
        "{applied}"
    );
    let p = entity::prelude::Property::find_by_id(puid)
        .one(&c.db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(p.year_built, 1948);
    let (_, list) = get_json(c, &format!("/properties/{pid}/checklist"), &staff).await;
    assert_eq!(step(&list, "record")["done"], true, "{list}");
    if has_unit {
        assert_eq!(step(&list, "rent")["done"], true, "{list}");
    }
    let reader = mint(c, Some(nw), false, &["lease:read"]);
    let (st, _) = get_json(c, &format!("/properties/{pid}/checklist"), &reader).await;
    assert_eq!(st, Status::Forbidden);
}

/// The vendor portal: staff invite a vendor; signed in, the vendor sees the
/// job sent to them and accepts it there; another vendor's job stays hidden;
/// the login opens nothing in the console.
async fn vendor_portal_flow(c: &Ctx) {
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    let nw = tenant_id(c, "northwind").await;
    let pid = property_ids(c, nw).await[0];
    let staff = mint(
        c,
        Some(nw),
        false,
        &[
            "maintenance:read",
            "maintenance:manage",
            "entity:read",
            "entity:manage",
            "member:manage",
            "property:read",
        ],
    );
    let make_vendor = |name: &'static str, email: &'static str| {
        let staff = staff.clone();
        async move {
            let (st, v) = post_json(
                c,
                "/entities",
                &staff,
                serde_json::json!({ "kind": "contractor", "name": name, "email": email, "trades": ["plumbing"] }),
            )
            .await;
            assert_eq!(st, Status::Ok, "{v}");
            v["id"].as_str().unwrap().to_string()
        }
    };
    let pipes = make_vendor("Portal Pipes", "portal.pipes@example.com").await;
    let other = make_vendor("Other Drains", "other.drains@example.com").await;
    let send = |vendor: String, title: &'static str| {
        let staff = staff.clone();
        async move {
            let (_, t) = post_json(
                c,
                &format!("/properties/{pid}/tickets"),
                &staff,
                serde_json::json!({ "title": title, "category": "plumbing" }),
            )
            .await;
            let tid = t["id"].as_str().unwrap().to_string();
            let (_, tasks) = post_json(
                c,
                &format!("/tickets/{tid}/tasks"),
                &staff,
                serde_json::json!({ "title": "Clear the line", "trade": "plumbing", "needs_contractor": true }),
            )
            .await;
            let task = tasks.as_array().unwrap().last().unwrap()["id"]
                .as_str()
                .unwrap()
                .to_string();
            let (st, sent) = post_json(
                c,
                &format!("/tickets/{tid}/dispatch-tasks"),
                &staff,
                serde_json::json!({ "task_ids": [task], "entity_id": vendor }),
            )
            .await;
            assert_eq!(st, Status::Ok, "{sent}");
            tid
        }
    };
    let mine_tid = send(pipes.clone(), "Portal: main line backs up").await;
    let theirs_tid = send(other.clone(), "Portal: other vendor's job").await;

    let (st, inv) = post_json(
        c,
        &format!("/entities/{pipes}/portal-invite"),
        &staff,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Ok, "{inv}");
    assert_eq!(inv["outcome"], "invited");
    let uid = uuid::Uuid::parse_str(inv["user_id"].as_str().unwrap()).unwrap();
    let (st, again) = post_json(
        c,
        &format!("/entities/{pipes}/portal-invite"),
        &staff,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Ok, "{again}");
    let tok = crate::auth::issue_access_token(&c.config, uid, Some(nw), false, vec![]).unwrap();

    let (st, me) = get_json(c, "/vendor-portal/me", &tok).await;
    assert_eq!(st, Status::Ok, "{me}");
    assert_eq!(me["vendor"], "Portal Pipes");
    assert!(me["open_jobs"].as_u64().unwrap() >= 1);
    let (_, jobs) = get_json(c, "/vendor-portal/jobs", &tok).await;
    let job = jobs["open"]
        .as_array()
        .unwrap()
        .iter()
        .find(|j| j["ticket_id"] == mine_tid)
        .expect("my job is listed")
        .clone();
    assert!(jobs["open"]
        .as_array()
        .unwrap()
        .iter()
        .all(|j| j["ticket_id"] != theirs_tid));
    let batch = job["batch"].as_str().unwrap().to_string();
    let (st, detail) = get_json(c, &format!("/vendor-portal/jobs/{batch}"), &tok).await;
    assert_eq!(st, Status::Ok, "{detail}");
    assert_eq!(detail["title"], "Portal: main line backs up");
    let (st, acc) = post_json(
        c,
        &format!("/vendor-portal/jobs/{batch}/accept"),
        &tok,
        serde_json::json!({ "note": "Tomorrow morning" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{acc}");
    assert_eq!(acc["response"], "accepted");

    // Someone else's job, by its batch: not found.
    let other_task = entity::prelude::TicketTask::find()
        .filter(
            entity::ticket_task::Column::TicketId.eq(uuid::Uuid::parse_str(&theirs_tid).unwrap()),
        )
        .one(&c.db)
        .await
        .unwrap()
        .unwrap();
    let other_batch = other_task.vendor_token_hash.unwrap();
    let (st, _) = get_json(c, &format!("/vendor-portal/jobs/{other_batch}"), &tok).await;
    assert_eq!(st, Status::NotFound);
    let (st, _) = post_json(
        c,
        &format!("/vendor-portal/jobs/{other_batch}/accept"),
        &tok,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::NotFound);

    // The vendor login opens nothing in the console; staff aren't vendors.
    let (st, _) = get_json(c, "/properties", &tok).await;
    assert_eq!(st, Status::Forbidden);
    let (st, _) = get_json(c, "/vendor-portal/me", &staff).await;
    assert_eq!(st, Status::Forbidden);
}

/// Owner approvals: work over the owner's limit can't go to a vendor until
/// the owner says yes from their link; finished billable work asks them to
/// sign off; the owner portal shows their holdings, asks and statement.
async fn owner_approvals_flow(c: &Ctx) {
    use rocket::http::Method;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    let nw = tenant_id(c, "northwind").await;
    let staff = mint(
        c,
        Some(nw),
        false,
        &[
            "maintenance:read",
            "maintenance:manage",
            "entity:read",
            "entity:manage",
            "member:manage",
            "property:read",
            "tenant:manage",
        ],
    );
    // Dana owns an LLC with properties; give her an email and a low limit.
    let dana = entity::prelude::Owner::find()
        .filter(entity::owner::Column::TenantId.eq(nw))
        .filter(entity::owner::Column::Name.eq("Dana Kessler"))
        .one(&c.db)
        .await
        .unwrap()
        .expect("the seeded owner");
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/crm/owners/{}", dana.id),
        &staff,
        serde_json::json!({ "name": "Dana Kessler", "email": "dana.owner@example.com", "phone": "503-555-0177" }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    let (st, lim) = send_json(
        c,
        Method::Patch,
        &format!("/crm/owners/{}/approvals", dana.id),
        &staff,
        serde_json::json!({ "approval_limit_cents": 100 }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{lim}");
    // Make Dana the majority holder of one LLC, so its properties are hers.
    let stake = entity::prelude::EntityOwnership::find()
        .filter(entity::entity_ownership::Column::OwnerId.eq(dana.id))
        .one(&c.db)
        .await
        .unwrap()
        .unwrap();
    {
        use sea_orm::ActiveModelTrait;
        let mut am: entity::entity_ownership::ActiveModel = stake.clone().into();
        am.ownership_bps = sea_orm::Set(9_900);
        am.update(&c.db).await.unwrap();
    }
    let pid = entity::prelude::Property::find()
        .filter(entity::property::Column::LlcId.eq(stake.entity_id))
        .one(&c.db)
        .await
        .unwrap()
        .expect("a property in Dana's LLC")
        .id;

    // A work order from a kit: the estimate is over her limit.
    let (_, kits) = get_json(c, "/issue-templates", &staff).await;
    let kit = kits
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["name"] == "Replace dishwasher")
        .unwrap()
        .clone();
    let (st, g) = post_json(
        c,
        &format!("/issue-templates/{}/generate", kit["id"].as_str().unwrap()),
        &staff,
        serde_json::json!({ "property_id": pid }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{g}");
    let tid = g["ticket"]["id"].as_str().unwrap().to_string();
    let (_, ap) = get_json(c, &format!("/tickets/{tid}/approvals"), &staff).await;
    assert_eq!(ap["owner_name"], "Dana Kessler");
    assert_eq!(ap["limit_cents"], 100);
    assert_eq!(ap["needs_approval"], true);

    // Sending a task to a vendor is refused and asks Dana.
    let (_, vendors) = get_json(c, &format!("/tickets/{tid}/vendors"), &staff).await;
    let vid = vendors
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["email"].is_string())
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let (_, tasks) = get_json(c, &format!("/tickets/{tid}/tasks"), &staff).await;
    let task_id = tasks[0]["id"].as_str().unwrap().to_string();
    let (st, refused) = post_json(
        c,
        &format!("/tickets/{tid}/dispatch-tasks"),
        &staff,
        serde_json::json!({ "task_ids": [task_id], "entity_id": vid }),
    )
    .await;
    assert_eq!(st, Status::Conflict, "{refused}");
    assert!(
        refused["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Ask them to approve"),
        "{refused}"
    );
    // Staff ask her from the work order.
    let (st, asked_now) = post_json(
        c,
        &format!("/tickets/{tid}/approvals"),
        &staff,
        serde_json::json!({ "note": "Old unit is leaking; this is the quote" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{asked_now}");
    assert_eq!(asked_now["status"], "pending");
    let (_, detail) = get_json(c, &format!("/tickets/{tid}"), &staff).await;
    assert_eq!(detail["status"], "on_hold");
    assert_eq!(detail["waiting_on"], "owner");
    let (_, ap) = get_json(c, &format!("/tickets/{tid}/approvals"), &staff).await;
    assert_eq!(ap["approvals"][0]["status"], "pending");
    assert_eq!(ap["approvals"][0]["kind"], "approval");
    let asked = entity::prelude::BackgroundJob::find()
        .filter(entity::background_job::Column::TenantId.eq(nw))
        .all(&c.db)
        .await
        .unwrap()
        .into_iter()
        .filter(|j| j.payload["template"] == "owner_approval_request")
        .collect::<Vec<_>>();
    assert!(asked.len() >= 2, "email and text to Dana");
    let link = asked[0].payload["vars"]["link"]
        .as_str()
        .unwrap()
        .to_string();
    let token = link.rsplit('/').next().unwrap().to_string();
    // Asking again while pending is refused; so is a second dispatch.
    let (st, _) = post_json(
        c,
        &format!("/tickets/{tid}/approvals"),
        &staff,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Conflict);

    // Dana opens the link, sees the plan, approves.
    let resp = c
        .client
        .get(format!("/public/approve/{token}"))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    let view: serde_json::Value = resp.into_json().await.unwrap();
    assert_eq!(view["kind"], "approval");
    assert_eq!(view["owner_name"], "Dana Kessler");
    assert!(view["tasks"].as_array().unwrap().len() >= 3);
    assert_eq!(view["limit_label"], "$1");
    let resp = c
        .client
        .post(format!("/public/approve/{token}"))
        .header(ContentType::JSON)
        .body(serde_json::json!({ "approve": true, "note": "Go ahead, use the Bosch" }).to_string())
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    let decided: serde_json::Value = resp.into_json().await.unwrap();
    assert_eq!(decided["status"], "approved");
    let (_, detail) = get_json(c, &format!("/tickets/{tid}"), &staff).await;
    assert_eq!(detail["status"], "open");
    assert!(detail["waiting_on"].is_null());
    assert!(detail["comments"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["action"] == "owner_approved" && m["author_name"] == "Dana Kessler"));
    // Now the dispatch goes through.
    let (st, sent) = post_json(
        c,
        &format!("/tickets/{tid}/dispatch-tasks"),
        &staff,
        serde_json::json!({ "task_ids": [task_id], "entity_id": vid }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{sent}");
    // A second answer on the same ask is refused.
    let resp = c
        .client
        .post(format!("/public/approve/{token}"))
        .header(ContentType::JSON)
        .body(serde_json::json!({ "approve": false }).to_string())
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Conflict);

    // Done with a cost on it: Dana is asked to sign off.
    let (st, _) = post_json(
        c,
        &format!("/tickets/{tid}/expenses"),
        &staff,
        serde_json::json!({ "description": "Dishwasher", "amount_cents": 61900, "billable_to_owner": true }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/tickets/{tid}"),
        &staff,
        serde_json::json!({ "status": "resolved" }),
    )
    .await;
    assert_eq!(st, Status::Ok);
    let (_, ap) = get_json(c, &format!("/tickets/{tid}/approvals"), &staff).await;
    let signoff = ap["approvals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["kind"] == "signoff")
        .expect("a sign-off ask")
        .clone();
    assert_eq!(signoff["status"], "pending");
    assert_eq!(signoff["amount_cents"], 61900);

    // Dana gets a login and answers from the portal.
    let (st, inv) = post_json(
        c,
        &format!("/crm/owners/{}/invite", dana.id),
        &staff,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, Status::Ok, "{inv}");
    assert_eq!(inv["outcome"], "invited");
    let uid = Uuid::parse_str(inv["user_id"].as_str().unwrap()).unwrap();
    let owner_tok =
        crate::auth::issue_access_token(&c.config, uid, Some(nw), false, vec![]).unwrap();
    let (st, home) = get_json(c, "/my/owner", &owner_tok).await;
    assert_eq!(st, Status::Ok, "{home}");
    assert_eq!(home["name"], "Dana Kessler");
    assert!(home["properties"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["id"] == pid.to_string()));
    assert_eq!(home["pending"][0]["kind"], "signoff");
    assert_eq!(home["approval_limit_cents"], 100);
    let sid = home["pending"][0]["id"].as_str().unwrap().to_string();
    let (st, done) = post_json(
        c,
        &format!("/my/owner/approvals/{sid}"),
        &owner_tok,
        serde_json::json!({ "approve": true, "note": "Looks great" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{done}");
    assert_eq!(done["status"], "approved");
    let (_, detail) = get_json(c, &format!("/tickets/{tid}"), &staff).await;
    assert_eq!(detail["status"], "closed");
    let (st, allw) = get_json(c, "/my/owner/work?all=true", &owner_tok).await;
    assert_eq!(st, Status::Ok);
    assert!(allw
        .as_array()
        .unwrap()
        .iter()
        .any(|w| w["id"] == tid.as_str() && w["approval"]["kind"] == "signoff"));
    // The statement for this month lists the work and the asks.
    let month = chrono::Utc::now().format("%Y-%m").to_string();
    let (st, stmt) = get_json(c, &format!("/my/owner/statement?month={month}"), &owner_tok).await;
    assert_eq!(st, Status::Ok, "{stmt}");
    assert!(stmt["work"]
        .as_array()
        .unwrap()
        .iter()
        .any(|w| w["ticket_id"] == tid.as_str()));
    assert!(stmt["approvals"].as_array().unwrap().len() >= 2);
    let resp = c
        .client
        .get(format!("/my/owner/statement.pdf?month={month}"))
        .header(bearer(&owner_tok))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    assert!(resp.into_bytes().await.unwrap().starts_with(b"%PDF"));
    // Someone who isn't an owner gets nothing here.
    let (st, _) = get_json(c, "/my/owner", &staff).await;
    assert_eq!(st, Status::NotFound);

    // Staff can go ahead over the limit with a reason, which is recorded.
    let (st, g2) = post_json(
        c,
        &format!("/issue-templates/{}/generate", kit["id"].as_str().unwrap()),
        &staff,
        serde_json::json!({ "property_id": pid }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{g2}");
    let tid2 = g2["ticket"]["id"].as_str().unwrap().to_string();
    let (_, tasks2) = get_json(c, &format!("/tickets/{tid2}/tasks"), &staff).await;
    let (st, sent) = post_json(
        c,
        &format!("/tickets/{tid2}/dispatch-tasks"),
        &staff,
        serde_json::json!({ "task_ids": [tasks2[0]["id"]], "entity_id": vid, "approval_override_reason": "Water pouring into the unit below" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{sent}");
    let (_, ap2) = get_json(c, &format!("/tickets/{tid2}/approvals"), &staff).await;
    assert_eq!(ap2["approvals"][0]["status"], "overridden");
    assert_eq!(ap2["needs_approval"], false);
    // Nudges: nothing is due yet (asked just now).
    let n = crate::owner_approvals::nudge_pending(&c.db, nw)
        .await
        .unwrap();
    assert_eq!(n, 0);
}

async fn appointments_flow(c: &Ctx) {
    use rocket::http::Method;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    let nw = tenant_id(c, "northwind").await;
    let staff = mint(
        c,
        Some(nw),
        false,
        &["maintenance:read", "maintenance:manage", "property:read"],
    );
    let taylor = entity::prelude::User::find()
        .filter(entity::user::Column::Email.eq("taylor@example.com"))
        .one(&c.db)
        .await
        .unwrap()
        .expect("the seeded resident");
    let res =
        crate::auth::issue_access_token(&c.config, taylor.id, Some(nw), false, vec![]).unwrap();

    // Taylor reports something; staff offer two windows.
    let (st, t) = post_json(
        c,
        "/my/tickets",
        &res,
        serde_json::json!({ "title": "Bathroom fan rattles", "category": "electrical" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{t}");
    let tid = t["id"].as_str().unwrap().to_string();
    let day = |n: i64, h: u32| {
        (chrono::Utc::now() + chrono::Duration::days(n))
            .date_naive()
            .and_hms_opt(h, 0, 0)
            .unwrap()
            .and_utc()
            .to_rfc3339()
    };
    let (st, e) = post_json(
        c,
        "/appointments",
        &staff,
        serde_json::json!({ "ticket_id": tid, "windows": [] }),
    )
    .await;
    assert_eq!(st, Status::BadRequest, "{e}");
    let (st, a) = post_json(
        c,
        "/appointments",
        &staff,
        serde_json::json!({ "ticket_id": tid, "windows": [
            { "start": day(2, 16) },
            { "start": day(3, 17), "end": day(3, 19) }
        ] }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{a}");
    assert_eq!(a["status"], "proposed");
    assert_eq!(a["with_email"], "taylor@example.com");
    assert_eq!(a["kind"], "repair");
    assert_eq!(a["windows"].as_array().unwrap().len(), 2);
    assert!(a["link"].as_str().unwrap().contains("/book/"));
    let aid = a["id"].as_str().unwrap().to_string();
    // The two-hour default filled the first window's end.
    let w0 = &a["windows"][0];
    let s0 = chrono::DateTime::parse_from_rfc3339(w0["start"].as_str().unwrap()).unwrap();
    let e0 = chrono::DateTime::parse_from_rfc3339(w0["end"].as_str().unwrap()).unwrap();
    assert_eq!((e0 - s0).num_minutes(), 120);
    // Taylor got the offer by email and text.
    let offers = entity::prelude::BackgroundJob::find()
        .filter(entity::background_job::Column::TenantId.eq(nw))
        .all(&c.db)
        .await
        .unwrap()
        .into_iter()
        .filter(|j| j.payload["template"] == "appointment_offered")
        .count();
    assert!(offers >= 1, "an offer went out");

    // In the portal, Taylor sees it on the request and picks the second window.
    let (st, mine) = get_json(c, "/my/appointments", &res).await;
    assert_eq!(st, Status::Ok, "{mine}");
    assert!(mine
        .as_array()
        .unwrap()
        .iter()
        .any(|x| x["id"] == aid.as_str()));
    let (st, picked) = post_json(
        c,
        &format!("/my/appointments/{aid}/pick"),
        &res,
        serde_json::json!({ "window": 1 }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{picked}");
    assert_eq!(picked["status"], "confirmed");
    assert_eq!(picked["confirmed_by"], "resident");
    assert!(picked["when_words"].as_str().unwrap().contains(" to "));
    let (_, detail) = get_json(c, &format!("/tickets/{tid}"), &staff).await;
    assert_eq!(detail["status"], "scheduled");
    assert!(detail["due_date"].is_string());
    assert!(detail["comments"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["action"] == "appointment_confirmed"
            && m["visibility"] == "public"
            && m["body"].as_str().unwrap().starts_with("Scheduled: ")));
    let (st, _) = post_json(
        c,
        &format!("/my/appointments/{aid}/pick"),
        &res,
        serde_json::json!({ "window": 9 }),
    )
    .await;
    assert_eq!(st, Status::BadRequest);

    // It shows on the calendar for that day, and the person going is on it.
    let (_, cal) = get_json(c, "/appointments?status=confirmed", &staff).await;
    assert!(cal
        .as_array()
        .unwrap()
        .iter()
        .any(|x| x["id"] == aid.as_str()));

    // Reminders: two leads reached at once send once and are remembered.
    let starts = chrono::DateTime::parse_from_rfc3339(picked["starts_at"].as_str().unwrap())
        .unwrap()
        .to_utc();
    let before = entity::prelude::BackgroundJob::find()
        .filter(entity::background_job::Column::TenantId.eq(nw))
        .all(&c.db)
        .await
        .unwrap()
        .into_iter()
        .filter(|j| j.payload["template"] == "appointment_reminder")
        .count();
    let n = crate::appointments::remind_due(&c.db, nw, starts - chrono::Duration::minutes(90))
        .await
        .unwrap();
    assert_eq!(n, 1, "one appointment reminded");
    let n2 = crate::appointments::remind_due(&c.db, nw, starts - chrono::Duration::minutes(60))
        .await
        .unwrap();
    assert_eq!(n2, 0, "not twice");
    let after = entity::prelude::BackgroundJob::find()
        .filter(entity::background_job::Column::TenantId.eq(nw))
        .all(&c.db)
        .await
        .unwrap()
        .into_iter()
        .filter(|j| j.payload["template"] == "appointment_reminder")
        .count();
    assert!(after > before, "a reminder went out");

    // Staff can mark the visit done; a second offer on the same work order
    // replaces the first, and the public link works without signing in.
    let (st, done) = send_json(
        c,
        Method::Patch,
        &format!("/appointments/{aid}"),
        &staff,
        serde_json::json!({ "status": "done" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{done}");
    assert_eq!(done["status"], "done");
    let (st, a2) = post_json(
        c,
        "/appointments",
        &staff,
        serde_json::json!({ "ticket_id": tid, "windows": [{ "start": day(5, 16) }], "note": "Return visit for the new motor" }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{a2}");
    let link = a2["link"].as_str().unwrap().to_string();
    let token = link.rsplit('/').next().unwrap().to_string();
    let aid2 = a2["id"].as_str().unwrap().to_string();
    let resp = c
        .client
        .get(format!("/public/book/{token}"))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    let view: serde_json::Value = resp.into_json().await.unwrap();
    assert_eq!(view["title"], "Bathroom fan rattles");
    assert_eq!(view["status"], "proposed");
    assert!(view["company"].as_str().unwrap().contains("Northwind"));
    let resp = c.client.get("/public/book/nope").dispatch().await;
    assert_eq!(resp.status(), Status::NotFound);
    // None of those work; Taylor asks for another time. Staff hear.
    let resp = c
        .client
        .post(format!("/public/book/{token}/decline"))
        .header(ContentType::JSON)
        .body(serde_json::json!({ "propose": { "start": day(6, 18) }, "reason": "Working late that day" }).to_string())
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    let declined: serde_json::Value = resp.into_json().await.unwrap();
    assert_eq!(declined["status"], "declined");
    let (_, got) = get_json(c, &format!("/appointments/{aid2}"), &staff).await;
    assert!(got["proposed_words"].is_string(), "{got}");
    assert_eq!(got["outcome_note"], "Working late that day");
    let heard = entity::prelude::Notification::find()
        .filter(entity::notification::Column::TenantId.eq(nw))
        .all(&c.db)
        .await
        .unwrap()
        .into_iter()
        .any(|n| n.template_key == "appointment_declined");
    assert!(heard, "staff were told");
    // A declined link can't be picked again.
    let resp = c
        .client
        .post(format!("/public/book/{token}"))
        .header(ContentType::JSON)
        .body(serde_json::json!({ "window": 0 }).to_string())
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Conflict);
    // Staff confirm the asked-for time by phone; marked done needs confirmed.
    let (st, _) = send_json(
        c,
        Method::Patch,
        &format!("/appointments/{aid2}"),
        &staff,
        serde_json::json!({ "status": "no_show" }),
    )
    .await;
    assert_eq!(st, Status::Conflict);
    let (st, e) = post_json(
        c,
        "/appointments",
        &staff,
        serde_json::json!({ "ticket_id": tid, "windows": [{ "start": day(6, 18) }] }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{e}");
    let aid3 = e["id"].as_str().unwrap().to_string();
    let (st, fixed) = send_json(
        c,
        Method::Patch,
        &format!("/appointments/{aid3}"),
        &staff,
        serde_json::json!({ "confirm": { "start": day(6, 18) } }),
    )
    .await;
    assert_eq!(st, Status::Ok, "{fixed}");
    assert_eq!(fixed["confirmed_by"], "staff");
    // Someone outside the property's reach doesn't see it.
    let other_tenant = tenant_id(c, "cascade").await;
    let outsider = mint(
        c,
        Some(other_tenant),
        false,
        &["maintenance:read", "maintenance:manage"],
    );
    let (st, _) = get_json(c, &format!("/appointments/{aid3}"), &outsider).await;
    assert_eq!(st, Status::NotFound);
}
