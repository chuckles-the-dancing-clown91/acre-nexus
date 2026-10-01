//! Saved replies: canned answers staff drop into a text ("We got your request
//! and will be out tomorrow between 9 and 12").

use super::{SavedReplyDto, SavedReplyReq};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::tenancy::TenantScope;
use crate::texts::MAX_TEXT_CHARS;
use chrono::Utc;
use entity::prelude::TextSavedReply;
use rocket::serde::json::Json;
use rocket::{delete, get, patch, post};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use uuid::Uuid;

/// Most saved replies a workspace keeps.
const MAX_REPLIES: usize = 200;

fn check(b: &SavedReplyReq) -> ApiResult<(String, String)> {
    let title = b.title.trim();
    let body = b.body.trim();
    if title.is_empty() || body.is_empty() {
        return Err(ApiError::BadRequest(
            "a saved reply needs a title and text".into(),
        ));
    }
    if title.chars().count() > 80 {
        return Err(ApiError::BadRequest(
            "keep the title under 80 characters".into(),
        ));
    }
    if body.chars().count() > MAX_TEXT_CHARS {
        return Err(ApiError::BadRequest(format!(
            "texts are limited to {MAX_TEXT_CHARS} characters"
        )));
    }
    Ok((title.to_string(), body.to_string()))
}

async fn find(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    id: &str,
) -> ApiResult<entity::text_saved_reply::Model> {
    let rid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    TextSavedReply::find_by_id(rid)
        .filter(entity::text_saved_reply::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("saved reply not found".into()))
}

/// `GET /texts/replies` — the workspace's saved replies, by title.
#[rocket_okapi::openapi(tag = "Texts")]
#[get("/texts/replies")]
pub async fn list(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<SavedReplyDto>>> {
    user.require(Permission::MessageRead)?;
    let rows = TextSavedReply::find()
        .filter(entity::text_saved_reply::Column::TenantId.eq(scope.tenant_id))
        .order_by_asc(entity::text_saved_reply::Column::Title)
        .all(&db)
        .await?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

/// `POST /texts/replies` — save a reply.
#[rocket_okapi::openapi(tag = "Texts")]
#[post("/texts/replies", data = "<body>")]
pub async fn create(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<SavedReplyReq>,
) -> ApiResult<Json<SavedReplyDto>> {
    user.require(Permission::MessageManage)?;
    let (title, text) = check(&body)?;
    let have = TextSavedReply::find()
        .filter(entity::text_saved_reply::Column::TenantId.eq(scope.tenant_id))
        .all(&db)
        .await?
        .len();
    if have >= MAX_REPLIES {
        return Err(ApiError::BadRequest(format!(
            "a workspace keeps up to {MAX_REPLIES} saved replies"
        )));
    }
    let now = Utc::now();
    let saved = entity::text_saved_reply::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        title: Set(title),
        body: Set(text),
        created_by: Set(Some(user.user_id)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;
    Ok(Json(saved.into()))
}

/// `PATCH /texts/replies/<id>` — edit a saved reply.
#[rocket_okapi::openapi(tag = "Texts")]
#[patch("/texts/replies/<id>", data = "<body>")]
pub async fn update(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<SavedReplyReq>,
) -> ApiResult<Json<SavedReplyDto>> {
    user.require(Permission::MessageManage)?;
    let (title, text) = check(&body)?;
    let row = find(&db, scope.tenant_id, id).await?;
    let mut am: entity::text_saved_reply::ActiveModel = row.into();
    am.title = Set(title);
    am.body = Set(text);
    am.updated_at = Set(Utc::now().into());
    Ok(Json(am.update(&db).await?.into()))
}

/// `DELETE /texts/replies/<id>` — remove a saved reply.
#[rocket_okapi::openapi(tag = "Texts")]
#[delete("/texts/replies/<id>")]
pub async fn remove(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::MessageManage)?;
    let row = find(&db, scope.tenant_id, id).await?;
    TextSavedReply::delete_by_id(row.id).exec(&db).await?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}
