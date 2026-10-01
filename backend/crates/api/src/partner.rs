//! **Alpha ↔ Vantedge** (Vantedge phase 2C): sending a work order to a vendor
//! who runs Alpha Power Wash, and hearing back.
//!
//! No shared database. Each side keeps its own plumbing:
//!
//! * **Link.** A vendor (a counterparty) is linked to their Alpha account with
//!   the base URL and one of Alpha's API keys (`write:jobs`). The key goes in
//!   the vault under `partner.<counterparty_id>.api_key`; linking pings Alpha
//!   (`GET /integrations/ping`) to prove the key works.
//! * **Dispatch.** `POST /tickets/<id>/dispatch` queues a `partner_dispatch`
//!   job that POSTs the work order to Alpha's `POST /integrations/jobs` with
//!   the property, access notes, the requested date and our callback. Alpha's
//!   job id lands on the ticket; the vendor becomes its assignee.
//! * **Callback.** Alpha POSTs every change (`job.scheduled`, `job.started`,
//!   `job.completed`, `job.photo`…) to `POST /webhooks/alpha?tenant=<slug>`,
//!   signed HMAC-SHA256 with `webhook.alpha.secret` (generated on first link,
//!   given to the vendor to paste into their Alpha API client). The inbound
//!   webhook framework verifies it and hands the event here: the ticket's
//!   status follows the job, the timeline gets the vendor's notes and photos,
//!   and a completed job's price becomes a labor line on the work order — so
//!   the owner bill and costing see what the vendor charged.
//!
//! Sandbox-first like every provider: without `LIVE_PROVIDERS=alpha` the
//! calls are simulated (a ping succeeds, a dispatch returns job `4242`).

use crate::error::{ApiError, ApiResult};
use crate::modules::JobOutcome;
use crate::providers::{err, Provider, ProviderCtx, ProviderError};
use chrono::Utc;
use entity::prelude::{Counterparty, MaintenanceTicket, Property, Tenant};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    Set,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

/// Inbound webhook provider key: Alpha posts to `/webhooks/alpha`.
pub const PROVIDER: &str = "alpha";
/// The background job that sends one work order to Alpha.
pub const DISPATCH_KIND: &str = "partner_dispatch";
pub const KIND_ALPHA: &str = "alpha";

/// The vault key holding a linked vendor's Alpha API key.
pub fn api_key_ref(counterparty_id: Uuid) -> String {
    format!("partner.{counterparty_id}.api_key")
}

/// Where Alpha reaches this server: `PUBLIC_API_URL` (no trailing slash).
pub fn public_api_url() -> String {
    std::env::var("PUBLIC_API_URL")
        .ok()
        .map(|s| s.trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "http://localhost:8000".into())
}

/// The callback URL a tenant gives its vendors.
pub fn callback_url(slug: &str) -> String {
    format!("{}/webhooks/{PROVIDER}?tenant={slug}", public_api_url())
}

// ---------------------------------------------------------------------------
// Provider: one HTTP call to an Alpha instance
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize)]
pub struct AlphaRequest {
    pub base_url: String,
    /// Vault key of the API key to send as `X-API-Key`.
    pub api_key_ref: String,
    pub method: String,
    /// e.g. `/api/v1/integrations/jobs`
    pub path: String,
    pub body: Option<Value>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AlphaResponse {
    pub status: u16,
    pub body: Value,
}

pub struct AlphaProvider;

#[async_trait::async_trait]
impl Provider for AlphaProvider {
    type Request = AlphaRequest;
    type Response = AlphaResponse;

    fn key(&self) -> &'static str {
        "alpha"
    }

    async fn call<C: ConnectionTrait + Sync>(
        &self,
        ctx: &ProviderCtx<'_, C>,
        req: &Self::Request,
    ) -> Result<Self::Response, ProviderError> {
        let key = ctx
            .secret(&req.api_key_ref)
            .await?
            .ok_or_else(|| err("this vendor's Alpha API key is missing — link them again"))?;
        let http = crate::providers::client::build_http_client()?;
        let url = format!("{}{}", req.base_url.trim_end_matches('/'), req.path);
        let mut r = match req.method.as_str() {
            "POST" => http.post(&url),
            _ => http.get(&url),
        }
        .header("X-API-Key", key)
        .header("Accept", "application/json");
        if let Some(b) = &req.body {
            r = r.json(b);
        }
        let resp = r
            .send()
            .await
            .map_err(|e| err(format!("alpha {} {}: {e}", req.method, req.path)))?;
        let status = resp.status().as_u16();
        let body: Value = resp.json().await.unwrap_or(Value::Null);
        if status == 401 || status == 403 {
            return Err(err(
                "Alpha refused the API key (check the key and its scopes)",
            ));
        }
        if status >= 400 {
            return Err(err(format!(
                "alpha {} {} → {status}: {body}",
                req.method, req.path
            )));
        }
        Ok(AlphaResponse { status, body })
    }

    async fn simulate<C: ConnectionTrait + Sync>(
        &self,
        _ctx: &ProviderCtx<'_, C>,
        req: &Self::Request,
    ) -> Result<Self::Response, ProviderError> {
        if req.base_url.contains("fail") {
            return Err(err("simulated Alpha refused the call"));
        }
        let body = match (req.method.as_str(), req.path.as_str()) {
            ("GET", "/api/v1/integrations/ping") => json!({
                "ok": true, "name": "Vantedge (simulated)", "scopes": ["read:jobs", "write:jobs"], "callback_set": true
            }),
            ("POST", "/api/v1/integrations/jobs") => json!({
                "id": 4242, "status": "unscheduled", "created": true,
                "external_ref": req.body.as_ref().and_then(|b| b.get("external_ref")).cloned().unwrap_or(Value::Null)
            }),
            _ => json!({ "ok": true }),
        };
        Ok(AlphaResponse { status: 200, body })
    }
}

// ---------------------------------------------------------------------------
// Linking a vendor
// ---------------------------------------------------------------------------

#[derive(Serialize, schemars::JsonSchema)]
pub struct PartnerLink {
    pub counterparty_id: Uuid,
    pub linked: bool,
    pub kind: Option<String>,
    pub base_url: Option<String>,
    pub linked_at: Option<String>,
    /// `ok` | `error`
    pub status: Option<String>,
    pub error: Option<String>,
    /// What the vendor pastes into their Alpha API client so we hear back.
    pub callback_url: String,
    /// Whether the signing secret exists (its value shows once, at link time).
    pub callback_secret_set: bool,
}

pub async fn find_counterparty(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    id: Uuid,
) -> ApiResult<entity::counterparty::Model> {
    Counterparty::find_by_id(id)
        .filter(entity::counterparty::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("vendor not found".into()))
}

async fn slug_of(db: &impl ConnectionTrait, tenant_id: Uuid) -> String {
    Tenant::find_by_id(tenant_id)
        .one(db)
        .await
        .ok()
        .flatten()
        .map(|t| t.slug)
        .unwrap_or_default()
}

pub async fn link_status(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    c: &entity::counterparty::Model,
) -> ApiResult<PartnerLink> {
    let secret_set = crate::secrets::reveal(
        db,
        Some(tenant_id),
        &crate::providers::webhook::secret_key_name(PROVIDER),
    )
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!(e.to_string())))?
    .is_some();
    Ok(PartnerLink {
        counterparty_id: c.id,
        linked: c.partner_kind.is_some(),
        kind: c.partner_kind.clone(),
        base_url: c.partner_base_url.clone(),
        linked_at: c.partner_linked_at.map(|d| d.to_rfc3339()),
        status: c.partner_status.clone(),
        error: c.partner_error.clone(),
        callback_url: callback_url(&slug_of(db, tenant_id).await),
        callback_secret_set: secret_set,
    })
}

/// The tenant's Alpha callback secret, created on first use. Returns
/// `(secret, created)`; the value is shown to the user only when created.
pub async fn ensure_callback_secret(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    user: Option<Uuid>,
) -> ApiResult<(String, bool)> {
    let key = crate::providers::webhook::secret_key_name(PROVIDER);
    if let Some(s) = crate::secrets::reveal(db, Some(tenant_id), &key)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!(e.to_string())))?
    {
        return Ok((s, false));
    }
    let secret = format!("whsec_{}", Uuid::new_v4().simple());
    crate::secrets::store(db, Some(tenant_id), &key, &secret, user)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!(e.to_string())))?;
    Ok((secret, true))
}

/// Link a vendor to their Alpha account: store the key, ping Alpha, record the
/// result. Returns the link and, when the callback secret was just created,
/// its value (to paste into Alpha).
pub async fn link(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    counterparty_id: Uuid,
    base_url: &str,
    api_key: &str,
    user: Option<Uuid>,
) -> ApiResult<(PartnerLink, Option<String>)> {
    let c = find_counterparty(db, tenant_id, counterparty_id).await?;
    let base_url = base_url.trim().trim_end_matches('/').to_string();
    if !(base_url.starts_with("https://") || base_url.starts_with("http://")) {
        return Err(ApiError::BadRequest(
            "base_url must start with https://".into(),
        ));
    }
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err(ApiError::BadRequest("paste the Alpha API key".into()));
    }
    crate::secrets::store(db, Some(tenant_id), &api_key_ref(c.id), api_key, user)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!(e.to_string())))?;
    let (secret, created) = ensure_callback_secret(db, tenant_id, user).await?;
    // Prove the key works before we call it linked.
    let ctx = ProviderCtx::new(db, tenant_id);
    let ping = AlphaProvider
        .execute(
            &ctx,
            &AlphaRequest {
                base_url: base_url.clone(),
                api_key_ref: api_key_ref(c.id),
                method: "GET".into(),
                path: "/api/v1/integrations/ping".into(),
                body: None,
            },
        )
        .await;
    let now = Utc::now();
    let mut am: entity::counterparty::ActiveModel = c.into();
    am.partner_kind = Set(Some(KIND_ALPHA.into()));
    am.partner_base_url = Set(Some(base_url));
    am.partner_linked_at = Set(Some(now.into()));
    match &ping {
        Ok(r) => {
            let scopes = r
                .body
                .get("scopes")
                .and_then(|s| s.as_array())
                .cloned()
                .unwrap_or_default();
            let can_write = scopes.iter().any(|s| s == "write:jobs");
            am.partner_status = Set(Some(if can_write { "ok" } else { "error" }.into()));
            am.partner_error = Set(if can_write {
                None
            } else {
                Some(
                    "the key works but lacks the write:jobs scope — add it in Alpha's admin".into(),
                )
            });
        }
        Err(e) => {
            am.partner_status = Set(Some("error".into()));
            am.partner_error = Set(Some(e.to_string()));
        }
    }
    am.updated_at = Set(now.into());
    let c = am.update(db).await?;
    crate::audit::record(
        db,
        user,
        crate::audit::actions::PARTNER_LINK,
        Some("counterparty"),
        Some(c.id.to_string()),
        Some(tenant_id),
        Some(json!({ "kind": KIND_ALPHA, "status": c.partner_status })),
    )
    .await;
    Ok((
        link_status(db, tenant_id, &c).await?,
        created.then_some(secret),
    ))
}

pub async fn unlink(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    counterparty_id: Uuid,
    user: Option<Uuid>,
) -> ApiResult<PartnerLink> {
    let _ = &user;
    let c = find_counterparty(db, tenant_id, counterparty_id).await?;
    let _ = crate::secrets::remove(db, Some(tenant_id), &api_key_ref(c.id)).await;
    let mut am: entity::counterparty::ActiveModel = c.into();
    am.partner_kind = Set(None);
    am.partner_base_url = Set(None);
    am.partner_linked_at = Set(None);
    am.partner_status = Set(None);
    am.partner_error = Set(None);
    am.updated_at = Set(Utc::now().into());
    let c = am.update(db).await?;
    crate::audit::record(
        db,
        user,
        crate::audit::actions::PARTNER_UNLINK,
        Some("counterparty"),
        Some(c.id.to_string()),
        Some(tenant_id),
        None,
    )
    .await;
    link_status(db, tenant_id, &c).await
}

// ---------------------------------------------------------------------------
// Dispatch: send a work order
// ---------------------------------------------------------------------------

#[derive(Deserialize, Serialize, Clone, Default)]
pub struct DispatchSpec {
    pub ticket_id: Uuid,
    pub counterparty_id: Uuid,
    /// ISO date-time the work is wanted, if known.
    pub requested_for: Option<String>,
    /// e.g. `driveway`, `house-wash` — Alpha's price key; else its first service.
    pub service_key: Option<String>,
    pub note: Option<String>,
}

/// Queue the send. The vendor becomes the ticket's assignee right away; the
/// job records Alpha's id when it comes back.
pub async fn dispatch(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    spec: DispatchSpec,
    user: Option<Uuid>,
) -> ApiResult<entity::maintenance_ticket::Model> {
    let c = find_counterparty(db, tenant_id, spec.counterparty_id).await?;
    if c.partner_kind.is_none() {
        return Err(ApiError::Conflict(format!(
            "{} isn't linked to Alpha yet",
            c.name
        )));
    }
    let t = MaintenanceTicket::find_by_id(spec.ticket_id)
        .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("work order not found".into()))?;
    if t.partner_job_id.is_some() || t.partner_status.as_deref() == Some("sending") {
        return Err(ApiError::Conflict(
            "this work order was already sent to a vendor".into(),
        ));
    }
    let now = Utc::now();
    let mut am: entity::maintenance_ticket::ActiveModel = t.into();
    am.assignee_entity_id = Set(Some(c.id));
    am.partner_counterparty_id = Set(Some(c.id));
    am.partner_status = Set(Some("sending".into()));
    am.updated_at = Set(now.into());
    let t = am.update(db).await?;
    crate::scheduler::enqueue(
        db,
        tenant_id,
        DISPATCH_KIND,
        serde_json::to_value(&spec).unwrap_or(json!({})),
        0,
    )
    .await?;
    add_note(
        db,
        tenant_id,
        t.id,
        "status",
        &format!("Sending to {} (Alpha)…", c.name),
        Some(c.name.clone()),
    )
    .await;
    crate::audit::record(
        db,
        user,
        crate::audit::actions::TICKET_DISPATCH,
        Some("maintenance_ticket"),
        Some(t.id.to_string()),
        Some(tenant_id),
        Some(json!({ "vendor_id": c.id })),
    )
    .await;
    crate::webhooks_out::emit(
        db,
        tenant_id,
        "maintenance_ticket.assigned",
        json!({ "ticket_id": t.id, "vendor_id": c.id, "vendor": c.name }),
    )
    .await;
    Ok(t)
}

async fn add_note(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    ticket_id: Uuid,
    kind: &str,
    body: &str,
    author: Option<String>,
) {
    let c = entity::ticket_comment::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        ticket_id: Set(ticket_id),
        author_user_id: Set(None),
        kind: Set(kind.into()),
        visibility: Set("internal".into()),
        author_name: Set(author),
        body: Set(body.to_string()),
        created_at: Set(Utc::now().into()),
    };
    if let Err(e) = c.insert(db).await {
        tracing::error!("partner: could not log note: {e}");
    }
}

/// The work order as Alpha wants it (`PartnerJobIn`).
pub fn job_request(
    t: &entity::maintenance_ticket::Model,
    p: &entity::property::Model,
    tenant_name: &str,
    spec: &DispatchSpec,
) -> Value {
    let mut details = t.description.clone().unwrap_or_default();
    if let Some(n) = spec.note.as_deref().filter(|n| !n.trim().is_empty()) {
        if !details.is_empty() {
            details.push_str("\n\n");
        }
        details.push_str(n.trim());
    }
    json!({
        "external_ref": t.id,
        "title": t.title,
        "details": details,
        "service_key": spec.service_key.clone().unwrap_or_default(),
        "service_title": t.category,
        "requested_for": spec.requested_for.clone().or_else(|| t.due_date.clone().map(|d| format!("{d}T09:00:00"))),
        "client_name": tenant_name,
        "property_label": p.name,
        "address": p.address,
        "city": p.city,
        "state": p.state,
        "postal_code": p.postal_code,
        "access_notes": t.access_notes,
        "site_contact_name": t.reporter,
    })
}

pub async fn handle_dispatch_job(
    db: &DatabaseConnection,
    job: &entity::background_job::Model,
) -> JobOutcome {
    let spec: DispatchSpec = match serde_json::from_value(job.payload.clone()) {
        Ok(s) => s,
        Err(e) => return JobOutcome::failed(format!("bad dispatch payload: {e}")),
    };
    let tenant_id = job.tenant_id;
    let (t, c, p, tenant_name) = match load_dispatch(db, tenant_id, &spec).await {
        Ok(v) => v,
        Err(e) => return JobOutcome::failed(e.to_string()),
    };
    let Some(base_url) = c.partner_base_url.clone() else {
        return JobOutcome::failed("vendor is not linked");
    };
    let ctx = ProviderCtx::new(db, tenant_id);
    let req = AlphaRequest {
        base_url,
        api_key_ref: api_key_ref(c.id),
        method: "POST".into(),
        path: "/api/v1/integrations/jobs".into(),
        body: Some(job_request(&t, &p, &tenant_name, &spec)),
    };
    let resp = match crate::providers::run(&AlphaProvider, &ctx, job, &req).await {
        Ok(r) => r,
        Err(outcome) => {
            // Terminal failure: say so on the ticket.
            if job.attempts + 1 >= job.max_attempts {
                let mut am: entity::maintenance_ticket::ActiveModel = t.clone().into();
                am.partner_status = Set(Some("failed".into()));
                am.updated_at = Set(Utc::now().into());
                let _ = am.update(db).await;
                add_note(
                    db,
                    tenant_id,
                    t.id,
                    "status",
                    &format!("Could not send to {} — check their Alpha link.", c.name),
                    Some(c.name.clone()),
                )
                .await;
                let mut cam: entity::counterparty::ActiveModel = c.into();
                cam.partner_status = Set(Some("error".into()));
                cam.partner_error = Set(Some("last dispatch failed".into()));
                let _ = cam.update(db).await;
            }
            return outcome;
        }
    };
    let job_id = resp
        .body
        .get("id")
        .map(|v| v.to_string().trim_matches('"').to_string())
        .unwrap_or_default();
    let status = resp
        .body
        .get("status")
        .and_then(|s| s.as_str())
        .unwrap_or("sent")
        .to_string();
    let now = Utc::now();
    let mut am: entity::maintenance_ticket::ActiveModel = t.clone().into();
    am.partner_job_id = Set(Some(job_id.clone()));
    am.partner_status = Set(Some(status.clone()));
    am.partner_synced_at = Set(Some(now.into()));
    if t.status == "open" || t.status == "triage" {
        am.status = Set("scheduled".into());
    }
    if t.first_response_at.is_none() {
        am.first_response_at = Set(Some(now.into()));
    }
    am.updated_at = Set(now.into());
    if let Err(e) = am.update(db).await {
        return JobOutcome::retry(crate::providers::backoff(job.attempts), format!("db: {e}"));
    }
    add_note(
        db,
        tenant_id,
        t.id,
        "status",
        &format!("Sent to {} — Alpha job #{job_id} ({status}).", c.name),
        Some(c.name.clone()),
    )
    .await;
    let mut cam: entity::counterparty::ActiveModel = c.into();
    cam.partner_status = Set(Some("ok".into()));
    cam.partner_error = Set(None);
    let _ = cam.update(db).await;
    JobOutcome::completed(json!({ "ticket_id": t.id, "alpha_job_id": job_id, "status": status }))
}

type Loaded = (
    entity::maintenance_ticket::Model,
    entity::counterparty::Model,
    entity::property::Model,
    String,
);

async fn load_dispatch(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    spec: &DispatchSpec,
) -> ApiResult<Loaded> {
    let t = MaintenanceTicket::find_by_id(spec.ticket_id)
        .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("work order not found".into()))?;
    let c = find_counterparty(db, tenant_id, spec.counterparty_id).await?;
    let p = Property::find_by_id(t.property_id)
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    let name = Tenant::find_by_id(tenant_id)
        .one(db)
        .await?
        .map(|t| t.name)
        .unwrap_or_default();
    Ok((t, c, p, name))
}

// ---------------------------------------------------------------------------
// Callback: Alpha tells us what happened
// ---------------------------------------------------------------------------

/// What one Alpha event does to the ticket (pure): the new status, if any,
/// and the timeline note.
pub fn apply_event(event: &str, job: &Value) -> (Option<&'static str>, String) {
    let s = |k: &str| {
        job.get(k)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    let when = s("scheduled_for");
    let crew: Vec<String> = job
        .get("crew")
        .and_then(|c| c.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let photos: Vec<String> = job
        .get("photos")
        .and_then(|c| c.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.get("url").and_then(|u| u.as_str()).map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    match event {
        "job.created" => (None, "Vendor received the work order.".into()),
        "job.scheduled" | "job.rescheduled" => (
            Some("scheduled"),
            format!(
                "Vendor scheduled for {}{}.",
                if when.is_empty() {
                    "a date to be confirmed".to_string()
                } else {
                    when.replace('T', " ")
                },
                if crew.is_empty() {
                    String::new()
                } else {
                    format!(" — crew: {}", crew.join(", "))
                }
            ),
        ),
        "job.started" => (Some("in_progress"), "Vendor is on site.".into()),
        "job.completed" => {
            let price = s("price");
            let mut body = format!(
                "Vendor finished{}.",
                if price.is_empty() {
                    String::new()
                } else {
                    format!(" — ${price}")
                }
            );
            let note = s("report_note");
            if !note.is_empty() {
                body.push_str(&format!("\n{note}"));
            }
            if !photos.is_empty() {
                body.push_str(&format!("\nPhotos:\n{}", photos.join("\n")));
            }
            (Some("resolved"), body)
        }
        "job.cancelled" => (
            Some("open"),
            "Vendor cancelled the job — needs a new vendor or date.".into(),
        ),
        "job.photo" => (
            None,
            match photos.last() {
                Some(u) => format!("Vendor added a photo: {u}"),
                None => "Vendor added a photo.".into(),
            },
        ),
        _ => (None, format!("Vendor update: {}", s("status"))),
    }
}

/// Cents from Alpha's decimal string ("189.50" → 18950).
pub fn cents(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (whole, frac) = s.split_once('.').unwrap_or((s, "0"));
    let whole: i64 = whole.parse().ok()?;
    let frac: i64 = format!("{frac}00")[..2].parse().ok()?;
    Some(whole * 100 + if whole < 0 { -frac } else { frac })
}

pub async fn handle_webhook_event(
    db: &DatabaseConnection,
    job: &entity::background_job::Model,
) -> Option<JobOutcome> {
    let provider = job.payload.get("provider").and_then(|v| v.as_str())?;
    if provider != PROVIDER {
        return None;
    }
    let tenant_id = job.tenant_id;
    let event = job.payload.get("event").cloned().unwrap_or(json!({}));
    let name = event
        .get("event")
        .and_then(|e| e.as_str())
        .unwrap_or("")
        .to_string();
    let aj = event.get("job").cloned().unwrap_or(json!({}));
    let ext = aj
        .get("external_ref")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok());
    let alpha_id = aj
        .get("id")
        .map(|v| v.to_string().trim_matches('"').to_string());
    let mut q = MaintenanceTicket::find()
        .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id));
    q = match (ext, alpha_id.clone()) {
        (Some(id), _) => q.filter(entity::maintenance_ticket::Column::Id.eq(id)),
        (None, Some(a)) => q.filter(entity::maintenance_ticket::Column::PartnerJobId.eq(a)),
        (None, None) => return Some(JobOutcome::failed("alpha event names no job")),
    };
    let t = match q.one(db).await {
        Ok(Some(t)) => t,
        Ok(None) => {
            return Some(JobOutcome::completed(
                json!({ "provider": PROVIDER, "matched": false, "event": name }),
            ))
        }
        Err(e) => {
            return Some(JobOutcome::retry(
                crate::providers::backoff(job.attempts),
                format!("db: {e}"),
            ))
        }
    };
    let vendor = match t.partner_counterparty_id {
        Some(cid) => Counterparty::find_by_id(cid)
            .one(db)
            .await
            .ok()
            .flatten()
            .map(|c| c.name),
        None => None,
    };
    let (next_status, note) = apply_event(&name, &aj);
    let now = Utc::now();
    let alpha_status = aj
        .get("status")
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string();
    let mut am: entity::maintenance_ticket::ActiveModel = t.clone().into();
    if let Some(a) = alpha_id {
        am.partner_job_id = Set(Some(a));
    }
    if !alpha_status.is_empty() {
        am.partner_status = Set(Some(alpha_status));
    }
    am.partner_synced_at = Set(Some(now.into()));
    let mut changed = None;
    if let Some(s) = next_status {
        let reopening_closed = s == "open" && !crate::routes::maintenance::is_open(&t.status);
        if s != t.status && !reopening_closed {
            am.status = Set(s.into());
            changed = Some(s);
            if s == "resolved" {
                am.resolved_at = Set(Some(now.into()));
            }
        }
    }
    am.updated_at = Set(now.into());
    if let Err(e) = am.update(db).await {
        return Some(JobOutcome::retry(
            crate::providers::backoff(job.attempts),
            format!("db: {e}"),
        ));
    }
    add_note(
        db,
        tenant_id,
        t.id,
        if changed.is_some() {
            "status"
        } else {
            "comment"
        },
        &note,
        vendor.clone(),
    )
    .await;

    // A finished job's price is what the vendor charged: a labor line on the
    // work order, so the owner bill and costing carry it (once).
    if name == "job.completed" {
        if let Some(amount) = aj
            .get("price")
            .and_then(|p| p.as_str())
            .and_then(cents)
            .filter(|c| *c > 0)
        {
            let desc = format!(
                "{} — {}",
                vendor.clone().unwrap_or_else(|| "Vendor".into()),
                aj.get("service")
                    .and_then(|s| s.as_str())
                    .unwrap_or("service")
            );
            let dup = entity::prelude::TicketLine::find()
                .filter(entity::ticket_line::Column::TicketId.eq(t.id))
                .filter(entity::ticket_line::Column::Description.eq(desc.clone()))
                .one(db)
                .await
                .ok()
                .flatten();
            if dup.is_none() {
                let _ = entity::ticket_line::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    tenant_id: Set(tenant_id),
                    ticket_id: Set(t.id),
                    kind: Set("labor".into()),
                    description: Set(desc),
                    inventory_item_id: Set(None),
                    serial_number: Set(None),
                    quantity: Set(1),
                    unit_cost_cents: Set(amount),
                    total_cents: Set(amount),
                    created_by: Set(None),
                    created_at: Set(now.into()),
                }
                .insert(db)
                .await;
                if let Ok(Some(fresh)) = MaintenanceTicket::find_by_id(t.id).one(db).await {
                    let lines = entity::prelude::TicketLine::find()
                        .filter(entity::ticket_line::Column::TicketId.eq(t.id))
                        .all(db)
                        .await
                        .unwrap_or_default();
                    let total: i64 = lines.iter().map(|l| l.total_cents).sum();
                    let mut fam: entity::maintenance_ticket::ActiveModel = fresh.into();
                    fam.cost_cents = Set(Some(total));
                    let _ = fam.update(db).await;
                }
            }
        }
    }
    if let Some(s) = changed {
        crate::webhooks_out::emit(
            db,
            tenant_id,
            if s == "resolved" {
                "maintenance_ticket.resolved"
            } else {
                "maintenance_ticket.updated"
            },
            json!({ "ticket_id": t.id, "status": s, "source": "alpha" }),
        )
        .await;
    }
    Some(JobOutcome::completed(
        json!({ "provider": PROVIDER, "matched": true, "event": name, "ticket_id": t.id, "status": changed }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_from_alpha_decimals() {
        assert_eq!(cents("189.50"), Some(18950));
        assert_eq!(cents("189.5"), Some(18950));
        assert_eq!(cents("200"), Some(20000));
        assert_eq!(cents(""), None);
        assert_eq!(cents("abc"), None);
    }

    #[test]
    fn events_move_the_ticket() {
        let (s, note) = apply_event(
            "job.scheduled",
            &json!({ "scheduled_for": "2030-01-02T09:00:00-08:00", "crew": ["Carlos"] }),
        );
        assert_eq!(s, Some("scheduled"));
        assert!(note.contains("Carlos"), "{note}");
        let (s, note) = apply_event(
            "job.completed",
            &json!({ "price": "189.50", "report_note": "Rinsed twice.", "photos": [{ "url": "https://alpha.example/m/1.jpg" }] }),
        );
        assert_eq!(s, Some("resolved"));
        assert!(
            note.contains("$189.50") && note.contains("Rinsed twice.") && note.contains("1.jpg"),
            "{note}"
        );
        assert_eq!(
            apply_event("job.started", &json!({})).0,
            Some("in_progress")
        );
        assert_eq!(apply_event("job.photo", &json!({})).0, None);
    }

    #[test]
    fn callback_url_names_the_tenant() {
        assert!(callback_url("northwind").ends_with("/webhooks/alpha?tenant=northwind"));
    }
}
