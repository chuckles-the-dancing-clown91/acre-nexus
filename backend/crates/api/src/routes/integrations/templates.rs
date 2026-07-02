//! Notification **message templates** settings — list the platform catalog
//! merged with the workspace's overrides, edit a template (subject / email
//! body / short SMS text), reset one back to the platform default, or import
//! the whole catalog into the workspace as editable DB copies.
//!
//! Overrides live in `theme.notification_templates` (JSON, keyed by template
//! key) — exactly what the render engine ([`crate::notify`]) already layers
//! over the platform defaults, so an edit takes effect on the next send.

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{Tenant, Theme};
use rocket::serde::json::Json;
use rocket::{delete, get, post, put, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

/// One template as the settings UI sees it: the effective fields (override
/// layered over the platform default) plus where each came from.
#[derive(Serialize, schemars::JsonSchema)]
pub struct TemplateView {
    pub key: String,
    /// Email subject; doubles as the push/in-app title.
    pub subject: String,
    /// Long email body.
    pub body: String,
    /// Short text used for SMS, chat, push, and in-app renditions.
    pub sms: String,
    /// True when the workspace holds its own copy (edited or imported).
    pub customized: bool,
    /// True when a platform default exists for this key (reset restores it;
    /// false = workspace-defined custom key).
    pub has_default: bool,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateTemplateReq {
    pub subject: Option<String>,
    pub body: Option<String>,
    pub sms: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ImportTemplatesResp {
    /// Templates copied into the workspace by this call.
    pub imported: usize,
    /// Size of the platform catalog.
    pub total: usize,
}

/// Merge the workspace's overrides over the platform catalog. Pure — unit
/// tested below and shared by every handler.
fn merged_templates(overrides: &serde_json::Value) -> Vec<TemplateView> {
    let str_of = |v: Option<&serde_json::Value>| -> Option<String> {
        v.and_then(|x| x.as_str()).map(str::to_string)
    };
    let mut out: Vec<TemplateView> = Vec::new();

    for d in crate::notify::default_templates() {
        let over = overrides.get(d.key);
        // A bare-string override is a body-for-every-channel (render() rule):
        // it stands in for both the email body and the short text.
        let plain = over.and_then(|o| o.as_str()).map(str::to_string);
        let field = |name: &str| str_of(over.and_then(|o| o.get(name)));
        out.push(TemplateView {
            key: d.key.to_string(),
            subject: field("subject").unwrap_or_else(|| d.subject.to_string()),
            body: field("body")
                .or_else(|| plain.clone())
                .unwrap_or_else(|| d.body.to_string()),
            sms: field("sms")
                .or_else(|| plain.clone())
                .unwrap_or_else(|| d.sms.to_string()),
            customized: over.is_some(),
            has_default: true,
        });
    }

    // Workspace-defined keys with no platform default.
    if let Some(map) = overrides.as_object() {
        for (key, over) in map {
            if crate::notify::default_templates()
                .iter()
                .any(|d| d.key == key)
            {
                continue;
            }
            let plain = over.as_str().map(str::to_string);
            let field = |name: &str| str_of(over.get(name));
            out.push(TemplateView {
                key: key.clone(),
                subject: field("subject").unwrap_or_default(),
                body: field("body").or_else(|| plain.clone()).unwrap_or_default(),
                sms: field("sms").or_else(|| plain.clone()).unwrap_or_default(),
                customized: true,
                has_default: false,
            });
        }
    }
    out
}

/// The workspace's theme row, created with sensible defaults when the tenant
/// never configured branding (overrides need somewhere to live).
async fn ensure_theme(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
) -> ApiResult<entity::theme::Model> {
    if let Some(t) = Theme::find()
        .filter(entity::theme::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
    {
        return Ok(t);
    }
    let company = Tenant::find_by_id(tenant_id)
        .one(db)
        .await?
        .map(|t| t.name)
        .unwrap_or_else(|| "Acre Nexus".into());
    Ok(entity::theme::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        company_name: Set(company),
        logo_url: Set(None),
        primary_color: Set("#F5451F".into()),
        accent_color: Set("#F5451F".into()),
        default_mode: Set("light".into()),
        legal_templates: Set(json!({})),
        notification_templates: Set(json!({})),
        updated_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await?)
}

fn validate_key(key: &str) -> ApiResult<String> {
    let key = key.trim().to_lowercase();
    if key.is_empty()
        || key.len() > 64
        || !key
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(ApiError::BadRequest(
            "template key must be 1-64 chars of a-z, 0-9, _".into(),
        ));
    }
    Ok(key)
}

/// `GET /integrations/templates` — the full template catalog with the
/// workspace's edits layered in.
#[rocket_okapi::openapi(tag = "Integrations")]
#[get("/integrations/templates")]
pub async fn list_templates(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<TemplateView>>> {
    user.require(Permission::IntegrationsManage)?;
    let overrides = Theme::find()
        .filter(entity::theme::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .map(|t| t.notification_templates)
        .unwrap_or_else(|| json!({}));
    Ok(Json(merged_templates(&overrides)))
}

/// `PUT /integrations/templates/<key>` — set the workspace's copy of a
/// template. Takes effect on the next send.
#[rocket_okapi::openapi(tag = "Integrations")]
#[put("/integrations/templates/<key>", data = "<body>")]
pub async fn update_template(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    key: &str,
    body: Json<UpdateTemplateReq>,
) -> ApiResult<Json<TemplateView>> {
    user.require(Permission::IntegrationsManage)?;
    let key = validate_key(key)?;
    let b = body.into_inner();

    let mut entry = serde_json::Map::new();
    for (name, value) in [("subject", b.subject), ("body", b.body), ("sms", b.sms)] {
        if let Some(v) = value {
            let v = v.trim().to_string();
            if !v.is_empty() {
                entry.insert(name.into(), json!(v));
            }
        }
    }
    if entry.is_empty() {
        return Err(ApiError::BadRequest(
            "provide at least one of subject, body, sms".into(),
        ));
    }

    let theme = ensure_theme(&db, scope.tenant_id).await?;
    let mut overrides = theme.notification_templates.clone();
    if !overrides.is_object() {
        overrides = json!({});
    }
    overrides
        .as_object_mut()
        .expect("normalized above")
        .insert(key.clone(), serde_json::Value::Object(entry));

    let mut am: entity::theme::ActiveModel = theme.into();
    am.notification_templates = Set(overrides.clone());
    am.updated_at = Set(Utc::now().into());
    am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::NOTIFICATION_TEMPLATE_UPDATE,
        Some("notification_template"),
        Some(key.clone()),
        Some(scope.tenant_id),
        None,
    )
    .await;

    let view = merged_templates(&overrides)
        .into_iter()
        .find(|t| t.key == key)
        .ok_or_else(|| ApiError::NotFound("template not found after update".into()))?;
    Ok(Json(view))
}

/// `DELETE /integrations/templates/<key>` — drop the workspace's copy; sends
/// fall back to the platform default (custom keys are removed outright).
#[rocket_okapi::openapi(tag = "Integrations")]
#[delete("/integrations/templates/<key>")]
pub async fn reset_template(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    key: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::IntegrationsManage)?;
    let key = validate_key(key)?;

    let theme = Theme::find()
        .filter(entity::theme::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("no template overrides configured".into()))?;
    let mut overrides = theme.notification_templates.clone();
    let removed = overrides
        .as_object_mut()
        .map(|m| m.remove(&key).is_some())
        .unwrap_or(false);
    if !removed {
        return Err(ApiError::NotFound(format!(
            "template '{key}' has no workspace copy"
        )));
    }

    let mut am: entity::theme::ActiveModel = theme.into();
    am.notification_templates = Set(overrides);
    am.updated_at = Set(Utc::now().into());
    am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::NOTIFICATION_TEMPLATE_RESET,
        Some("notification_template"),
        Some(key.clone()),
        Some(scope.tenant_id),
        None,
    )
    .await;

    Ok(Json(json!({ "reset": true, "key": key })))
}

/// `POST /integrations/templates/import` — copy every platform default the
/// workspace hasn't customized into `theme.notification_templates` as a full,
/// editable DB copy. Existing edits are left untouched.
#[rocket_okapi::openapi(tag = "Integrations")]
#[post("/integrations/templates/import")]
pub async fn import_templates(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<ImportTemplatesResp>> {
    user.require(Permission::IntegrationsManage)?;

    let theme = ensure_theme(&db, scope.tenant_id).await?;
    let mut overrides = theme.notification_templates.clone();
    if !overrides.is_object() {
        overrides = json!({});
    }
    let map = overrides.as_object_mut().expect("normalized above");

    let defaults = crate::notify::default_templates();
    let mut imported = 0;
    for d in defaults {
        if map.contains_key(d.key) {
            continue;
        }
        map.insert(
            d.key.to_string(),
            json!({ "subject": d.subject, "body": d.body, "sms": d.sms }),
        );
        imported += 1;
    }

    if imported > 0 {
        let mut am: entity::theme::ActiveModel = theme.into();
        am.notification_templates = Set(overrides);
        am.updated_at = Set(Utc::now().into());
        am.update(&db).await?;
    }

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::NOTIFICATION_TEMPLATE_IMPORT,
        Some("notification_template"),
        None,
        Some(scope.tenant_id),
        Some(json!({ "imported": imported, "total": defaults.len() })),
    )
    .await;

    Ok(Json(ImportTemplatesResp {
        imported,
        total: defaults.len(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merged_view_layers_overrides_over_defaults() {
        let plain = merged_templates(&json!({}));
        assert!(plain.iter().all(|t| !t.customized && t.has_default));
        let req = plain.iter().find(|t| t.key == "esign_request").unwrap();
        assert!(req.body.contains("{sign_url}"));

        let overrides = json!({
            "esign_request": { "subject": "Please sign, {signer}!" },
            "my_custom": { "body": "Custom body", "sms": "Custom sms" },
            "plain_key": "One body for all channels",
        });
        let merged = merged_templates(&overrides);

        let req = merged.iter().find(|t| t.key == "esign_request").unwrap();
        assert!(req.customized && req.has_default);
        assert_eq!(req.subject, "Please sign, {signer}!");
        // Un-overridden fields still show the platform default.
        assert!(req.body.contains("{sign_url}"));

        let custom = merged.iter().find(|t| t.key == "my_custom").unwrap();
        assert!(custom.customized && !custom.has_default);
        assert_eq!(custom.body, "Custom body");

        // A bare-string override stands in for body + sms (render() rule).
        let plain_key = merged.iter().find(|t| t.key == "plain_key").unwrap();
        assert_eq!(plain_key.body, "One body for all channels");
        assert_eq!(plain_key.sms, "One body for all channels");
    }

    #[test]
    fn template_keys_are_validated() {
        assert_eq!(validate_key(" Esign_Request ").unwrap(), "esign_request");
        assert!(validate_key("").is_err());
        assert!(validate_key("bad key!").is_err());
        assert!(validate_key(&"x".repeat(65)).is_err());
    }
}
