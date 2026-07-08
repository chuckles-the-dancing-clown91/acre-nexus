//! **SaaS billing — platform plane** (roadmap Phase 8). Acre HQ's billing
//! console: an overview of every workspace's plan, usage, and outstanding
//! balance; the full invoice ledger; and the operations that generate and
//! settle invoices. All gated by `platform:admin` (cross-tenant, staff-only),
//! exactly like the rest of `/platform/*`. Pricing lives in [`crate::saas`]; the
//! workspace-facing half in [`crate::routes::billing`].

use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::routes::billing::InvoiceDto;
use crate::saas;
use chrono::Utc;
use entity::prelude::{PlatformInvoice, PlatformInvoiceLine, Tenant};
use rocket::serde::json::Json;
use rocket::{get, patch, post};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

// ---- Overview ------------------------------------------------------------

#[derive(Serialize, schemars::JsonSchema)]
pub struct TenantBilling {
    pub tenant_id: Uuid,
    pub name: String,
    pub slug: String,
    pub plan: String,
    pub status: String,
    pub units: i32,
    /// Recurring charge at current usage (this period's estimate).
    pub mrr_cents: i64,
    pub mrr_label: String,
    /// Sum of unpaid (open) invoices.
    pub outstanding_cents: i64,
    pub outstanding_label: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct BillingOverview {
    pub tenant_count: i64,
    /// Sum of every workspace's current recurring charge — platform MRR.
    pub mrr_cents: i64,
    pub mrr_label: String,
    pub outstanding_cents: i64,
    pub outstanding_label: String,
    pub tenants: Vec<TenantBilling>,
}

/// `GET /platform/billing/overview` — MRR + per-workspace billing snapshot.
#[rocket_okapi::openapi(tag = "Platform Admin")]
#[get("/platform/billing/overview")]
pub async fn overview(
    db: crate::db::RequestDb,
    user: AuthUser,
) -> ApiResult<Json<BillingOverview>> {
    user.require(Permission::PlatformAdmin)?;
    let tenants = Tenant::find()
        .order_by_asc(entity::tenant::Column::Name)
        .all(&db)
        .await?;

    let mut rows = Vec::new();
    let mut mrr = 0i64;
    let mut outstanding_total = 0i64;
    for t in tenants {
        let plan = saas::plan_for(&t.plan);
        let metered = saas::meter(&db, t.id).await?;
        let assembled = saas::assemble(plan, metered.units);
        let outstanding: i64 = PlatformInvoice::find()
            .filter(entity::platform_invoice::Column::TenantId.eq(t.id))
            .filter(entity::platform_invoice::Column::Status.eq("open"))
            .all(&db)
            .await?
            .iter()
            .map(|i| i.total_cents)
            .sum();
        mrr += assembled.total_cents;
        outstanding_total += outstanding;
        rows.push(TenantBilling {
            tenant_id: t.id,
            name: t.name,
            slug: t.slug,
            plan: plan.key.into(),
            status: t.status,
            units: metered.units,
            mrr_cents: assembled.total_cents,
            mrr_label: usd(assembled.total_cents),
            outstanding_cents: outstanding,
            outstanding_label: usd(outstanding),
        });
    }

    Ok(Json(BillingOverview {
        tenant_count: rows.len() as i64,
        mrr_cents: mrr,
        mrr_label: usd(mrr),
        outstanding_cents: outstanding_total,
        outstanding_label: usd(outstanding_total),
        tenants: rows,
    }))
}

// ---- Invoice ledger ------------------------------------------------------

/// `GET /platform/billing/invoices?<status>&<period>` — every invoice across
/// tenants, newest first, optionally filtered by status / period.
#[rocket_okapi::openapi(tag = "Platform Admin")]
#[get("/platform/billing/invoices?<status>&<period>")]
pub async fn invoices(
    db: crate::db::RequestDb,
    user: AuthUser,
    status: Option<String>,
    period: Option<String>,
) -> ApiResult<Json<Vec<InvoiceDto>>> {
    user.require(Permission::PlatformAdmin)?;
    let mut query = PlatformInvoice::find()
        .order_by_desc(entity::platform_invoice::Column::Period)
        .order_by_asc(entity::platform_invoice::Column::TenantId);
    if let Some(s) = status.filter(|s| !s.is_empty()) {
        query = query.filter(entity::platform_invoice::Column::Status.eq(s));
    }
    if let Some(p) = period.filter(|p| !p.is_empty()) {
        query = query.filter(entity::platform_invoice::Column::Period.eq(p));
    }
    let invoices = query.all(&db).await?;

    let mut out = Vec::with_capacity(invoices.len());
    for inv in invoices {
        let lines = PlatformInvoiceLine::find()
            .filter(entity::platform_invoice_line::Column::InvoiceId.eq(inv.id))
            .order_by_asc(entity::platform_invoice_line::Column::SortOrder)
            .all(&db)
            .await?;
        out.push(InvoiceDto::from(inv, lines));
    }
    Ok(Json(out))
}

// ---- Billing run ---------------------------------------------------------

#[derive(Deserialize, schemars::JsonSchema)]
pub struct RunReq {
    /// `YYYY-MM` period to bill; defaults to the previous month.
    pub period: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct RunResp {
    pub period: String,
    pub generated: i64,
}

/// `POST /platform/billing/run` — generate invoices for a period across every
/// tenant. Idempotent per `(tenant, period)`.
#[rocket_okapi::openapi(tag = "Platform Admin")]
#[post("/platform/billing/run", data = "<body>")]
pub async fn run(
    state: &rocket::State<crate::state::AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    body: Json<RunReq>,
) -> ApiResult<Json<RunResp>> {
    user.require(Permission::PlatformAdmin)?;
    let period = body
        .into_inner()
        .period
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| saas::previous_month(Utc::now().date_naive()));
    if !valid_period(&period) {
        return Err(ApiError::BadRequest("period must be YYYY-MM".into()));
    }

    // Generation runs on the platform connection (cross-tenant, null GUC).
    let generated = saas::run_for_period(&state.db, &period).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::PLATFORM_BILLING_RUN,
        Some("platform_invoice"),
        None,
        None,
        Some(json!({ "period": period, "generated": generated })),
    )
    .await;

    Ok(Json(RunResp { period, generated }))
}

fn valid_period(p: &str) -> bool {
    let parts: Vec<&str> = p.split('-').collect();
    matches!(parts[..], [y, m]
        if y.len() == 4 && y.parse::<i32>().is_ok()
        && m.len() == 2 && (1..=12).contains(&m.parse::<u32>().unwrap_or(0)))
}

// ---- Settlement ----------------------------------------------------------

async fn load_dto(db: &crate::db::RequestDb, id: Uuid) -> ApiResult<InvoiceDto> {
    let inv = PlatformInvoice::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("invoice".into()))?;
    let lines = PlatformInvoiceLine::find()
        .filter(entity::platform_invoice_line::Column::InvoiceId.eq(id))
        .order_by_asc(entity::platform_invoice_line::Column::SortOrder)
        .all(db)
        .await?;
    Ok(InvoiceDto::from(inv, lines))
}

fn parse_id(id: &str) -> ApiResult<Uuid> {
    Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid invoice id".into()))
}

/// `POST /platform/billing/invoices/<id>/pay` — mark an open invoice paid.
#[rocket_okapi::openapi(tag = "Platform Admin")]
#[post("/platform/billing/invoices/<id>/pay")]
pub async fn mark_paid(
    db: crate::db::RequestDb,
    user: AuthUser,
    id: &str,
) -> ApiResult<Json<InvoiceDto>> {
    user.require(Permission::PlatformAdmin)?;
    let id = parse_id(id)?;
    let inv = PlatformInvoice::find_by_id(id)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("invoice".into()))?;
    if inv.status == "void" {
        return Err(ApiError::BadRequest("cannot pay a voided invoice".into()));
    }
    let tenant_id = inv.tenant_id;
    let now = Utc::now();
    let mut am: entity::platform_invoice::ActiveModel = inv.into();
    am.status = Set("paid".into());
    am.paid_at = Set(Some(now.into()));
    am.updated_at = Set(now.into());
    am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::PLATFORM_INVOICE_PAID,
        Some("platform_invoice"),
        Some(id.to_string()),
        Some(tenant_id),
        None,
    )
    .await;
    Ok(Json(load_dto(&db, id).await?))
}

/// `POST /platform/billing/invoices/<id>/void` — void an invoice (write-off).
#[rocket_okapi::openapi(tag = "Platform Admin")]
#[post("/platform/billing/invoices/<id>/void")]
pub async fn void(
    db: crate::db::RequestDb,
    user: AuthUser,
    id: &str,
) -> ApiResult<Json<InvoiceDto>> {
    user.require(Permission::PlatformAdmin)?;
    let id = parse_id(id)?;
    let inv = PlatformInvoice::find_by_id(id)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("invoice".into()))?;
    let tenant_id = inv.tenant_id;
    let now = Utc::now();
    let mut am: entity::platform_invoice::ActiveModel = inv.into();
    am.status = Set("void".into());
    am.updated_at = Set(now.into());
    am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::PLATFORM_INVOICE_VOID,
        Some("platform_invoice"),
        Some(id.to_string()),
        Some(tenant_id),
        None,
    )
    .await;
    Ok(Json(load_dto(&db, id).await?))
}

// ---- Plan change ---------------------------------------------------------

#[derive(Deserialize, schemars::JsonSchema)]
pub struct PlanReq {
    pub plan: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct PlanResp {
    pub tenant_id: Uuid,
    pub plan: String,
}

/// `PATCH /platform/billing/tenants/<id>/plan` — move a workspace to a plan.
#[rocket_okapi::openapi(tag = "Platform Admin")]
#[patch("/platform/billing/tenants/<id>/plan", data = "<body>")]
pub async fn set_plan(
    db: crate::db::RequestDb,
    user: AuthUser,
    id: &str,
    body: Json<PlanReq>,
) -> ApiResult<Json<PlanResp>> {
    user.require(Permission::PlatformAdmin)?;
    let tenant_id =
        Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid tenant id".into()))?;
    let plan_key = body.into_inner().plan;
    if !saas::PLANS.iter().any(|p| p.key == plan_key) {
        return Err(ApiError::BadRequest(format!("unknown plan '{plan_key}'")));
    }
    let tenant = Tenant::find_by_id(tenant_id)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("tenant".into()))?;
    let previous = tenant.plan.clone();
    let mut am: entity::tenant::ActiveModel = tenant.into();
    am.plan = Set(plan_key.clone());
    am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::TENANT_PLAN_CHANGE,
        Some("tenant"),
        Some(tenant_id.to_string()),
        Some(tenant_id),
        Some(json!({ "from": previous, "to": plan_key })),
    )
    .await;
    Ok(Json(PlanResp {
        tenant_id,
        plan: plan_key,
    }))
}

#[cfg(test)]
mod tests {
    use super::valid_period;

    #[test]
    fn period_validation() {
        assert!(valid_period("2026-06"));
        assert!(valid_period("2025-12"));
        assert!(!valid_period("2026-13"));
        assert!(!valid_period("2026-00"));
        assert!(!valid_period("26-6"));
        assert!(!valid_period("2026/06"));
        assert!(!valid_period("nonsense"));
    }
}
