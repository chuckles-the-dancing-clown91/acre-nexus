//! `GET /platform/impersonations` + `DELETE /platform/impersonations/<id>` —
//! list and revoke audited impersonation sessions (§3.3). Listing is gated by
//! `platform:admin`; revocation is immediate and audit-logged.

use super::dto::ImpersonationSummary;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use chrono::Utc;
use entity::prelude::{ImpersonationSession, Tenant};
use rocket::serde::json::Json;
use rocket::{delete, get, State};
use sea_orm::{ActiveModelTrait, EntityTrait, QueryOrder, Set};
use std::collections::HashMap;
use uuid::Uuid;

/// `GET /platform/impersonations` — every impersonation session, newest first.
#[rocket_okapi::openapi(tag = "Platform Admin")]
#[get("/platform/impersonations")]
pub async fn list_impersonations(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
) -> ApiResult<Json<Vec<ImpersonationSummary>>> {
    user.require(Permission::PlatformAdmin)?;
    let rows = ImpersonationSession::find()
        .order_by_desc(entity::impersonation_session::Column::CreatedAt)
        .all(&db)
        .await?;

    // Resolve tenant names in one pass.
    let mut names: HashMap<Uuid, String> = HashMap::new();
    for t in Tenant::find().all(&db).await? {
        names.insert(t.id, t.name);
    }

    let now = Utc::now();
    let out = rows
        .into_iter()
        .map(|s| {
            let active = s.revoked_at.is_none() && s.expires_at > now;
            ImpersonationSummary {
                id: s.id,
                platform_staff_id: s.platform_staff_id,
                tenant_id: s.tenant_id,
                tenant_name: names.get(&s.tenant_id).cloned(),
                reason: s.reason,
                expires_at: s.expires_at.to_rfc3339(),
                revoked_at: s.revoked_at.map(|d| d.to_rfc3339()),
                active,
                created_at: s.created_at.to_rfc3339(),
            }
        })
        .collect();
    Ok(Json(out))
}

/// `DELETE /platform/impersonations/<id>` — revoke a session immediately.
#[rocket_okapi::openapi(tag = "Platform Admin")]
#[delete("/platform/impersonations/<id>")]
pub async fn revoke_impersonation(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::ImpersonateTenant)?;
    let sid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid session id".into()))?;
    let session = ImpersonationSession::find_by_id(sid)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("session not found".into()))?;

    let tenant_id = session.tenant_id;
    let mut am: entity::impersonation_session::ActiveModel = session.into();
    am.revoked_at = Set(Some(Utc::now().into()));
    am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::IMPERSONATION_REVOKE,
        Some("impersonation_session"),
        Some(sid.to_string()),
        Some(tenant_id),
        None,
    )
    .await;
    Ok(Json(serde_json::json!({ "revoked": true })))
}
