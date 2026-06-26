//! An **audit log** entry records an action against the platform — who did what,
//! to which target, in which workspace, and when.
//!
//! Two complementary kinds of entry share this table:
//! * **Domain events** — rich, semantic records of state changes (e.g.
//!   `pii.reveal`, `role.update`, `property.create`) written explicitly by
//!   handlers via the `audit::record` writer, carrying structured `metadata`.
//! * **Request events** — one row per HTTP request (reads included), written by
//!   the `AuditFairing`. These populate the request-context columns below
//!   (method, path, status, latency, principal kind, correlation id).
//!
//! Together they give a complete, queryable trail the Acre dashboard surfaces
//! and that can be shipped to an external audit sink.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "audit_log")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    /// The user who performed the action (`NULL` for system / anonymous actions).
    pub actor_user_id: Option<Uuid>,
    /// Dotted action key, e.g. `pii.reveal`, `role.update`, `http.request`.
    pub action: String,
    /// What was acted on, e.g. `user`, `role`, `property`.
    pub target_type: Option<String>,
    /// Identifier of the target (stringified id).
    pub target_id: Option<String>,
    /// Workspace context, if any.
    pub tenant_id: Option<Uuid>,
    /// Free-form structured detail.
    pub metadata: Option<Json>,
    // ---- Request context (populated for per-request entries) ----
    /// HTTP method, e.g. `GET`, `POST` (request entries only).
    pub method: Option<String>,
    /// Request path or matched route template, e.g. `/properties/<id>`.
    pub path: Option<String>,
    /// HTTP status code of the response.
    pub status_code: Option<i32>,
    /// Per-request correlation id, echoed in the `X-Request-Id` response header.
    pub request_id: Option<Uuid>,
    /// Client IP address, when resolvable.
    pub ip: Option<String>,
    /// Wall-clock handling time in milliseconds.
    pub duration_ms: Option<i64>,
    /// Kind of principal behind the request: `user`, `api_token`, `public`,
    /// or `system`.
    pub principal_kind: Option<String>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
