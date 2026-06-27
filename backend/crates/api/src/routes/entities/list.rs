use super::dto::CounterpartyDto;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Counterparty;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

/// `GET /entities?<kind>` — list the active tenant's counterparties, optionally
/// filtered by `kind`, ordered by name.
#[rocket_okapi::openapi(tag = "Entities")]
#[get("/entities?<kind>")]
pub async fn list(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    kind: Option<String>,
) -> ApiResult<Json<Vec<CounterpartyDto>>> {
    user.require(Permission::EntityRead)?;
    let mut q =
        Counterparty::find().filter(entity::counterparty::Column::TenantId.eq(scope.tenant_id));
    if let Some(k) = kind {
        if !k.is_empty() {
            q = q.filter(entity::counterparty::Column::Kind.eq(k));
        }
    }
    let rows = q
        .order_by_asc(entity::counterparty::Column::Name)
        .all(&state.db)
        .await?;
    Ok(Json(rows.into_iter().map(CounterpartyDto::from).collect()))
}
