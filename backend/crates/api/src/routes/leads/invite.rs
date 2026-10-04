//! `POST /leads/<id>/invite` — send a prospect the application, by email and
//! text, with a link to the public form that already knows who they are.
//! Made for the landlord on a phone at the end of a showing.

use super::dto::LeadDto;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{Lead, Tenant};
use rocket::post;
use rocket::serde::json::Json;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

#[derive(Deserialize, schemars::JsonSchema)]
pub struct InviteReq {
    /// The listing they'd be applying for, if known.
    pub listing_id: Option<Uuid>,
    /// A line from you, on the email.
    pub message: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct InviteResp {
    pub lead: LeadDto,
    /// The link they were sent, so you can also text or show it yourself.
    pub apply_url: String,
}

/// The public application link for a lead: the form opens with their name
/// and email filled in, and the application links back to the lead.
pub fn apply_url(slug: &str, lead: &entity::lead::Model, listing_id: Option<Uuid>) -> String {
    let enc = crate::google_places::urlencode;
    let mut url = format!(
        "{}/apply?tenant={}&lead={}&name={}&email={}",
        crate::oauth::public_app_url(),
        enc(slug),
        lead.id,
        enc(&lead.name),
        enc(&lead.email)
    );
    if let Some(p) = lead.phone.as_deref().filter(|p| !p.trim().is_empty()) {
        url.push_str(&format!("&phone={}", enc(p)));
    }
    if let Some(l) = listing_id {
        url.push_str(&format!("&listing={l}"));
    }
    url
}

/// `POST /leads/<id>/invite` — email (and text) the prospect a link to apply.
#[rocket_okapi::openapi(tag = "Leads")]
#[post("/leads/<id>/invite", data = "<body>")]
pub async fn invite(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<InviteReq>,
) -> ApiResult<Json<InviteResp>> {
    user.require(Permission::ApplicationWrite)?;
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let lead = Lead::find_by_id(lid)
        .filter(entity::lead::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("lead not found".into()))?;
    if lead.application_id.is_some() {
        return Err(ApiError::Conflict("they've already applied".into()));
    }
    if lead.email.trim().is_empty() && lead.phone.as_deref().unwrap_or("").trim().is_empty() {
        return Err(ApiError::BadRequest(
            "the lead has no email or phone to send to".into(),
        ));
    }
    let b = body.into_inner();
    let slug = Tenant::find_by_id(scope.tenant_id)
        .one(&db)
        .await?
        .map(|t| t.slug)
        .unwrap_or_default();
    let url = apply_url(&slug, &lead, b.listing_id);
    let message = b
        .message
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty());
    let now = Utc::now();
    let vars = json!({
        "apply_url": url,
        "message": message.map(|m| format!("\n\n\"{m}\"")).unwrap_or_default(),
    });
    let trigger = format!("application_invite:{}:{}", lead.id, now.timestamp());
    if !lead.email.trim().is_empty() {
        crate::notify::notify_person(
            &db,
            scope.tenant_id,
            &lead.email,
            "application_invite",
            vars.clone(),
            Some(("lead", lead.id)),
            &trigger,
        )
        .await;
    }
    if let Some(phone) = lead.phone.as_deref().filter(|p| !p.trim().is_empty()) {
        crate::scheduler::enqueue(
            &db,
            scope.tenant_id,
            "auto_sms",
            json!({
                "template": "application_invite",
                "to": phone,
                "owner_type": "lead",
                "owner_id": lead.id,
                "trigger": format!("{trigger}:sms"),
                "vars": vars,
            }),
            0,
        )
        .await?;
    }
    let mut am: entity::lead::ActiveModel = lead.clone().into();
    if lead.status == "new" {
        am.status = Set("contacted".into());
    }
    am.updated_at = Set(now.into());
    let saved = am.update(&db).await?;
    Ok(Json(InviteResp {
        lead: saved.into(),
        apply_url: url,
    }))
}
