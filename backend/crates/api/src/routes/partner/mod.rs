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
    /// When the insurance rule is on and this vendor has no current liability
    /// cover, the reason to send them anyway (audited).
    pub coi_override_reason: Option<String>,
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
    let ticket_id = parse_id(id, "work order")?;
    crate::vendor_compliance::check_dispatch(
        &db,
        scope.tenant_id,
        b.counterparty_id,
        b.coi_override_reason.as_deref(),
        Some(user.user_id),
        ticket_id,
    )
    .await?;
    let t = partner::dispatch(
        &db,
        scope.tenant_id,
        DispatchSpec {
            ticket_id,
            counterparty_id: b.counterparty_id,
            requested_for: b.requested_for,
            service_key: b.service_key,
            note: b.note,
            title: None,
        },
        Some(user.user_id),
    )
    .await?;
    Ok(Json(TicketDto::from(t)))
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct AlphaInviteResp {
    pub alpha_invited_at: String,
    /// The sign-up link they were sent, prefilled with their details.
    pub join_url: String,
}

/// `POST /entities/<id>/alpha-invite` — invite a vendor to sign up for Alpha.
/// Once they do and link, work orders land on their own board instead of in
/// their inbox. The link is the `partners.alpha_join_url` setting with the
/// vendor's details on it, so the form is prefilled.
#[rocket_okapi::openapi(tag = "Partners")]
#[post("/entities/<id>/alpha-invite")]
pub async fn alpha_invite(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<AlphaInviteResp>> {
    user.require(Permission::EntityManage)?;
    let c = partner::find_counterparty(&db, scope.tenant_id, parse_id(id, "vendor")?).await?;
    if c.partner_kind.is_some() {
        return Err(ApiError::Conflict(format!(
            "{} is already linked to Alpha",
            c.name
        )));
    }
    let Some(email) = c.email.as_deref().map(str::trim).filter(|e| !e.is_empty()) else {
        return Err(ApiError::BadRequest(format!(
            "{} has no email on file; add one first",
            c.name
        )));
    };
    let company = entity::prelude::Tenant::find_by_id(scope.tenant_id)
        .one(&db)
        .await?
        .map(|t| t.name)
        .unwrap_or_default();
    let base = crate::settings::get_string(
        &db,
        scope.tenant_id,
        crate::settings::PARTNERS_ALPHA_JOIN_URL,
    )
    .await;
    let join_url = alpha_join_url(&base, &c, &company);
    let now = chrono::Utc::now();
    crate::scheduler::enqueue(
        &db,
        scope.tenant_id,
        "auto_email",
        serde_json::json!({
            "template": "alpha_invite",
            "to": email,
            "owner_type": "counterparty",
            "owner_id": c.id,
            "trigger": format!("alpha_invite:{}:{}", c.id, now.timestamp()),
            "vars": { "join_url": join_url, "recipient": c.contact_name.clone().unwrap_or_else(|| c.name.clone()) },
        }),
        0,
    )
    .await?;
    let mut am: entity::counterparty::ActiveModel = c.into();
    am.alpha_invited_at = sea_orm::Set(Some(now.into()));
    am.updated_at = sea_orm::Set(now.into());
    sea_orm::ActiveModelTrait::update(am, &db).await?;
    Ok(Json(AlphaInviteResp {
        alpha_invited_at: now.to_rfc3339(),
        join_url,
    }))
}

/// The sign-up link with the vendor's details on the query string.
pub fn alpha_join_url(base: &str, c: &entity::counterparty::Model, company: &str) -> String {
    let base = base.trim().trim_end_matches('/');
    let base = if base.is_empty() {
        "https://alphapowerwash.com/partners/join"
    } else {
        base
    };
    let mut pairs = vec![("business", c.name.as_str())];
    if let Some(n) = c.contact_name.as_deref().filter(|n| !n.trim().is_empty()) {
        pairs.push(("name", n));
    }
    if let Some(e) = c.email.as_deref().filter(|e| !e.trim().is_empty()) {
        pairs.push(("email", e));
    }
    if let Some(p) = c.phone.as_deref().filter(|p| !p.trim().is_empty()) {
        pairs.push(("phone", p));
    }
    pairs.push(("from", company));
    let q: Vec<String> = pairs
        .iter()
        .map(|(k, v)| format!("{k}={}", crate::google_places::urlencode(v)))
        .collect();
    let sep = if base.contains('?') { '&' } else { '?' };
    format!("{base}{sep}{}", q.join("&"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn join_url_prefills_the_vendor() {
        let c = entity::counterparty::Model {
            id: uuid::Uuid::nil(),
            tenant_id: uuid::Uuid::nil(),
            kind: "contractor".into(),
            name: "Ace & Sons".into(),
            contact_name: Some("Ray".into()),
            email: Some("ray@ace.example".into()),
            phone: None,
            website: None,
            address: None,
            notes: None,
            partner_kind: None,
            trades: serde_json::json!([]),
            partner_base_url: None,
            partner_web_url: None,
            partner_linked_at: None,
            partner_status: None,
            partner_error: None,
            alpha_invited_at: None,
            related_llc_id: None,
            related_owner_id: None,
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        };
        let u = super::alpha_join_url("", &c, "Northwind");
        assert!(u.starts_with("https://alphapowerwash.com/partners/join?business=Ace"));
        assert!(u.contains("email=ray%40ace.example"));
        assert!(u.contains("from=Northwind"));
        let u2 = super::alpha_join_url("https://x.test/join?src=v", &c, "N");
        assert!(u2.starts_with("https://x.test/join?src=v&business="));
    }
}
