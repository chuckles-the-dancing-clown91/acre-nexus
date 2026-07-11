use super::dto::{CapitalCallDto, CapitalCallLineDto, CreateCapitalCallReq};
use super::{load_entity, owner_names};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::syndication::split_capital_call;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{CapitalCall, CapitalCallLine, InvestorCommitment};
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use std::collections::HashMap;
use uuid::Uuid;

const MODULE_KEY: &str = "syndication";

/// Assemble a call + its lines into the response DTO.
async fn build_call(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    call: &entity::capital_call::Model,
) -> ApiResult<CapitalCallDto> {
    let lines = CapitalCallLine::find()
        .filter(entity::capital_call_line::Column::TenantId.eq(tenant_id))
        .filter(entity::capital_call_line::Column::CallId.eq(call.id))
        .all(db)
        .await?;
    let names = owner_names(db, tenant_id).await?;
    Ok(CapitalCallDto {
        id: call.id,
        number: call.number,
        amount_cents: call.amount_cents,
        status: call.status.clone(),
        due_date: call.due_date.clone(),
        memo: call.memo.clone(),
        lines: lines
            .into_iter()
            .map(|l| CapitalCallLineDto {
                owner_name: names.get(&l.owner_id).cloned().unwrap_or_default(),
                id: l.id,
                commitment_id: l.commitment_id,
                owner_id: l.owner_id,
                amount_cents: l.amount_cents,
                status: l.status,
            })
            .collect(),
    })
}

/// `POST /entities/<entity_id>/capital-calls` — call capital, split pro-rata by
/// committed capital across the active commitments.
#[rocket_okapi::openapi(tag = "Syndication")]
#[post("/entities/<entity_id>/capital-calls", data = "<body>")]
pub async fn create(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    entity_id: &str,
    body: Json<CreateCapitalCallReq>,
) -> ApiResult<Json<CapitalCallDto>> {
    user.require(Permission::InvestorManage)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, MODULE_KEY).await?;
    let llc = load_entity(&db, scope.tenant_id, entity_id).await?;
    let b = body.into_inner();
    if b.amount_cents <= 0 {
        return Err(ApiError::BadRequest("amount_cents must be positive".into()));
    }

    let commitments = InvestorCommitment::find()
        .filter(entity::investor_commitment::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::investor_commitment::Column::EntityId.eq(llc.id))
        .filter(entity::investor_commitment::Column::Status.eq("active"))
        .order_by_asc(entity::investor_commitment::Column::CreatedAt)
        .all(&db)
        .await?;
    if commitments.is_empty() {
        return Err(ApiError::BadRequest(
            "no active commitments to call capital from".into(),
        ));
    }
    let weights: Vec<i64> = commitments.iter().map(|c| c.committed_cents).collect();
    if weights.iter().sum::<i64>() <= 0 {
        return Err(ApiError::BadRequest(
            "commitments have no committed capital".into(),
        ));
    }
    let split = split_capital_call(b.amount_cents, &weights);

    let number = CapitalCall::find()
        .filter(entity::capital_call::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::capital_call::Column::EntityId.eq(llc.id))
        .count(&db)
        .await? as i32
        + 1;

    let now = Utc::now();
    let call = entity::capital_call::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        entity_id: Set(llc.id),
        number: Set(number),
        amount_cents: Set(b.amount_cents),
        status: Set("open".into()),
        due_date: Set(b.due_date),
        memo: Set(b.memo),
        created_by: Set(Some(user.user_id)),
        created_at: Set(now.into()),
    }
    .insert(&db)
    .await?;

    for (c, amount) in commitments.iter().zip(split) {
        entity::capital_call_line::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(scope.tenant_id),
            call_id: Set(call.id),
            commitment_id: Set(c.id),
            owner_id: Set(c.owner_id),
            amount_cents: Set(amount),
            status: Set("pending".into()),
            created_at: Set(now.into()),
        }
        .insert(&db)
        .await?;
    }

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::CAPITAL_CALL_CREATE,
        Some("capital_call"),
        Some(call.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "entity_id": llc.id, "amount_cents": call.amount_cents })),
    )
    .await;

    Ok(Json(build_call(&db, scope.tenant_id, &call).await?))
}

/// `POST /capital-calls/<id>/fund` — mark a call funded, crediting each
/// investor's contributed capital by their line amount.
#[rocket_okapi::openapi(tag = "Syndication")]
#[post("/capital-calls/<id>/fund")]
pub async fn fund(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<CapitalCallDto>> {
    user.require(Permission::InvestorManage)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, MODULE_KEY).await?;
    let cid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let call = CapitalCall::find_by_id(cid)
        .one(&db)
        .await?
        .filter(|c| c.tenant_id == scope.tenant_id)
        .ok_or_else(|| ApiError::NotFound("capital call not found".into()))?;
    if call.status == "funded" {
        return Err(ApiError::BadRequest(
            "capital call is already funded".into(),
        ));
    }

    let lines = CapitalCallLine::find()
        .filter(entity::capital_call_line::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::capital_call_line::Column::CallId.eq(call.id))
        .all(&db)
        .await?;

    // Cache the commitments once, then credit each by its line.
    let commitments: HashMap<Uuid, entity::investor_commitment::Model> = InvestorCommitment::find()
        .filter(entity::investor_commitment::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::investor_commitment::Column::EntityId.eq(call.entity_id))
        .all(&db)
        .await?
        .into_iter()
        .map(|c| (c.id, c))
        .collect();

    for line in &lines {
        if let Some(c) = commitments.get(&line.commitment_id) {
            let mut cm = c.clone().into_active_model();
            cm.contributed_cents = Set(c.contributed_cents + line.amount_cents);
            cm.update(&db).await?;
        }
        let mut lm = line.clone().into_active_model();
        lm.status = Set("funded".into());
        lm.update(&db).await?;
    }

    let mut call_m = call.clone().into_active_model();
    call_m.status = Set("funded".into());
    let saved = call_m.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::CAPITAL_CALL_FUND,
        Some("capital_call"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        None,
    )
    .await;

    Ok(Json(build_call(&db, scope.tenant_id, &saved).await?))
}
