use super::dto::{ProfileDto, ProfileInput};
use super::helpers::{profile_fields_touched, upsert_profile_inner};
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
    let target = User::find_by_id(uid)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("user not found".into()))?;
    let input = body.into_inner();
    let fields = profile_fields_touched(&input);

    upsert_profile_inner(&db, &state.config.pii_key, uid, &input).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::PROFILE_WRITE,
        Some("user"),
        Some(uid.to_string()),
        target.tenant_id,
        Some(serde_json::json!({ "fields_set": fields })),
    )
    .await;

    let p = UserProfile::find_by_id(uid)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::Internal(anyhow::anyhow!("profile vanished after upsert")))?;
    Ok(Json(p.into()))
}
