use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use entity::prelude::*;
use rocket::delete;
use rocket::serde::json::Json;
use rocket::State;
use sea_orm::EntityTrait;
use uuid::Uuid;

/// `DELETE /admin/memberships/<id>` — remove a membership.
#[rocket_okapi::openapi(tag = "IAM")]
#[delete("/admin/memberships/<id>")]
pub async fn remove_membership(
    state: &State<AppState>,
    user: AuthUser,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::MemberManage)?;
    let mid =
        Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid membership id".into()))?;
    Membership::delete_by_id(mid).exec(&state.db).await?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}
