use super::PlanDto;
use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::saas;
use crate::tenancy::TenantScope;
use entity::prelude::{PlatformInvoice, Tenant};
use rocket::get;
use rocket::serde::json::Json;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::Serialize;

/// A priced-out line for the current-period estimate.
#[derive(Serialize, schemars::JsonSchema)]
pub struct EstimateLine {
    pub description: String,
    pub quantity: i32,
    pub amount_cents: i64,
    pub amount_label: String,
}

/// The estimated charge for the period currently in progress at present usage.
#[derive(Serialize, schemars::JsonSchema)]
pub struct Estimate {
    pub unit_count: i32,
    pub included_units: i32,
    pub base_cents: i64,
    pub base_label: String,
    pub overage_cents: i64,
    pub overage_label: String,
    pub total_cents: i64,
    pub total_label: String,
    pub lines: Vec<EstimateLine>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct SubscriptionResp {
    pub plan: String,
    pub plan_name: String,
    pub status: String,
    /// Live footprint.
    pub properties: i32,
    pub units: i32,
    /// What this period would bill at current usage.
    pub estimate: Estimate,
    /// Unpaid (open) platform invoices total.
    pub outstanding_cents: i64,
    pub outstanding_label: String,
    /// The full plan catalogue, with the current plan flagged.
    pub plans: Vec<PlanDto>,
}

/// `GET /billing/subscription` — this workspace's plan, live meter, and the
/// estimated charge for the current billing period.
#[rocket_okapi::openapi(tag = "Billing")]
#[get("/billing/subscription")]
pub async fn subscription(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<SubscriptionResp>> {
    user.require(Permission::BillingRead)?;

    let tenant = Tenant::find_by_id(scope.tenant_id)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("tenant".into()))?;
    let plan = saas::plan_for(&tenant.plan);
    let metered = saas::meter(&db, scope.tenant_id).await?;
    let assembled = saas::assemble(plan, metered.units);

    let outstanding: i64 = PlatformInvoice::find()
        .filter(entity::platform_invoice::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::platform_invoice::Column::Status.eq("open"))
        .all(&db)
        .await?
        .iter()
        .map(|i| i.total_cents)
        .sum();

    let estimate = Estimate {
        unit_count: metered.units,
        included_units: plan.included_units,
        base_cents: assembled.base_cents,
        base_label: usd(assembled.base_cents),
        overage_cents: assembled.overage_cents,
        overage_label: usd(assembled.overage_cents),
        total_cents: assembled.total_cents,
        total_label: usd(assembled.total_cents),
        lines: assembled
            .lines
            .iter()
            .map(|l| EstimateLine {
                description: l.description.clone(),
                quantity: l.quantity,
                amount_cents: l.amount_cents,
                amount_label: usd(l.amount_cents),
            })
            .collect(),
    };

    Ok(Json(SubscriptionResp {
        plan: plan.key.into(),
        plan_name: plan.name.into(),
        status: tenant.status,
        properties: metered.properties,
        units: metered.units,
        estimate,
        outstanding_cents: outstanding,
        outstanding_label: usd(outstanding),
        plans: saas::PLANS
            .iter()
            .map(|p| PlanDto::from(p, plan.key))
            .collect(),
    }))
}
