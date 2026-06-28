//! Document **generation** endpoints: render a lease contract or tenant letter
//! from a template + the LLC's branding to a stored PDF, optionally emailing it;
//! list generated documents; and download a generated PDF.

use super::dto::{GenerateReq, GeneratedDocumentDto};
use super::helpers::{parse_uuid, require_llc};
use crate::auth::AuthUser;
use crate::email::{self, OutboundEmail};
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use crate::{documents, storage, templating};
use entity::prelude::{GeneratedDocument, Lease, LlcBranding, LlcTemplate, Property};
use rocket::http::ContentType;
use rocket::serde::json::Json;
use rocket::{get, post, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde_json::Value;
use uuid::Uuid;

/// `POST /llcs/<id>/generate` — render and store a lease/letter PDF, and
/// optionally email it to the recipient.
#[rocket_okapi::openapi(tag = "LLCs")]
#[post("/llcs/<id>/generate", data = "<body>")]
pub async fn generate_document(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<GenerateReq>,
) -> ApiResult<Json<GeneratedDocumentDto>> {
    user.require(Permission::LlcManage)?;
    let llc_id = parse_uuid(id)?;
    let llc = require_llc(state, scope.tenant_id, llc_id).await?;
    let req = body.into_inner();

    let kind = req.kind.clone().unwrap_or_else(|| "letter".into());
    let branding = LlcBranding::find()
        .filter(entity::llc_branding::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::llc_branding::Column::LlcId.eq(llc_id))
        .one(&state.property_db)
        .await?;

    // Resolve the template body + subject: a saved template, else a built-in default.
    let (body_template, subject) = match req.template_id {
        Some(tid) => {
            let t = LlcTemplate::find_by_id(tid)
                .filter(entity::llc_template::Column::TenantId.eq(scope.tenant_id))
                .filter(entity::llc_template::Column::LlcId.eq(llc_id))
                .one(&state.property_db)
                .await?
                .ok_or_else(|| ApiError::NotFound("template not found".into()))?;
            (t.body, t.subject)
        }
        None if kind == "lease" => (templating::DEFAULT_LEASE_TEMPLATE.to_string(), None),
        None => (templating::DEFAULT_LETTER_TEMPLATE.to_string(), None),
    };

    // Build the merge context: branding base + lease/recipient facts + extras.
    let mut ctx = documents::base_context(&llc, branding.as_ref());
    if let Some(rn) = &req.recipient_name {
        ctx.insert("tenant_name".into(), Value::String(rn.clone()));
    }
    if let Some(addr) = &req.property_address {
        ctx.insert("property_address".into(), Value::String(addr.clone()));
    }
    if let Some(lease_id) = req.lease_id {
        merge_lease_context(state, scope.tenant_id, lease_id, &mut ctx).await?;
    }
    if let Some(Value::Object(extra)) = req.context.clone() {
        for (k, v) in extra {
            ctx.insert(k, v);
        }
    }

    let title = req.title.clone().unwrap_or_else(|| match kind.as_str() {
        "lease" => format!("Lease Agreement — {}", llc.name),
        _ => format!("Letter from {}", llc.name),
    });

    let generated = documents::generate(
        state,
        documents::RenderInput {
            tenant_id: scope.tenant_id,
            llc: llc.clone(),
            branding: branding.clone(),
            template_id: req.template_id,
            kind: kind.clone(),
            title: title.clone(),
            lease_id: req.lease_id,
            body_template,
            context: Value::Object(ctx),
            rendered_by: Some(user.user_id),
        },
    )
    .await
    .map_err(ApiError::Internal)?;

    // Optionally email it to the recipient (records a sent_email row either way).
    if req.send_email.unwrap_or(false) {
        if let Some(to) = req.recipient_email.as_deref().filter(|s| !s.trim().is_empty()) {
            let subject = subject.unwrap_or_else(|| title.clone());
            let _ = email::send(
                &state.user_db,
                &state.config.email,
                OutboundEmail {
                    tenant_id: scope.tenant_id,
                    llc_id: Some(llc_id),
                    to: to.to_string(),
                    cc: None,
                    subject,
                    body: generated.body.clone(),
                    template_id: req.template_id,
                    job_id: None,
                    generated_document_id: Some(generated.doc.id),
                },
            )
            .await;
        }
    }

    crate::audit::record(
        &state.user_db,
        Some(user.user_id),
        crate::audit::actions::DOCUMENT_GENERATE,
        Some("generated_document"),
        Some(generated.doc.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "llc_id": llc_id, "kind": kind, "emailed": req.send_email.unwrap_or(false) })),
    )
    .await;

    Ok(Json(GeneratedDocumentDto::from(generated.doc)))
}

/// Pull a lease's facts (and its property's address) into the render context.
async fn merge_lease_context(
    state: &AppState,
    tenant_id: Uuid,
    lease_id: Uuid,
    ctx: &mut serde_json::Map<String, Value>,
) -> ApiResult<()> {
    let lease = Lease::find_by_id(lease_id)
        .filter(entity::lease::Column::TenantId.eq(tenant_id))
        .one(&state.property_db)
        .await?
        .ok_or_else(|| ApiError::NotFound("lease not found".into()))?;
    ctx.insert("tenant_name".into(), Value::String(lease.tenant_name.clone()));
    ctx.insert("rent".into(), Value::String(documents::fmt_money(lease.rent_cents)));
    ctx.insert(
        "deposit".into(),
        Value::String(documents::fmt_money(lease.deposit_cents.unwrap_or(0))),
    );
    ctx.insert("start_date".into(), Value::String(lease.start_date.clone()));
    ctx.insert(
        "end_date".into(),
        Value::String(lease.end_date.clone().unwrap_or_default()),
    );
    if !ctx.contains_key("property_address") {
        if let Ok(Some(p)) = Property::find_by_id(lease.property_id)
            .filter(entity::property::Column::TenantId.eq(tenant_id))
            .one(&state.property_db)
            .await
        {
            ctx.insert(
                "property_address".into(),
                Value::String(format!("{}, {}", p.address, p.city)),
            );
        }
    }
    Ok(())
}

/// `GET /llcs/<id>/generated` — list documents generated for an LLC.
#[rocket_okapi::openapi(tag = "LLCs")]
#[get("/llcs/<id>/generated")]
pub async fn list_generated(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Vec<GeneratedDocumentDto>>> {
    user.require(Permission::LlcRead)?;
    let llc_id = parse_uuid(id)?;
    require_llc(state, scope.tenant_id, llc_id).await?;
    let rows = GeneratedDocument::find()
        .filter(entity::generated_document::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::generated_document::Column::LlcId.eq(llc_id))
        .order_by_desc(entity::generated_document::Column::CreatedAt)
        .all(&state.property_db)
        .await?;
    Ok(Json(rows.into_iter().map(GeneratedDocumentDto::from).collect()))
}

/// `GET /generated-documents/<gid>/download` — download a generated PDF.
#[get("/generated-documents/<gid>/download")]
pub async fn download_generated(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    gid: &str,
) -> ApiResult<(ContentType, Vec<u8>)> {
    user.require(Permission::LlcRead)?;
    let id = parse_uuid(gid)?;
    let doc = GeneratedDocument::find_by_id(id)
        .filter(entity::generated_document::Column::TenantId.eq(scope.tenant_id))
        .one(&state.property_db)
        .await?
        .ok_or_else(|| ApiError::NotFound("document not found".into()))?;
    let store = storage::resolve_for_tenant(state, scope.tenant_id).await?;
    let bytes = store.get(&doc.storage_key).await?;
    let ct = ContentType::parse_flexible(&doc.mime_type).unwrap_or(ContentType::PDF);
    Ok((ct, bytes))
}
