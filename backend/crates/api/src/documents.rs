//! **Document generation** orchestration: merge an LLC template with a render
//! context + the LLC's branding, rasterise it to a PDF, store the bytes in the
//! tenant's object store, and record a `generated_document` row.
//!
//! This is the shared engine behind "generate a lease contract" and "generate a
//! tenant letter": the only difference is the template body and the context that
//! feeds it.

use crate::state::AppState;
use crate::{pdf, storage, templating};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::{Map, Value};
use uuid::Uuid;

/// Everything needed to render one document.
pub struct RenderInput {
    pub tenant_id: Uuid,
    pub llc: entity::llc::Model,
    pub branding: Option<entity::llc_branding::Model>,
    pub template_id: Option<Uuid>,
    /// `lease` | `letter`.
    pub kind: String,
    pub title: String,
    pub lease_id: Option<Uuid>,
    /// Raw Handlebars template source for the body.
    pub body_template: String,
    /// Merge context for the body + branding fields.
    pub context: Value,
    pub rendered_by: Option<Uuid>,
}

/// A freshly generated document: the persisted row plus the merged body text
/// (reusable as an email body so it isn't rendered twice).
pub struct Generated {
    pub doc: entity::generated_document::Model,
    pub body: String,
}

/// Render, store, and record a document; returns the row + merged body text.
pub async fn generate(state: &AppState, input: RenderInput) -> anyhow::Result<Generated> {
    let store = storage::resolve_for_tenant(state, input.tenant_id).await?;

    // Logo (best-effort — never block document generation on a missing/bad logo).
    let logo = match input.branding.as_ref().and_then(|b| b.logo_document_id) {
        Some(doc_id) => fetch_doc_bytes(state, &store, input.tenant_id, doc_id)
            .await
            .ok(),
        None => None,
    };

    let body = templating::render(&input.body_template, &input.context)?;
    let signature_block = render_opt(
        input.branding.as_ref().and_then(|b| b.signature_block.clone()),
        &input.context,
    )?;
    let letterhead = render_opt(
        input.branding.as_ref().and_then(|b| b.letterhead.clone()),
        &input.context,
    )?;
    let footer = render_opt(
        input.branding.as_ref().and_then(|b| b.footer.clone()),
        &input.context,
    )?;

    let bytes = pdf::render(&pdf::DocSpec {
        title: input.title.clone(),
        logo,
        letterhead,
        body: body.clone(),
        signature_block,
        footer,
    })?;
    let size = bytes.len() as i64;

    let rel = format!(
        "tenants/{}/llc/{}/generated/{}.pdf",
        input.tenant_id,
        input.llc.id,
        Uuid::new_v4()
    );
    let key = store.object_key(&rel);
    store.put(&key, bytes).await?;

    let now = chrono::Utc::now();
    let row = entity::generated_document::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(input.tenant_id),
        llc_id: Set(input.llc.id),
        template_id: Set(input.template_id),
        lease_id: Set(input.lease_id),
        kind: Set(input.kind),
        title: Set(input.title),
        storage_provider: Set(store.provider_label.clone()),
        storage_key: Set(key),
        mime_type: Set("application/pdf".into()),
        size_bytes: Set(size),
        status: Set("final".into()),
        rendered_by: Set(input.rendered_by),
        created_at: Set(now.into()),
    };
    let doc = row.insert(&state.property_db).await?;
    Ok(Generated { doc, body })
}

/// Load a stored document's bytes (tenant-checked) via the resolved store.
async fn fetch_doc_bytes(
    state: &AppState,
    store: &storage::ResolvedStore,
    tenant_id: Uuid,
    doc_id: Uuid,
) -> anyhow::Result<Vec<u8>> {
    let doc = entity::prelude::LlcDocument::find_by_id(doc_id)
        .filter(entity::llc_document::Column::TenantId.eq(tenant_id))
        .one(&state.property_db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("document not found"))?;
    store.get(&doc.storage_key).await
}

fn render_opt(template: Option<String>, ctx: &Value) -> anyhow::Result<Option<String>> {
    match template {
        Some(s) if !s.trim().is_empty() => Ok(Some(templating::render(&s, ctx)?)),
        _ => Ok(None),
    }
}

/// The branding/identity fields every document context starts from.
pub fn base_context(
    llc: &entity::llc::Model,
    branding: Option<&entity::llc_branding::Model>,
) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("llc_name".into(), Value::String(llc.name.clone()));
    m.insert("llc_ein".into(), Value::String(llc.ein.clone()));
    m.insert("llc_state".into(), Value::String(llc.state.clone()));
    m.insert("entity_type".into(), Value::String(llc.entity_type.clone()));
    m.insert(
        "llc_address".into(),
        Value::String(llc.principal_address.clone().unwrap_or_default()),
    );
    m.insert("today".into(), Value::String(today()));
    if let Some(b) = branding {
        m.insert(
            "signature_name".into(),
            Value::String(b.signature_name.clone().unwrap_or_default()),
        );
        m.insert(
            "signature_title".into(),
            Value::String(b.signature_title.clone().unwrap_or_default()),
        );
    }
    m
}

/// Today's date, long form (e.g. "June 28, 2026").
pub fn today() -> String {
    chrono::Utc::now().format("%B %d, %Y").to_string()
}

/// Format integer cents as `$1,234.56`.
pub fn fmt_money(cents: i64) -> String {
    let neg = cents < 0;
    let abs = cents.unsigned_abs();
    let dollars = abs / 100;
    let rem = abs % 100;
    format!(
        "{}${}.{:02}",
        if neg { "-" } else { "" },
        group_thousands(dollars),
        rem
    )
}

fn group_thousands(mut n: u64) -> String {
    if n == 0 {
        return "0".into();
    }
    let mut parts = Vec::new();
    while n > 0 {
        parts.push(format!("{:03}", n % 1000));
        n /= 1000;
    }
    parts.reverse();
    // Trim leading zeros on the most-significant group.
    let mut s = parts.join(",");
    while s.starts_with('0') && s.len() > 1 && s.as_bytes()[1] != b',' {
        s.remove(0);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_formats_with_separators() {
        assert_eq!(fmt_money(0), "$0.00");
        assert_eq!(fmt_money(199_900), "$1,999.00");
        assert_eq!(fmt_money(1_234_567_89), "$1,234,567.89");
        assert_eq!(fmt_money(5), "$0.05");
    }

    #[test]
    fn template_merges_context() {
        let ctx = serde_json::json!({ "tenant_name": "Sam Lee", "rent": "$1,500.00" });
        let out = crate::templating::render(
            "Dear {{tenant_name}}, rent is {{rent}}/mo. {{missing}}",
            &ctx,
        )
        .unwrap();
        assert_eq!(out, "Dear Sam Lee, rent is $1,500.00/mo. ");
    }
}
