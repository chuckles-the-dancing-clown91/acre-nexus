use super::dto::{UpdateUserReq, UserDetail};
use super::helpers::load_user_detail;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use entity::prelude::*;
use rocket::patch;
use rocket::serde::json::Json;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use uuid::Uuid;

/// `PATCH /admin/users/<id>` — update identity fields / status.
#[rocket_okapi::openapi(tag = "IAM")]
#[patch("/admin/users/<id>", data = "<body>")]
pub async fn update_user(
    db: crate::db::RequestDb,
    user: AuthUser,
    id: &str,
    body: Json<UpdateUserReq>,
) -> ApiResult<Json<UserDetail>> {
    user.require(Permission::UserManage)?;
    let uid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid user id".into()))?;
    let u = User::find_by_id(uid)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("user not found".into()))?;
    let before = serde_json::json!({
        "name": u.name,
        "username": u.username,
        "status": u.status,
    });
    let tenant_id = u.tenant_id;
    let body = body.into_inner();
    let mut am: entity::user::ActiveModel = u.into();
    if let Some(name) = body.name {
        am.name = Set(name);
    }
    if let Some(username) = body.username {
        am.username = Set(Some(username));
    }
    if let Some(status) = body.status {
        am.status = Set(status);
    }
    let saved = am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::USER_UPDATE,
        Some("user"),
        Some(uid.to_string()),
        tenant_id,
        Some(serde_json::json!({
            "before": before,
            "after": {
                "name": saved.name,
                "username": saved.username,
                "status": saved.status,
            },
        })),
    )
    .await;

    load_user_detail(&db, uid).await
}
