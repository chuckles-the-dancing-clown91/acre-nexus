//! Per-request database access that activates Postgres **row-level security**.
//!
//! The app already filters every tenant query by `tenant_id` (the primary wall).
//! This module adds the DB-level second wall: each request runs its queries inside
//! a transaction that sets `app.tenant_id` (via `SET LOCAL`), so the RLS policies
//! on tenant-owned tables enforce isolation even if a handler forgets its filter.
//!
//! Why a transaction: with a connection pool, `SET LOCAL` is the only way to bind a
//! GUC to exactly the connection running a request's queries. [`RequestDb`] lazily
//! opens one transaction per request, sets the tenant, and implements
//! [`ConnectionTrait`] so handlers use `&db` exactly like the old `&state.db`. The
//! [`TxCommit`] fairing commits it on a 2xx response and rolls back otherwise.
//!
//! The tenant is resolved best-effort (JWT `tid` → `X-Tenant` → `Host`). When it is
//! `None` (platform staff at Acre HQ, login, background paths) the GUC is left
//! unset and the RLS policies' "no tenant context" branch allows all rows — i.e.
//! the platform plane is the intentional cross-tenant path, exactly as before.
//! (A custom GUC set by `SET LOCAL` reverts to `''`, not `NULL`, after its
//! transaction, so a reused pooled connection reports `''` here; the policy
//! treats `''` and `NULL` alike — see migration `000038` — so the platform plane
//! stays correct regardless of connection reuse.)

use crate::state::AppState;
use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};
use rocket::Response;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, DbErr, ExecResult,
    QueryResult, Statement, TransactionTrait, Value,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

struct TxState {
    conn: DatabaseConnection,
    tenant: Option<Uuid>,
    txn: Option<DatabaseTransaction>,
}

impl TxState {
    /// Open the request transaction on first use and pin `app.tenant_id` to it.
    async fn ensure(&mut self) -> Result<&DatabaseTransaction, DbErr> {
        if self.txn.is_none() {
            let txn = self.conn.begin().await?;
            if let Some(t) = self.tenant {
                // `set_config(_, _, is_local=true)` == `SET LOCAL` — scoped to this txn.
                txn.execute(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    "SELECT set_config('app.tenant_id', $1, true)",
                    [Value::from(t.to_string())],
                ))
                .await?;
            }
            self.txn = Some(txn);
        }
        Ok(self.txn.as_ref().unwrap())
    }
}

/// A request-scoped database handle. Implements [`ConnectionTrait`], so it is a
/// drop-in for `&state.db` in handlers, but every query runs under the request's
/// RLS tenant context.
#[derive(Clone)]
pub struct RequestDb {
    inner: Arc<Mutex<TxState>>,
}

impl RequestDb {
    fn new(conn: DatabaseConnection, tenant: Option<Uuid>) -> Self {
        RequestDb {
            inner: Arc::new(Mutex::new(TxState {
                conn,
                tenant,
                txn: None,
            })),
        }
    }

    /// Commit (on success) or roll back the request transaction, if one was opened.
    async fn finish(&self, commit: bool) {
        let mut g = self.inner.lock().await;
        if let Some(txn) = g.txn.take() {
            let r = if commit {
                txn.commit().await
            } else {
                txn.rollback().await
            };
            if let Err(e) = r {
                tracing::error!(
                    "request tx {} failed: {e}",
                    if commit { "commit" } else { "rollback" }
                );
            }
        }
    }
}

#[async_trait::async_trait]
impl ConnectionTrait for RequestDb {
    fn get_database_backend(&self) -> DbBackend {
        DbBackend::Postgres
    }
    async fn execute(&self, stmt: Statement) -> Result<ExecResult, DbErr> {
        let mut g = self.inner.lock().await;
        g.ensure().await?.execute(stmt).await
    }
    async fn execute_unprepared(&self, sql: &str) -> Result<ExecResult, DbErr> {
        let mut g = self.inner.lock().await;
        g.ensure().await?.execute_unprepared(sql).await
    }
    async fn query_one(&self, stmt: Statement) -> Result<Option<QueryResult>, DbErr> {
        let mut g = self.inner.lock().await;
        g.ensure().await?.query_one(stmt).await
    }
    async fn query_all(&self, stmt: Statement) -> Result<Vec<QueryResult>, DbErr> {
        let mut g = self.inner.lock().await;
        g.ensure().await?.query_all(stmt).await
    }
}

/// Resolve the tenant this request acts within: JWT `tid`, else `X-Tenant`, else
/// the inbound `Host`. `None` means the platform/cross-tenant plane.
async fn resolve_request_tenant(req: &Request<'_>, state: &AppState) -> Option<Uuid> {
    if let Some(tok) = req
        .headers()
        .get_one("Authorization")
        .and_then(|h| h.strip_prefix("Bearer "))
    {
        if let Some(claims) = crate::auth::decode_access_token(&state.config, tok) {
            return claims.tid;
        }
    }
    if let Some(x) = req.headers().get_one("X-Tenant") {
        if let Some(id) = crate::tenancy::helpers::resolve_tenant_ref(state, x).await {
            return Some(id);
        }
    }
    if let Some(host) = req.headers().get_one("Host") {
        if let Some(r) = crate::tenancy::helpers::resolve_host(state, host).await {
            return Some(r.tenant_id);
        }
    }
    None
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for RequestDb {
    type Error = ();
    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let state = match req.rocket().state::<AppState>() {
            Some(s) => s,
            None => return Outcome::Error((Status::InternalServerError, ())),
        };
        let tenant = resolve_request_tenant(req, state).await;
        let conn = state.db.clone();
        let db = req
            .local_cache_async(async move { RequestDb::new(conn, tenant) })
            .await;
        Outcome::Success(db.clone())
    }
}

/// Commits (2xx) or rolls back the per-request RLS transaction after each response.
pub struct TxCommit;

#[rocket::async_trait]
impl Fairing for TxCommit {
    fn info(&self) -> Info {
        Info {
            name: "RLS request-transaction commit",
            kind: Kind::Response,
        }
    }

    async fn on_response<'r>(&self, req: &'r Request<'_>, res: &mut Response<'r>) {
        let Some(state) = req.rocket().state::<AppState>() else {
            return;
        };
        // Retrieve the request's RequestDb if the guard created one; the fallback
        // (no txn) makes this a no-op for requests that never touched the database.
        let conn = state.db.clone();
        let db = req
            .local_cache_async(async move { RequestDb::new(conn, None) })
            .await;
        db.finish(res.status().code < 400).await;
    }
}
