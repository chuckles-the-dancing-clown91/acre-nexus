//! `POST /properties/onboard` — bring a new house onto the platform in one call:
//! create the property, attach financing (creating lender entities as needed),
//! start its investment workflow, and (optionally) kick off data enrichment.

use super::dto::{OnboardReq, OnboardResp};
use crate::auth::AuthUser;
use crate::enrichment::{Source, ORCHESTRATOR_KIND};
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::scheduler;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, Set};
use uuid::Uuid;

/// `POST /properties/onboard` — full property intake (property + financing).
#[rocket_okapi::openapi(tag = "Onboarding")]
#[post("/properties/onboard", data = "<body>")]
pub async fn onboard(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<OnboardReq>,
) -> ApiResult<Json<OnboardResp>> {
    user.require(Permission::PropertyWrite)?;
    let b = body.into_inner();

    // Validate the strategy and resolve its starting stage.
    let stage = crate::workflow::first_stage(&b.strategy)
        .ok_or_else(|| ApiError::BadRequest(format!("unknown strategy: {}", b.strategy)))?
        .to_string();

    let pid = Uuid::new_v4();
    let now = Utc::now();
    // The whole request runs inside one RLS-scoped transaction (see `crate::db`).

    // ---- property ----
    entity::property::ActiveModel {
        id: Set(pid),
        tenant_id: Set(scope.tenant_id),
        llc_id: Set(b.llc_id),
        portfolio_id: Set(b.portfolio_id),
        name: Set(b.name.clone()),
        address: Set(b.address.clone()),
        city: Set(b.city.clone()),
        units: Set(b.units.unwrap_or(1)),
        occupied_units: Set(b.occupied_units.unwrap_or(0)),
        monthly_rent_cents: Set(b.monthly_rent_cents.unwrap_or(0)),
        status: Set(b.status.clone().unwrap_or_else(|| "Onboarding".into())),
        year_built: Set(b.year_built.unwrap_or(0)),
        manager: Set(b.manager.clone().unwrap_or_default()),
        property_type: Set(b.property_type.clone()),
        strategy: Set(b.strategy.clone()),
        workflow_stage: Set(stage.clone()),
        purchase_price_cents: Set(b.purchase_price_cents),
        acquired_on: Set(b.acquired_on.clone()),
        created_at: Set(now.into()),
    }
    .insert(&db)
    .await?;

    // ---- financing (+ lender entities created on the fly) ----
    let mut lenders_created = 0usize;
    for m in &b.mortgages {
        let lender_id = match (m.lender_id, m.lender_name.as_deref()) {
            (Some(id), _) => Some(id),
            (None, Some(name)) if !name.trim().is_empty() => {
                let cid = Uuid::new_v4();
                entity::counterparty::ActiveModel {
                    id: Set(cid),
                    tenant_id: Set(scope.tenant_id),
                    kind: Set("lender".into()),
                    name: Set(name.trim().to_string()),
                    contact_name: Set(None),
                    email: Set(None),
                    phone: Set(None),
                    website: Set(None),
                    address: Set(None),
                    notes: Set(None),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(&db)
                .await?;
                lenders_created += 1;
                Some(cid)
            }
            _ => None,
        };

        entity::mortgage::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(scope.tenant_id),
            property_id: Set(pid),
            lender_id: Set(lender_id),
            kind: Set(if m.kind.is_empty() {
                "purchase".into()
            } else {
                m.kind.clone()
            }),
            position: Set(m.position.unwrap_or(1)),
            original_amount_cents: Set(m.original_amount_cents),
            current_balance_cents: Set(m.current_balance_cents),
            interest_rate_bps: Set(m.interest_rate_bps),
            term_months: Set(m.term_months),
            monthly_payment_cents: Set(m.monthly_payment_cents),
            escrow_monthly_cents: Set(m.escrow_monthly_cents),
            start_date: Set(m.start_date.clone()),
            maturity_date: Set(m.maturity_date.clone()),
            loan_number: Set(m.loan_number.clone()),
            status: Set("active".into()),
            notes: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&db)
        .await?;
    }

    // ---- initial workflow event ----
    entity::workflow_event::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        property_id: Set(pid),
        strategy: Set(b.strategy.clone()),
        from_stage: Set(None),
        to_stage: Set(stage.clone()),
        note: Set(Some("Onboarded".into())),
        actor_user_id: Set(Some(user.user_id)),
        created_at: Set(now.into()),
    }
    .insert(&db)
    .await?;

    // ---- kick off enrichment (best-effort, off the critical path) ----
    let enrich_job_id = if b.enrich {
        let sources: Vec<&str> = Source::all().iter().map(|s| s.as_str()).collect();
        scheduler::enqueue(
            &db,
            scope.tenant_id,
            ORCHESTRATOR_KIND,
            serde_json::json!({ "property_id": pid.to_string(), "sources": sources }),
            0,
        )
        .await
        .ok()
    } else {
        None
    };

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::PROPERTY_ONBOARD,
        Some("property"),
        Some(pid.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({
            "strategy": b.strategy,
            "property_type": b.property_type,
            "mortgages": b.mortgages.len(),
        })),
    )
    .await;

    Ok(Json(OnboardResp {
        property_id: pid,
        strategy: b.strategy,
        workflow_stage: stage,
        mortgages_created: b.mortgages.len(),
        lenders_created,
        enrich_job_id,
    }))
}
