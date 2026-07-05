use super::dto::{LedgerEntryDto, LedgerTxnDto, ManualTxnReq};
use crate::accounting::{Leg, PostArgs, Side};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{LedgerAccount, LedgerEntry, LedgerTxn};
use rocket::serde::json::Json;
use rocket::{get, post, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use std::collections::HashMap;
use uuid::Uuid;

const MAX_TXNS: u64 = 200;

/// `GET /accounting/transactions?entity=<llc>&limit=` — the journal, newest
/// first, each transaction with its balanced legs.
#[rocket_okapi::openapi(tag = "Accounting")]
#[get("/accounting/transactions?<entity>&<limit>")]
pub async fn list_transactions(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    entity: &str,
    limit: Option<u64>,
) -> ApiResult<Json<Vec<LedgerTxnDto>>> {
    user.require(Permission::LedgerRead)?;
    let entity_id = super::accounts::parse_entity(&db, scope.tenant_id, entity).await?;

    let txns = LedgerTxn::find()
        .filter(entity::ledger_txn::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::ledger_txn::Column::EntityId.eq(entity_id))
        .order_by_desc(entity::ledger_txn::Column::TxnDate)
        .order_by_desc(entity::ledger_txn::Column::CreatedAt)
        .limit(limit.unwrap_or(50).min(MAX_TXNS))
        .all(&db)
        .await?;
    let txn_ids: Vec<Uuid> = txns.iter().map(|t| t.id).collect();

    let entries = if txn_ids.is_empty() {
        vec![]
    } else {
        LedgerEntry::find()
            .filter(entity::ledger_entry::Column::TenantId.eq(scope.tenant_id))
            .filter(entity::ledger_entry::Column::TxnId.is_in(txn_ids))
            .all(&db)
            .await?
    };
    let account_ids: Vec<Uuid> = entries.iter().map(|e| e.account_id).collect();
    let accounts: HashMap<Uuid, entity::ledger_account::Model> = if account_ids.is_empty() {
        HashMap::new()
    } else {
        LedgerAccount::find()
            .filter(entity::ledger_account::Column::TenantId.eq(scope.tenant_id))
            .filter(entity::ledger_account::Column::Id.is_in(account_ids))
            .all(&db)
            .await?
            .into_iter()
            .map(|a| (a.id, a))
            .collect()
    };

    let mut by_txn: HashMap<Uuid, Vec<LedgerEntryDto>> = HashMap::new();
    for e in entries {
        let (code, name) = accounts
            .get(&e.account_id)
            .map(|a| (a.code.clone(), a.name.clone()))
            .unwrap_or_default();
        by_txn.entry(e.txn_id).or_default().push(LedgerEntryDto {
            id: e.id,
            account_id: e.account_id,
            account_code: code,
            account_name: name,
            side: e.side,
            amount_cents: e.amount_cents,
            amount_label: crate::dto::usd(e.amount_cents),
            property_id: e.property_id,
            lease_id: e.lease_id,
        });
    }

    Ok(Json(
        txns.into_iter()
            .map(|t| {
                let mut entries = by_txn.remove(&t.id).unwrap_or_default();
                // Debits first, then credits — conventional journal layout.
                entries.sort_by_key(|e| e.side.clone());
                LedgerTxnDto {
                    id: t.id,
                    entity_id: t.entity_id,
                    txn_date: t.txn_date,
                    memo: t.memo,
                    source_type: t.source_type,
                    source_id: t.source_id,
                    posted_by: t.posted_by,
                    created_at: t.created_at.to_rfc3339(),
                    entries,
                }
            })
            .collect(),
    ))
}

/// `POST /accounting/transactions` — post a manual journal entry (e.g. an
/// operating expense). The posting engine enforces balance + trust integrity.
#[rocket_okapi::openapi(tag = "Accounting")]
#[post("/accounting/transactions", data = "<body>")]
pub async fn post_transaction(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<ManualTxnReq>,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::LedgerManage)?;
    let b = body.into_inner();
    super::accounts::parse_entity(&db, scope.tenant_id, &b.entity_id.to_string()).await?;
    if b.memo.trim().is_empty() {
        return Err(ApiError::BadRequest("memo is required".into()));
    }
    let date = b
        .txn_date
        .unwrap_or_else(|| chrono::Utc::now().date_naive().to_string());
    let legs: Vec<Leg> = b
        .legs
        .into_iter()
        .map(|l| {
            let side = match l.side.as_str() {
                "debit" => Side::Debit,
                "credit" => Side::Credit,
                _ => return Err(ApiError::BadRequest("side must be debit|credit".into())),
            };
            Ok(Leg {
                account_id: l.account_id,
                side,
                amount_cents: l.amount_cents,
                property_id: None,
                lease_id: None,
            })
        })
        .collect::<Result<_, _>>()?;

    let txn = crate::accounting::post(
        &db,
        scope.tenant_id,
        PostArgs {
            entity_id: b.entity_id,
            txn_date: &date,
            memo: b.memo.trim(),
            source_type: "manual",
            source_id: None,
            posted_by: Some(user.user_id),
        },
        legs,
    )
    .await?;

    Ok(Json(serde_json::json!({ "id": txn.id })))
}
