//! `POST /modules/flips/deals/<id>/convert` — turn a closed deal into a fully
//! onboarded, owned [`property`](entity::property), closing the acquisition loop.

use super::dto::{ConvertResp, DealDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, IntoActiveModel, Set};
use uuid::Uuid;

/// Convert a deal into an owned property. Copies name/address/strategy/price/
/// rent onto a new property, starts its investment workflow, marks the deal
/// `owned`, and links the two. Idempotency: a deal that already converted is
/// rejected.
#[rocket_okapi::openapi(tag = "Flips")]
#[post("/modules/flips/deals/<id>/convert")]
pub async fn convert(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<ConvertResp>> {
    user.require(Permission::DealWrite)?;
    // Converting mints a property, so require the property-write grant too.
    user.require(Permission::PropertyWrite)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "flips").await?;

    let deal = super::load_deal(&db, scope.tenant_id, id).await?;
    if let Some(existing) = deal.converted_property_id {
        return Err(ApiError::BadRequest(format!(
            "deal already converted to property {existing}"
        )));
    }

    let now = Utc::now();
    let pid = Uuid::new_v4();
    let strategy = deal.strategy.clone();
    let stage = crate::workflow::first_stage(&strategy)
        .unwrap_or("acquisition")
        .to_string();
    let purchase_price = deal.offer_price_cents.or(deal.asking_price_cents);
    let property_type = deal
        .property_type
        .clone()
        .unwrap_or_else(|| "single_family".into());

    entity::property::ActiveModel {
        id: Set(pid),
        tenant_id: Set(scope.tenant_id),
        llc_id: Set(None),
        portfolio_id: Set(None),
        name: Set(deal.name.clone()),
        address: Set(deal.address.clone()),
        city: Set(deal.city.clone()),
        units: Set(1),
        occupied_units: Set(0),
        monthly_rent_cents: Set(deal.est_monthly_rent_cents.unwrap_or(0)),
        status: Set("Onboarding".into()),
        year_built: Set(0),
        manager: Set(String::new()),
        property_type: Set(property_type),
        strategy: Set(strategy.clone()),
        workflow_stage: Set(stage.clone()),
        purchase_price_cents: Set(purchase_price),
        acquired_on: Set(Some(now.date_naive().format("%Y-%m-%d").to_string())),
        image_url: Set(None),
        created_at: Set(now.into()),
    }
    .insert(&db)
    .await?;

    entity::workflow_event::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        property_id: Set(pid),
        strategy: Set(strategy.clone()),
        from_stage: Set(None),
        to_stage: Set(stage.clone()),
        note: Set(Some("Converted from acquisition deal".into())),
        actor_user_id: Set(Some(user.user_id)),
        created_at: Set(now.into()),
    }
    .insert(&db)
    .await?;

    let from_stage = deal.stage.clone();
    let mut m = deal.into_active_model();
    m.stage = Set("owned".into());
    m.converted_property_id = Set(Some(pid));
    m.updated_at = Set(now.into());
    let saved = m.update(&db).await?;

    entity::deal_event::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        deal_id: Set(saved.id),
        kind: Set("converted".into()),
        from_stage: Set(Some(from_stage)),
        to_stage: Set(Some("owned".into())),
        body: Set(Some(format!("Converted to property {pid}"))),
        actor_user_id: Set(Some(user.user_id)),
        created_at: Set(now.into()),
    }
    .insert(&db)
    .await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::DEAL_CONVERT,
        Some("deal"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "property_id": pid, "strategy": strategy })),
    )
    .await;

    Ok(Json(ConvertResp {
        deal: DealDto::build(&saved),
        property_id: pid,
    }))
}
