//! LLC document-template endpoints: list, create, update, delete, and a live
//! **preview** that merges a (possibly unsaved) template body with the LLC's
//! branding context — so the editor can show the result before saving.

use super::dto::{CreateTemplateReq, PreviewReq, PreviewResp, TemplateDto, UpdateTemplateReq};
use super::helpers::{parse_uuid, require_llc};
use crate::auth::AuthUser;
use crate::documents;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use crate::templating;
use chrono::Utc;
use entity::prelude::{LlcBranding, LlcTemplate};
use rocket::serde::json::Json;
use rocket::{delete, get, patch, post, State};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, ModelTrait, QueryFilter, QueryOrder, Set,
};
use serde_json::Value;
use uuid::Uuid;

/// `GET /llcs/<id>/templates` — list an LLC's templates.
#[rocket_okapi::openapi(tag = "LLCs")]
#[get("/llcs/<id>/templates")]
pub async fn list_templates(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Vec<TemplateDto>>> {
    user.require(Permission::LlcRead)?;
    let llc_id = parse_uuid(id)?;
    require_llc(state, scope.tenant_id, llc_id).await?;
    let rows = LlcTemplate::find()
        .filter(entity::llc_template::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::llc_template::Column::LlcId.eq(llc_id))
        .order_by_asc(entity::llc_template::Column::Name)
        .all(&state.property_db)
        .await?;
    Ok(Json(rows.into_iter().map(TemplateDto::from).collect()))
}

/// `POST /llcs/<id>/templates` — create a template.
#[rocket_okapi::openapi(tag = "LLCs")]
#[post("/llcs/<id>/templates", data = "<body>")]
pub async fn create_template(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<CreateTemplateReq>,
) -> ApiResult<Json<TemplateDto>> {
    user.require(Permission::LlcManage)?;
    let llc_id = parse_uuid(id)?;
    require_llc(state, scope.tenant_id, llc_id).await?;
    let b = body.into_inner();
    let now = Utc::now();
    let saved = entity::llc_template::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        llc_id: Set(llc_id),
        kind: Set(b.kind),
        name: Set(b.name),
        subject: Set(b.subject),
        body: Set(b.body),
        is_default: Set(b.is_default.unwrap_or(false)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&state.property_db)
    .await?;
    crate::audit::record(
        &state.user_db,
        Some(user.user_id),
        crate::audit::actions::LLC_TEMPLATE_CREATE,
        Some("llc_template"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "llc_id": llc_id, "kind": saved.kind })),
    )
    .await;
    Ok(Json(TemplateDto::from(saved)))
}

/// `PATCH /llcs/<id>/templates/<tid>` — update a template.
#[rocket_okapi::openapi(tag = "LLCs")]
#[patch("/llcs/<id>/templates/<tid>", data = "<body>")]
pub async fn update_template(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    tid: &str,
    body: Json<UpdateTemplateReq>,
) -> ApiResult<Json<TemplateDto>> {
    user.require(Permission::LlcManage)?;
    let llc_id = parse_uuid(id)?;
    let template_id = parse_uuid(tid)?;
    let existing = LlcTemplate::find_by_id(template_id)
        .filter(entity::llc_template::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::llc_template::Column::LlcId.eq(llc_id))
        .one(&state.property_db)
        .await?
        .ok_or_else(|| ApiError::NotFound("template not found".into()))?;
    let b = body.into_inner();
    let mut am: entity::llc_template::ActiveModel = existing.into();
    if let Some(v) = b.kind {
        am.kind = Set(v);
    }
    if let Some(v) = b.name {
        am.name = Set(v);
    }
    if let Some(v) = b.subject {
        am.subject = Set(Some(v));
    }
    if let Some(v) = b.body {
        am.body = Set(v);
    }
    if let Some(v) = b.is_default {
        am.is_default = Set(v);
    }
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&state.property_db).await?;
    crate::audit::record(
        &state.user_db,
        Some(user.user_id),
        crate::audit::actions::LLC_TEMPLATE_UPDATE,
        Some("llc_template"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        None,
    )
    .await;
    Ok(Json(TemplateDto::from(saved)))
}

/// `DELETE /llcs/<id>/templates/<tid>` — delete a template.
#[rocket_okapi::openapi(tag = "LLCs")]
#[delete("/llcs/<id>/templates/<tid>")]
pub async fn delete_template(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    tid: &str,
) -> ApiResult<Json<Value>> {
    user.require(Permission::LlcManage)?;
    let llc_id = parse_uuid(id)?;
    let template_id = parse_uuid(tid)?;
    let existing = LlcTemplate::find_by_id(template_id)
        .filter(entity::llc_template::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::llc_template::Column::LlcId.eq(llc_id))
        .one(&state.property_db)
        .await?
        .ok_or_else(|| ApiError::NotFound("template not found".into()))?;
    existing.delete(&state.property_db).await?;
    crate::audit::record(
        &state.user_db,
        Some(user.user_id),
        crate::audit::actions::LLC_TEMPLATE_DELETE,
        Some("llc_template"),
        Some(template_id.to_string()),
        Some(scope.tenant_id),
        None,
    )
    .await;
    Ok(Json(serde_json::json!({ "deleted": true })))
}

/// `POST /llcs/<id>/templates/preview` — render a template body against the LLC's
/// branding context plus any sample fields, returning the merged text.
#[rocket_okapi::openapi(tag = "LLCs")]
#[post("/llcs/<id>/templates/preview", data = "<body>")]
pub async fn preview_template(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<PreviewReq>,
) -> ApiResult<Json<PreviewResp>> {
    user.require(Permission::LlcRead)?;
    let llc_id = parse_uuid(id)?;
    let llc = require_llc(state, scope.tenant_id, llc_id).await?;
    let branding = LlcBranding::find()
        .filter(entity::llc_branding::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::llc_branding::Column::LlcId.eq(llc_id))
        .one(&state.property_db)
        .await?;
    let req = body.into_inner();
    let mut ctx = documents::base_context(&llc, branding.as_ref());
    if let Some(Value::Object(extra)) = req.context {
        for (k, v) in extra {
            ctx.insert(k, v);
        }
    }
    let rendered = templating::render(&req.body, &Value::Object(ctx))
        .map_err(|e| ApiError::BadRequest(format!("template error: {e}")))?;
    Ok(Json(PreviewResp { rendered }))
}
