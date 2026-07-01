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
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::MemberManage)?;
    let mid =
        Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid membership id".into()))?;
    // Fetch first so the audit entry captures what was removed (and its tenant).
    let m = Membership::find_by_id(mid)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("membership not found".into()))?;
    Membership::delete_by_id(mid).exec(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::MEMBERSHIP_REMOVE,
        Some("user"),
        Some(m.user_id.to_string()),
        m.tenant_id,
        Some(serde_json::json!({
            "membership_id": mid,
            "profile_type": m.profile_type,
            "scope": m.scope,
        })),
    )
    .await;

    Ok(Json(serde_json::json!({ "deleted": true })))
}
