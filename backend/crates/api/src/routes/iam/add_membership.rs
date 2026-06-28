use super::dto::{MembershipDto, NewMembership};
use super::helpers::add_membership_inner;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use entity::prelude::*;
use rocket::post;
use rocket::serde::json::Json;
use rocket::State;
use sea_orm::EntityTrait;
use uuid::Uuid;

/// `POST /admin/users/<id>/memberships` — add a persona; auto-grants its default role.
#[rocket_okapi::openapi(tag = "IAM")]
#[post("/admin/users/<id>/memberships", data = "<body>")]
pub async fn add_membership(
    state: &State<AppState>,
    user: AuthUser,
    id: &str,
    body: Json<NewMembership>,
) -> ApiResult<Json<MembershipDto>> {
    user.require(Permission::MemberManage)?;
    let uid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid user id".into()))?;
    if User::find_by_id(uid).one(&state.user_db).await?.is_none() {
        return Err(ApiError::NotFound("user not found".into()));
    }
    let m = add_membership_inner(&state.user_db, uid, &body.into_inner(), false).await?;
    Ok(Json(MembershipDto {
        id: m.id,
        scope: m.scope,
        tenant_id: m.tenant_id,
        profile_type: m.profile_type,
        title: m.title,
        status: m.status,
        is_primary: m.is_primary,
    }))
}
