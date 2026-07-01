use super::dto::AuditEntry;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use entity::prelude::*;
use rocket::get;
use rocket::serde::json::Json;
use rocket::State;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

/// `GET /admin/audit?limit=&action=` — recent security audit entries, newest
/// first, with the actor's display name resolved.
#[rocket_okapi::openapi(tag = "IAM")]
#[get("/admin/audit?<limit>&<action>")]
pub async fn list_audit(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    limit: Option<u64>,
    action: Option<String>,
) -> ApiResult<Json<Vec<AuditEntry>>> {
    user.require(Permission::AuditRead)?;
    let mut q = AuditLog::find().order_by_desc(entity::audit_log::Column::CreatedAt);
    if let Some(a) = action.filter(|s| !s.is_empty()) {
        q = q.filter(entity::audit_log::Column::Action.eq(a));
    }
    let rows = q.limit(limit.unwrap_or(100).min(500)).all(&db).await?;
    let mut out = Vec::new();
    for r in rows {
        let actor_name = match r.actor_user_id {
            Some(aid) => User::find_by_id(aid).one(&db).await?.map(|u| u.name),
            None => None,
        };
        out.push(AuditEntry {
            id: r.id,
            actor_user_id: r.actor_user_id,
            actor_name,
            action: r.action,
            target_type: r.target_type,
            target_id: r.target_id,
            tenant_id: r.tenant_id,
            metadata: r.metadata,
            principal_kind: r.principal_kind,
            method: r.method,
            path: r.path,
            status_code: r.status_code,
            ip: r.ip,
            duration_ms: r.duration_ms,
            request_id: r.request_id,
            created_at: r.created_at.to_rfc3339(),
        });
    }
    Ok(Json(out))
}
