use super::dto::{CounterpartyDetailDto, CounterpartyDto, NoteDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{Counterparty, CounterpartyNote};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

/// `GET /entities/<id>` — a counterparty plus its notes (newest first).
#[rocket_okapi::openapi(tag = "Entities")]
#[get("/entities/<id>")]
pub async fn get(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<CounterpartyDetailDto>> {
    user.require(Permission::EntityRead)?;
    let cid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let c = Counterparty::find_by_id(cid)
        .filter(entity::counterparty::Column::TenantId.eq(scope.tenant_id))
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("counterparty not found".into()))?;
    let notes = CounterpartyNote::find()
        .filter(entity::counterparty_note::Column::CounterpartyId.eq(cid))
        .order_by_desc(entity::counterparty_note::Column::CreatedAt)
        .all(&state.db)
        .await?;
    Ok(Json(CounterpartyDetailDto {
        entity: CounterpartyDto::from(c),
        notes: notes.into_iter().map(NoteDto::from).collect(),
    }))
}
