//! `GET /integrations/inbound-emails` — the inbound half of the communication
//! history (issue #62): every message received at the tenant's inbound
//! addresses, with where it was routed.

use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::InboundEmail;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct InboundEmailDto {
    pub id: Uuid,
    pub from_email: String,
    pub to_email: String,
    pub subject: String,
    pub body_text: String,
    /// `ticket_comment` | `lead` | `unmatched`.
    pub routed: String,
    pub routed_id: Option<Uuid>,
    pub created_at: String,
}

/// `GET /integrations/inbound-emails` — inbound comms log, newest first.
#[rocket_okapi::openapi(tag = "Integrations")]
#[get("/integrations/inbound-emails")]
pub async fn list_inbound_emails(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<InboundEmailDto>>> {
    user.require(Permission::IntegrationsManage)?;
    let rows = InboundEmail::find()
        .filter(entity::inbound_email::Column::TenantId.eq(scope.tenant_id))
        .order_by_desc(entity::inbound_email::Column::CreatedAt)
        .limit(200)
        .all(&db)
        .await?;
    Ok(Json(
        rows.into_iter()
            .map(|m| InboundEmailDto {
                id: m.id,
                from_email: m.from_email,
                to_email: m.to_email,
                subject: m.subject,
                body_text: m.body_text,
                routed: m.routed,
                routed_id: m.routed_id,
                created_at: m.created_at.to_rfc3339(),
            })
            .collect(),
    ))
}
