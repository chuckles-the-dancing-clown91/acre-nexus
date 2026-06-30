//! `POST /platform/impersonate` — begin an **audited, time-boxed** impersonation
//! session into a tenant. Platform staff are never tenant members; this is the
//! only way they act inside a client workspace. The session is reason-logged,
//! expires after a short TTL, and is revocable; the minted access token carries
//! the staff actor's platform permissions scoped to the tenant.

use super::dto::{ImpersonateReq, ImpersonationResp};
use crate::auth::{issue_access_token, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::routes::auth::helpers::permissions_for;
use crate::state::AppState;
use crate::tenancy::helpers::resolve_tenant_ref;
use chrono::{Duration, Utc};
use entity::prelude::{PlatformStaff, Tenant};
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// Default impersonation lifetime — short, renewable, fully revocable (§3.3).
const IMPERSONATION_TTL_SECS: i64 = 30 * 60;

/// `POST /platform/impersonate` — start a session and mint a tenant-scoped token.
#[rocket_okapi::openapi(tag = "Platform Admin")]
#[post("/platform/impersonate", data = "<body>")]
pub async fn impersonate(
    state: &State<AppState>,
    user: AuthUser,
    body: Json<ImpersonateReq>,
) -> ApiResult<Json<ImpersonationResp>> {
    user.require(Permission::ImpersonateTenant)?;
    let b = body.into_inner();
    if b.reason.trim().is_empty() {
        return Err(ApiError::BadRequest("a reason is required".into()));
    }

    // The actor must be on the platform plane (have a platform_staff row).
    let staff = PlatformStaff::find()
        .filter(entity::platform_staff::Column::UserId.eq(user.user_id))
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::Forbidden("not a platform-staff member".into()))?;
    if staff.status != "active" {
        return Err(ApiError::Forbidden(format!(
            "platform-staff account is {}",
            staff.status
        )));
    }

    let tenant_id = resolve_tenant_ref(state, &b.tenant)
        .await
        .ok_or_else(|| ApiError::NotFound("tenant not found".into()))?;
    if Tenant::find_by_id(tenant_id)
        .one(&state.db)
        .await?
        .is_none()
    {
        return Err(ApiError::NotFound("tenant not found".into()));
    }

    let now = Utc::now();
    let expires_at = now + Duration::seconds(IMPERSONATION_TTL_SECS);
    let session_id = Uuid::new_v4();
    entity::impersonation_session::ActiveModel {
        id: Set(session_id),
        platform_staff_id: Set(staff.id),
        tenant_id: Set(tenant_id),
        reason: Set(b.reason.clone()),
        expires_at: Set(expires_at.into()),
        revoked_at: Set(None),
        created_at: Set(now.into()),
    }
    .insert(&state.db)
    .await?;

    // Mint a tenant-scoped token carrying the staff actor's platform permissions.
    let perms = permissions_for(&state.db, user.user_id, Some(tenant_id)).await?;
    let access = issue_access_token(&state.config, user.user_id, Some(tenant_id), true, perms)
        .map_err(ApiError::Internal)?;

    crate::audit::record(
        &state.db,
        Some(user.user_id),
        crate::audit::actions::IMPERSONATION_START,
        Some("impersonation_session"),
        Some(session_id.to_string()),
        Some(tenant_id),
        Some(serde_json::json!({
            "reason": b.reason,
            "expires_at": expires_at.to_rfc3339(),
        })),
    )
    .await;

    Ok(Json(ImpersonationResp {
        session_id,
        tenant_id,
        reason: b.reason,
        expires_at: expires_at.to_rfc3339(),
        access_token: access,
        token_type: "Bearer",
        expires_in: IMPERSONATION_TTL_SECS,
    }))
}
