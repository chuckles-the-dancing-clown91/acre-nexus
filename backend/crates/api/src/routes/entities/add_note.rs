use super::dto::{AddNoteReq, NoteDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::Counterparty;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `POST /entities/<id>/notes` — append a note to a counterparty.
#[rocket_okapi::openapi(tag = "Entities")]
#[post("/entities/<id>/notes", data = "<body>")]
pub async fn add_note(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<AddNoteReq>,
) -> ApiResult<Json<NoteDto>> {
    user.require(Permission::EntityManage)?;
    let cid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    Counterparty::find_by_id(cid)
        .filter(entity::counterparty::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("counterparty not found".into()))?;
    let model = entity::counterparty_note::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        counterparty_id: Set(cid),
        author_user_id: Set(Some(user.user_id)),
        body: Set(body.into_inner().body),
        created_at: Set(Utc::now().into()),
    };
    let saved = model.insert(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::ENTITY_NOTE_ADD,
        Some("counterparty"),
        Some(cid.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "note_id": saved.id })),
    )
    .await;
    Ok(Json(NoteDto::from(saved)))
}
