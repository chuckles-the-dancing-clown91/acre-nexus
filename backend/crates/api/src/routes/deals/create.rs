use super::dto::{CreateDealReq, DealDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, Set};
use uuid::Uuid;

/// `POST /modules/flips/deals` — start a new acquisition deal in the pipeline
/// (at `prospecting`). Only `name` is required.
#[rocket_okapi::openapi(tag = "Flips")]
#[post("/modules/flips/deals", data = "<body>")]
pub async fn create(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<CreateDealReq>,
) -> ApiResult<Json<DealDto>> {
    user.require(Permission::DealWrite)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "flips").await?;

    let b = body.into_inner();
    let name = b.name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::BadRequest("name is required".into()));
    }
    let strategy = b
        .strategy
        .filter(|s| crate::workflow::strategy(s).is_some())
        .unwrap_or_else(|| "flip".to_string());

    let now = Utc::now();
    let id = Uuid::new_v4();
    let deal = entity::deal::ActiveModel {
        id: Set(id),
        tenant_id: Set(scope.tenant_id),
        name: Set(name.clone()),
        address: Set(b.address.unwrap_or_default()),
        city: Set(b.city.unwrap_or_default()),
        stage: Set(crate::deals::FIRST_STAGE.to_string()),
        strategy: Set(strategy),
        property_type: Set(b.property_type),
        source: Set(b.source),
        broker_id: Set(b.broker_id),
        notes: Set(b.notes),
        asking_price_cents: Set(b.asking_price_cents),
        offer_price_cents: Set(b.offer_price_cents),
        earnest_money_cents: Set(None),
        target_close_on: Set(None),
        arv_cents: Set(None),
        rehab_budget_cents: Set(b.rehab_budget_cents),
        closing_costs_cents: Set(None),
        est_monthly_rent_cents: Set(b.est_monthly_rent_cents),
        est_monthly_expenses_cents: Set(None),
        vacancy_bps: Set(None),
        down_payment_bps: Set(None),
        interest_rate_bps: Set(None),
        loan_term_years: Set(None),
        rent_growth_bps: Set(None),
        appreciation_bps: Set(None),
        exit_cap_rate_bps: Set(None),
        selling_costs_bps: Set(None),
        hold_years: Set(None),
        checklist: Set(serde_json::json!([])),
        converted_property_id: Set(None),
        created_by: Set(Some(user.user_id)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;

    entity::deal_event::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        deal_id: Set(id),
        kind: Set("created".into()),
        from_stage: Set(None),
        to_stage: Set(Some(deal.stage.clone())),
        body: Set(None),
        actor_user_id: Set(Some(user.user_id)),
        created_at: Set(now.into()),
    }
    .insert(&db)
    .await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::DEAL_CREATE,
        Some("deal"),
        Some(id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "name": name, "strategy": deal.strategy })),
    )
    .await;

    Ok(Json(DealDto::build(&deal)))
}
