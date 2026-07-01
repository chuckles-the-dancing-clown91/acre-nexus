use super::dto::{ProfileDto, ProfileInput};
use super::helpers::upsert_profile_inner;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use entity::prelude::*;
use rocket::put;
use rocket::serde::json::Json;
use rocket::State;
use sea_orm::EntityTrait;
use uuid::Uuid;

/// `PUT /admin/users/<id>/profile` — upsert profile; SSN/gov-ID encrypted at rest.
#[rocket_okapi::openapi(tag = "IAM")]
#[put("/admin/users/<id>/profile", data = "<body>")]
pub async fn put_profile(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    id: &str,
    body: Json<ProfileInput>,
) -> ApiResult<Json<ProfileDto>> {
    user.require(Permission::ProfileWrite)?;
    let uid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid user id".into()))?;
    if User::find_by_id(uid).one(&db).await?.is_none() {
        return Err(ApiError::NotFound("user not found".into()));
    }
    upsert_profile_inner(&db, &state.config.pii_key, uid, &body.into_inner()).await?;
    let p = UserProfile::find_by_id(uid).one(&db).await?.unwrap();
    Ok(Json(p.into()))
}
