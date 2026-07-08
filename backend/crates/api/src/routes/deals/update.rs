use super::dto::{DealDto, UpdateDealReq};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use rocket::serde::json::Json;
use rocket::{patch, State};
use sea_orm::{ActiveModelTrait, IntoActiveModel, Set};

/// `PATCH /modules/flips/deals/<id>` — edit deal fields and underwriting
/// assumptions. Every field is optional; a `null`/omitted field is left
/// unchanged. Stage is moved via the dedicated `/stage` endpoint, not here.
#[rocket_okapi::openapi(tag = "Flips")]
#[patch("/modules/flips/deals/<id>", data = "<body>")]
pub async fn update(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateDealReq>,
) -> ApiResult<Json<DealDto>> {
    user.require(Permission::DealWrite)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "flips").await?;

    let b = body.into_inner();
    let deal = super::load_deal(&db, scope.tenant_id, id).await?;
    let mut m = deal.into_active_model();

    if let Some(v) = b.name {
        let v = v.trim().to_string();
        if v.is_empty() {
            return Err(ApiError::BadRequest("name cannot be empty".into()));
        }
        m.name = Set(v);
    }
    if let Some(v) = b.address {
        m.address = Set(v);
    }
    if let Some(v) = b.city {
        m.city = Set(v);
    }
    if let Some(v) = b.strategy {
        if crate::workflow::strategy(&v).is_none() {
            return Err(ApiError::BadRequest(format!("unknown strategy: {v}")));
        }
        m.strategy = Set(v);
    }
    if let Some(v) = b.property_type {
        m.property_type = Set(Some(v));
    }
    if let Some(v) = b.source {
        m.source = Set(Some(v));
    }
    if let Some(v) = b.broker_id {
        m.broker_id = Set(Some(v));
    }
    if let Some(v) = b.notes {
        m.notes = Set(Some(v));
    }
    if let Some(v) = b.asking_price_cents {
        m.asking_price_cents = Set(Some(v));
    }
    if let Some(v) = b.offer_price_cents {
        m.offer_price_cents = Set(Some(v));
    }
    if let Some(v) = b.earnest_money_cents {
        m.earnest_money_cents = Set(Some(v));
    }
    if let Some(v) = b.target_close_on {
        m.target_close_on = Set(Some(v));
    }
    if let Some(v) = b.arv_cents {
        m.arv_cents = Set(Some(v));
    }
    if let Some(v) = b.rehab_budget_cents {
        m.rehab_budget_cents = Set(Some(v));
    }
    if let Some(v) = b.closing_costs_cents {
        m.closing_costs_cents = Set(Some(v));
    }
    if let Some(v) = b.est_monthly_rent_cents {
        m.est_monthly_rent_cents = Set(Some(v));
    }
    if let Some(v) = b.est_monthly_expenses_cents {
        m.est_monthly_expenses_cents = Set(Some(v));
    }
    if let Some(v) = b.vacancy_bps {
        m.vacancy_bps = Set(Some(v));
    }
    if let Some(v) = b.down_payment_bps {
        m.down_payment_bps = Set(Some(v));
    }
    if let Some(v) = b.interest_rate_bps {
        m.interest_rate_bps = Set(Some(v));
    }
    if let Some(v) = b.loan_term_years {
        m.loan_term_years = Set(Some(v));
    }
    if let Some(v) = b.rent_growth_bps {
        m.rent_growth_bps = Set(Some(v));
    }
    if let Some(v) = b.appreciation_bps {
        m.appreciation_bps = Set(Some(v));
    }
    if let Some(v) = b.exit_cap_rate_bps {
        m.exit_cap_rate_bps = Set(Some(v));
    }
    if let Some(v) = b.selling_costs_bps {
        m.selling_costs_bps = Set(Some(v));
    }
    if let Some(v) = b.hold_years {
        m.hold_years = Set(Some(v));
    }
    m.updated_at = Set(Utc::now().into());

    let saved = m.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::DEAL_UPDATE,
        Some("deal"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        None,
    )
    .await;

    Ok(Json(DealDto::build(&saved)))
}
