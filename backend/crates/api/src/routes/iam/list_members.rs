use super::dto::MemberDto;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::*;
use rocket::serde::json::Json;
use rocket::State;
use rocket::{get, post};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

/// `GET /members` — the active tenant's member directory (persona + status).
#[rocket_okapi::openapi(tag = "IAM")]
#[get("/members")]
pub async fn list_members(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<MemberDto>>> {
    user.require(Permission::MemberRead)?;
    let memberships = Membership::find()
        .filter(entity::membership::Column::TenantId.eq(scope.tenant_id))
        .all(&db)
        .await?;
    let mut out = Vec::new();
    for m in memberships {
        if let Some(u) = User::find_by_id(m.user_id).one(&db).await? {
            out.push(MemberDto {
                membership_id: m.id,
                user_id: u.id,
                name: u.name,
                email: u.email,
                profile_type: m.profile_type,
                title: m.title,
                status: m.status,
                account_status: u.status,
            });
        }
    }
    Ok(Json(out))
}

/// `POST /members/<membership_id>/login-link` — send a member a fresh link to
/// choose their password: an invite while they haven't signed in yet, a reset
/// once they have. The previous link stops working.
#[rocket_okapi::openapi(tag = "IAM")]
#[post("/members/<membership_id>/login-link")]
pub async fn send_login_link(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    membership_id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::MemberManage)?;
    let membership_id = uuid::Uuid::parse_str(membership_id)
        .map_err(|_| crate::error::ApiError::BadRequest("invalid member id".into()))?;
    let m = Membership::find_by_id(membership_id)
        .filter(entity::membership::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| crate::error::ApiError::NotFound("member not found".into()))?;
    let account = User::find_by_id(m.user_id)
        .one(&db)
        .await?
        .ok_or_else(|| crate::error::ApiError::NotFound("member not found".into()))?;
    if account.status != "active" && account.status != "invited" {
        return Err(crate::error::ApiError::Conflict(format!(
            "account is {} — reactivate it first",
            account.status
        )));
    }
    let purpose = if account.status == "invited" {
        crate::password_links::PURPOSE_INVITE
    } else {
        crate::password_links::PURPOSE_RESET
    };
    let (token, row) = crate::password_links::issue(&db, account.id, purpose).await?;
    let sent =
        crate::password_links::deliver(&db, Some(scope.tenant_id), &account, &token, &row).await;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::AUTH_LOGIN_LINK_SEND,
        Some("user"),
        Some(account.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "purpose": purpose })),
    )
    .await;
    Ok(Json(serde_json::json!({ "ok": sent, "purpose": purpose })))
}
