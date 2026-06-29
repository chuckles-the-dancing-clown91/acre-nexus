//! `GET /entities/<entity_id>/bank-accounts` — accounts for one legal entity.

use super::dto::BankAccountResp;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{BankAccount, Llc};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

/// `GET /entities/<entity_id>/bank-accounts` — list operating + trust accounts.
#[rocket_okapi::openapi(tag = "Legal Entities")]
#[get("/entities/<entity_id>/bank-accounts")]
pub async fn list(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    entity_id: &str,
) -> ApiResult<Json<Vec<BankAccountResp>>> {
    user.require(Permission::FinanceRead)?;
    let eid =
        Uuid::parse_str(entity_id).map_err(|_| ApiError::BadRequest("invalid entity id".into()))?;
    Llc::find_by_id(eid)
        .one(&state.db)
        .await?
        .filter(|l| l.tenant_id == scope.tenant_id)
        .ok_or_else(|| ApiError::NotFound("legal entity not found".into()))?;

    let rows = BankAccount::find()
        .filter(entity::bank_account::Column::EntityId.eq(eid))
        .filter(entity::bank_account::Column::TenantId.eq(scope.tenant_id))
        .order_by_asc(entity::bank_account::Column::Kind)
        .all(&state.db)
        .await?;
    Ok(Json(rows.into_iter().map(BankAccountResp::from).collect()))
}
