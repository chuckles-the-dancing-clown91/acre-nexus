//! A **platform (SaaS) invoice** — Acre HQ billing a client workspace for its
//! subscription (roadmap Phase 8). Distinct from the resident-facing rent
//! billing in [`crate::lease_payment`]: this is the *platform's* revenue, one
//! invoice per tenant per billing month, priced from the tenant's plan plus a
//! metered per-unit-under-management overage. Its line items live in
//! [`crate::platform_invoice_line`].
//!
//! Tenant-owned (RLS on `tenant_id`), but authored on the platform plane: Acre
//! staff (null tenant GUC) generate them across every workspace, while a
//! workspace sees only its own via `billing:read`.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "platform_invoice")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// The billing month this invoice covers, `YYYY-MM`. Unique per tenant, so
    /// generation is idempotent.
    pub period: String,
    /// Plan key at the time of billing (`starter` | `growth` | `enterprise`).
    pub plan: String,
    /// Units under management metered for this period.
    pub unit_count: i32,
    /// Units included in the plan's base fee (the overage threshold).
    pub included_units: i32,
    /// Plan base fee.
    pub base_cents: i64,
    /// Metered overage charge (units beyond `included_units`).
    pub overage_cents: i64,
    /// `base_cents + overage_cents` (no tax modelled yet).
    pub total_cents: i64,
    /// `draft` | `open` | `paid` | `void`.
    pub status: String,
    /// When the invoice was issued to the workspace (left `draft` until then).
    pub issued_at: Option<DateTimeWithTimeZone>,
    /// Payment due date, `YYYY-MM-DD` (net-15 from issue).
    pub due_date: Option<String>,
    pub paid_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
