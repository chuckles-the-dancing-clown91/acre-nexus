use super::dto::{CreateDistributionReq, DistributionDto, DistributionLineDto};
use super::{load_entity, owner_names};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::syndication::{run_waterfall, Stake, WaterfallParams};
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{Distribution, DistributionLine, InvestorCommitment};
use rocket::serde::json::Json;
use rocket::{get, post, State};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use std::collections::HashMap;
use uuid::Uuid;

const MODULE_KEY: &str = "syndication";

/// Assemble a distribution + its lines into the response DTO.
async fn build_distribution(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    dist: &entity::distribution::Model,
    names: &HashMap<Uuid, String>,
) -> ApiResult<DistributionDto> {
    let lines = DistributionLine::find()
        .filter(entity::distribution_line::Column::TenantId.eq(tenant_id))
        .filter(entity::distribution_line::Column::DistributionId.eq(dist.id))
        .all(db)
        .await?;
    Ok(DistributionDto {
        id: dist.id,
        number: dist.number,
        amount_cents: dist.amount_cents,
        pref_rate_bps: dist.pref_rate_bps,
        carry_bps: dist.carry_bps,
        memo: dist.memo.clone(),
        lines: lines
            .iter()
            .map(|l| {
                DistributionLineDto::build(l, names.get(&l.owner_id).cloned().unwrap_or_default())
            })
            .collect(),
    })
}

/// `POST /entities/<entity_id>/distributions` — distribute cash through the
/// waterfall and post the per-investor result.
#[rocket_okapi::openapi(tag = "Syndication")]
#[post("/entities/<entity_id>/distributions", data = "<body>")]
pub async fn create(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    entity_id: &str,
    body: Json<CreateDistributionReq>,
) -> ApiResult<Json<DistributionDto>> {
    user.require(Permission::InvestorManage)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, MODULE_KEY).await?;
    let llc = load_entity(&db, scope.tenant_id, entity_id).await?;
    let b = body.into_inner();
    if b.amount_cents <= 0 {
        return Err(ApiError::BadRequest("amount_cents must be positive".into()));
    }
    let pref_rate_bps = b.pref_rate_bps.unwrap_or(0).max(0);
    let carry_bps = b.carry_bps.unwrap_or(0).clamp(0, 10_000);

    let commitments = InvestorCommitment::find()
        .filter(entity::investor_commitment::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::investor_commitment::Column::EntityId.eq(llc.id))
        .filter(entity::investor_commitment::Column::Status.eq("active"))
        .order_by_asc(entity::investor_commitment::Column::CreatedAt)
        .all(&db)
        .await?;
    if commitments.is_empty() {
        return Err(ApiError::BadRequest(
            "no active commitments to distribute to".into(),
        ));
    }

    let stakes: Vec<Stake> = commitments
        .iter()
        .map(|c| Stake {
            commitment_id: c.id,
            owner_id: c.owner_id,
            contributed_cents: c.contributed_cents,
            unreturned_cents: (c.contributed_cents - c.returned_cents).max(0),
            is_gp: c.role == "manager",
        })
        .collect();
    let allocations = run_waterfall(
        b.amount_cents,
        &stakes,
        &WaterfallParams {
            pref_rate_bps,
            carry_bps,
        },
    );

    let number = Distribution::find()
        .filter(entity::distribution::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::distribution::Column::EntityId.eq(llc.id))
        .count(&db)
        .await? as i32
        + 1;

    let now = Utc::now();
    let dist = entity::distribution::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        entity_id: Set(llc.id),
        number: Set(number),
        amount_cents: Set(b.amount_cents),
        pref_rate_bps: Set(pref_rate_bps),
        carry_bps: Set(carry_bps),
        status: Set("final".into()),
        memo: Set(b.memo),
        created_by: Set(Some(user.user_id)),
        created_at: Set(now.into()),
    }
    .insert(&db)
    .await?;

    for (c, a) in commitments.iter().zip(&allocations) {
        entity::distribution_line::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(scope.tenant_id),
            distribution_id: Set(dist.id),
            commitment_id: Set(c.id),
            owner_id: Set(c.owner_id),
            return_of_capital_cents: Set(a.return_of_capital_cents),
            preferred_cents: Set(a.preferred_cents),
            profit_cents: Set(a.profit_cents),
            carry_cents: Set(a.carry_cents),
            total_cents: Set(a.total_cents()),
            created_at: Set(now.into()),
        }
        .insert(&db)
        .await?;

        // Returned capital reduces the investor's unreturned basis.
        if a.return_of_capital_cents > 0 {
            let mut cm = c.clone().into_active_model();
            cm.returned_cents = Set(c.returned_cents + a.return_of_capital_cents);
            cm.update(&db).await?;
        }
    }

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::DISTRIBUTION_POST,
        Some("distribution"),
        Some(dist.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({
            "entity_id": llc.id,
            "amount_cents": dist.amount_cents,
            "pref_rate_bps": pref_rate_bps,
            "carry_bps": carry_bps,
        })),
    )
    .await;

    let names = owner_names(&db, scope.tenant_id).await?;
    Ok(Json(
        build_distribution(&db, scope.tenant_id, &dist, &names).await?,
    ))
}

/// `GET /entities/<entity_id>/distributions` — distribution history (newest first).
#[rocket_okapi::openapi(tag = "Syndication")]
#[get("/entities/<entity_id>/distributions")]
pub async fn list(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    entity_id: &str,
) -> ApiResult<Json<Vec<DistributionDto>>> {
    user.require(Permission::InvestorRead)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, MODULE_KEY).await?;
    let llc = load_entity(&db, scope.tenant_id, entity_id).await?;

    let dists = Distribution::find()
        .filter(entity::distribution::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::distribution::Column::EntityId.eq(llc.id))
        .order_by_desc(entity::distribution::Column::Number)
        .all(&db)
        .await?;
    let names = owner_names(&db, scope.tenant_id).await?;

    let mut out = Vec::with_capacity(dists.len());
    for d in &dists {
        out.push(build_distribution(&db, scope.tenant_id, d, &names).await?);
    }
    Ok(Json(out))
}
