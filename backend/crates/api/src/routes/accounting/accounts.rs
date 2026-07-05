use super::dto::{CreateAccountReq, LedgerAccountDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{LedgerAccount, Llc};
use rocket::serde::json::Json;
use rocket::{get, post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `GET /accounting/accounts?entity=<llc>` — the entity's chart of accounts
/// with lifetime activity + balances. Seeds the default chart on first read
/// so a fresh entity always has books.
#[rocket_okapi::openapi(tag = "Accounting")]
#[get("/accounting/accounts?<entity>")]
pub async fn list_accounts(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    entity: &str,
) -> ApiResult<Json<Vec<LedgerAccountDto>>> {
    user.require(Permission::LedgerRead)?;
    let entity_id = parse_entity(&db, scope.tenant_id, entity).await?;
    crate::accounting::ensure_chart(&db, scope.tenant_id, entity_id).await?;
    let activity =
        crate::accounting::account_activity(&db, scope.tenant_id, entity_id, None, None).await?;
    let mut rows: Vec<LedgerAccountDto> = activity
        .into_iter()
        .map(LedgerAccountDto::from_activity)
        .collect();
    rows.sort_by(|a, b| a.code.cmp(&b.code));
    Ok(Json(rows))
}

/// `POST /accounting/accounts` — add a custom account to an entity's chart.
#[rocket_okapi::openapi(tag = "Accounting")]
#[post("/accounting/accounts", data = "<body>")]
pub async fn create_account(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<CreateAccountReq>,
) -> ApiResult<Json<LedgerAccountDto>> {
    user.require(Permission::LedgerManage)?;
    let b = body.into_inner();
    Llc::find_by_id(b.entity_id)
        .filter(entity::llc::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("legal entity not found".into()))?;
    if crate::accounting::AccountKind::parse(&b.kind).is_none() {
        return Err(ApiError::BadRequest(
            "kind must be one of asset|liability|equity|income|expense".into(),
        ));
    }
    let code = b.code.trim().to_string();
    let name = b.name.trim().to_string();
    if code.is_empty() || name.is_empty() {
        return Err(ApiError::BadRequest("code and name are required".into()));
    }
    let clash = LedgerAccount::find()
        .filter(entity::ledger_account::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::ledger_account::Column::EntityId.eq(b.entity_id))
        .filter(entity::ledger_account::Column::Code.eq(code.clone()))
        .one(&db)
        .await?;
    if clash.is_some() {
        return Err(ApiError::Conflict(format!(
            "account code {code} already exists for this entity"
        )));
    }

    let saved = entity::ledger_account::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        entity_id: Set(b.entity_id),
        code: Set(code),
        name: Set(name),
        kind: Set(b.kind),
        subtype: Set(None),
        is_trust: Set(b.is_trust.unwrap_or(false)),
        system: Set(false),
        active: Set(true),
        created_at: Set(Utc::now().into()),
    }
    .insert(&db)
    .await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::LEDGER_ACCOUNT_CREATE,
        Some("ledger_account"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({
            "entity_id": saved.entity_id,
            "code": saved.code,
            "kind": saved.kind,
            "is_trust": saved.is_trust,
        })),
    )
    .await;

    Ok(Json(LedgerAccountDto::from_activity(
        crate::accounting::AccountActivity {
            account: saved,
            debit_cents: 0,
            credit_cents: 0,
        },
    )))
}

/// Parse + tenant-check an entity (LLC) id used by the accounting endpoints.
pub async fn parse_entity(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    entity: &str,
) -> ApiResult<Uuid> {
    let eid =
        Uuid::parse_str(entity).map_err(|_| ApiError::BadRequest("invalid entity id".into()))?;
    Llc::find_by_id(eid)
        .filter(entity::llc::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("legal entity not found".into()))?;
    Ok(eid)
}
