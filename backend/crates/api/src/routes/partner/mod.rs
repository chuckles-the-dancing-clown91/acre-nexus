//! Alpha ↔ Vantedge routes: link a vendor to their Alpha account, send a work
//! order, and see where it stands (the logic lives in [`crate::partner`]).

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::partner::{self, DispatchSpec, PartnerLink};
use crate::rbac::Permission;
use crate::routes::maintenance::dto::TicketDto;
use crate::routes::team::parse_id;
use crate::tenancy::TenantScope;
use entity::prelude::Counterparty;
use rocket::serde::json::Json;
use rocket::{delete, get, post};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, schemars::JsonSchema)]
pub struct LinkReq {
    /// The vendor's Alpha server, e.g. `https://api.alphapowerwash.com`.
    pub base_url: String,
    /// An Alpha API key with `write:jobs` (made in their admin).
    pub api_key: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct LinkResp {
    #[serde(flatten)]
    pub link: PartnerLink,
    /// Shown once, when the callback secret is first created: the vendor
    /// pastes it (with `callback_url`) into their Alpha API client.
    pub callback_secret: Option<String>,
}

/// `GET /entities/<id>/partner` — the vendor's link status and our callback.
#[rocket_okapi::openapi(tag = "Partners")]
#[get("/entities/<id>/partner")]
pub async fn get_link(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<PartnerLink>> {
    user.require(Permission::EntityRead)?;
    let c = partner::find_counterparty(&db, scope.tenant_id, parse_id(id, "vendor")?).await?;
    Ok(Json(partner::link_status(&db, scope.tenant_id, &c).await?))
}

/// `POST /entities/<id>/partner/link` — link a vendor to their Alpha account.
#[rocket_okapi::openapi(tag = "Partners")]
#[post("/entities/<id>/partner/link", data = "<body>")]
pub async fn link(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<LinkReq>,
) -> ApiResult<Json<LinkResp>> {
    user.require(Permission::EntityManage)?;
    let (link, secret) = partner::link(
        &db,
        scope.tenant_id,
        parse_id(id, "vendor")?,
        &body.base_url,
        &body.api_key,
        Some(user.user_id),
    )
    .await?;
    Ok(Json(LinkResp {
        link,
        callback_secret: secret,
    }))
}

/// `DELETE /entities/<id>/partner/link`.
#[rocket_okapi::openapi(tag = "Partners")]
#[delete("/entities/<id>/partner/link")]
pub async fn unlink(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<PartnerLink>> {
    user.require(Permission::EntityManage)?;
    Ok(Json(
        partner::unlink(
            &db,
            scope.tenant_id,
            parse_id(id, "vendor")?,
            Some(user.user_id),
        )
        .await?,
    ))
}

/// `POST /entities/<id>/partner/secret` — rotate the callback secret (shown once).
#[rocket_okapi::openapi(tag = "Partners")]
#[post("/entities/<id>/partner/secret")]
pub async fn rotate_secret(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<LinkResp>> {
    user.require(Permission::EntityManage)?;
    let c = partner::find_counterparty(&db, scope.tenant_id, parse_id(id, "vendor")?).await?;
    let key = crate::providers::webhook::secret_key_name(partner::PROVIDER);
    let secret = format!("whsec_{}", Uuid::new_v4().simple());
    crate::secrets::store(
        &db,
        Some(scope.tenant_id),
        &key,
        &secret,
        Some(user.user_id),
    )
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!(e.to_string())))?;
    Ok(Json(LinkResp {
        link: partner::link_status(&db, scope.tenant_id, &c).await?,
        callback_secret: Some(secret),
    }))
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct LinkedVendor {
    pub id: Uuid,
    pub name: String,
    pub kind: String,
    pub partner_kind: String,
    pub status: Option<String>,
}

/// `GET /partner/vendors` — vendors you can send work to.
#[rocket_okapi::openapi(tag = "Partners")]
#[get("/partner/vendors")]
pub async fn linked_vendors(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<LinkedVendor>>> {
    user.require(Permission::MaintenanceRead)?;
    let rows = Counterparty::find()
        .filter(entity::counterparty::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::counterparty::Column::PartnerKind.is_not_null())
        .order_by_asc(entity::counterparty::Column::Name)
        .all(&db)
        .await?;
    Ok(Json(
        rows.into_iter()
            .map(|c| LinkedVendor {
                id: c.id,
                name: c.name,
                kind: c.kind,
                partner_kind: c.partner_kind.unwrap_or_default(),
                status: c.partner_status,
            })
            .collect(),
    ))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct DispatchReq {
    pub counterparty_id: Uuid,
    pub requested_for: Option<String>,
    pub service_key: Option<String>,
    pub note: Option<String>,
}

/// `POST /tickets/<id>/dispatch` — send the work order to a linked vendor.
#[rocket_okapi::openapi(tag = "Partners")]
#[post("/tickets/<id>/dispatch", data = "<body>")]
pub async fn dispatch(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<DispatchReq>,
) -> ApiResult<Json<TicketDto>> {
    user.require(Permission::MaintenanceManage)?;
    let b = body.into_inner();
    let t = partner::dispatch(
        &db,
        scope.tenant_id,
        DispatchSpec {
            ticket_id: parse_id(id, "work order")?,
            counterparty_id: b.counterparty_id,
            requested_for: b.requested_for,
            service_key: b.service_key,
            note: b.note,
        },
        Some(user.user_id),
    )
    .await?;
    Ok(Json(TicketDto::from(t)))
}
