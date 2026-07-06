//! Vendor **webhook subscription** endpoints (`/api/v1/webhooks`, issue #68)
//! — subscribe, don't poll. Token-authenticated like every `/api/v1` route; a
//! subscription belongs to the token that created it, and its requested event
//! types are validated against that token's scopes.

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use crate::tokens::ApiPrincipal;
use crate::webhooks_out;
use chrono::Utc;
use entity::prelude::{WebhookDelivery, WebhookSubscription};
use rocket::serde::json::Json;
use rocket::{delete, get, patch, post, State};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, ModelTrait, QueryFilter, QueryOrder, QuerySelect,
    Set,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(Serialize, schemars::JsonSchema)]
pub struct EventTypeDto {
    pub event: String,
    /// The token scope required to subscribe.
    pub required_scope: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct SubscriptionDto {
    pub id: Uuid,
    pub url: String,
    pub event_types: Vec<String>,
    pub enabled: bool,
    pub description: Option<String>,
    pub created_at: String,
}

impl From<entity::webhook_subscription::Model> for SubscriptionDto {
    fn from(s: entity::webhook_subscription::Model) -> Self {
        SubscriptionDto {
            id: s.id,
            url: s.url,
            event_types: serde_json::from_value(s.event_types).unwrap_or_default(),
            enabled: s.enabled,
            description: s.description,
            created_at: s.created_at.to_rfc3339(),
        }
    }
}

/// Creation response — the signing secret appears here **once**, like an API
/// token's raw value. Verify deliveries by HMAC-SHA256ing the raw body with
/// it and comparing to `X-Acre-Signature`.
#[derive(Serialize, schemars::JsonSchema)]
pub struct CreateSubscriptionResp {
    #[serde(flatten)]
    pub subscription: SubscriptionDto,
    pub secret: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateSubscriptionReq {
    pub url: String,
    pub event_types: Vec<String>,
    pub description: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateSubscriptionReq {
    pub url: Option<String>,
    pub event_types: Option<Vec<String>>,
    pub enabled: Option<bool>,
    pub description: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct DeliveryDto {
    pub id: Uuid,
    pub event_type: String,
    pub status: String,
    pub attempts: i32,
    pub response_status: Option<i32>,
    pub last_error: Option<String>,
    pub delivered_at: Option<String>,
    pub created_at: String,
}

impl From<entity::webhook_delivery::Model> for DeliveryDto {
    fn from(d: entity::webhook_delivery::Model) -> Self {
        DeliveryDto {
            id: d.id,
            event_type: d.event_type,
            status: d.status,
            attempts: d.attempts,
            response_status: d.response_status,
            last_error: d.last_error,
            delivered_at: d.delivered_at.map(|x| x.to_rfc3339()),
            created_at: d.created_at.to_rfc3339(),
        }
    }
}

fn validate_url(url: &str) -> Result<(), ApiError> {
    let url = url.trim();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(ApiError::BadRequest(
            "url must be an http(s) callback URL".into(),
        ));
    }
    Ok(())
}

/// Load a subscription owned by the presenting token (a vendor never sees
/// another integration's subscriptions, even on the same tenant).
async fn own_subscription(
    db: &crate::db::RequestDb,
    principal: &ApiPrincipal,
    id: &str,
) -> ApiResult<entity::webhook_subscription::Model> {
    let sid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    WebhookSubscription::find_by_id(sid)
        .filter(entity::webhook_subscription::Column::TenantId.eq(principal.tenant_id))
        .filter(entity::webhook_subscription::Column::ApiTokenId.eq(principal.token_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("subscription not found".into()))
}

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

/// `GET /api/v1/webhooks/events` — the subscribable event catalog with the
/// scope each requires.
#[rocket_okapi::openapi(tag = "Vendor API")]
#[get("/api/v1/webhooks/events")]
pub async fn event_catalog(
    _state: &State<AppState>,
    _principal: ApiPrincipal,
) -> ApiResult<Json<Vec<EventTypeDto>>> {
    Ok(Json(
        webhooks_out::EVENTS
            .iter()
            .map(|(event, perm)| EventTypeDto {
                event: event.to_string(),
                required_scope: perm.as_str().to_string(),
            })
            .collect(),
    ))
}

/// `GET /api/v1/webhooks` — this token's subscriptions.
#[rocket_okapi::openapi(tag = "Vendor API")]
#[get("/api/v1/webhooks")]
pub async fn list_subscriptions(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    principal: ApiPrincipal,
) -> ApiResult<Json<Vec<SubscriptionDto>>> {
    let rows = WebhookSubscription::find()
        .filter(entity::webhook_subscription::Column::TenantId.eq(principal.tenant_id))
        .filter(entity::webhook_subscription::Column::ApiTokenId.eq(principal.token_id))
        .order_by_desc(entity::webhook_subscription::Column::CreatedAt)
        .all(&db)
        .await?;
    Ok(Json(rows.into_iter().map(SubscriptionDto::from).collect()))
}

/// `POST /api/v1/webhooks` — register a callback. The requested event types
/// must be covered by this token's scopes; the response carries the signing
/// secret exactly once.
#[rocket_okapi::openapi(tag = "Vendor API")]
#[post("/api/v1/webhooks", data = "<body>")]
pub async fn create_subscription(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    principal: ApiPrincipal,
    body: Json<CreateSubscriptionReq>,
) -> ApiResult<Json<CreateSubscriptionResp>> {
    let b = body.into_inner();
    validate_url(&b.url)?;
    webhooks_out::validate_event_types(&b.event_types, &principal.scopes)
        .map_err(ApiError::Forbidden)?;

    let id = Uuid::new_v4();
    let secret = format!("whsec_{}", crate::auth::random_secret(24));
    let secret_ref = webhooks_out::secret_ref(id);
    crate::secrets::store(&db, Some(principal.tenant_id), &secret_ref, &secret, None).await?;

    let now = Utc::now();
    let saved = entity::webhook_subscription::ActiveModel {
        id: Set(id),
        tenant_id: Set(principal.tenant_id),
        api_token_id: Set(principal.token_id),
        url: Set(b.url.trim().to_string()),
        event_types: Set(json!(b.event_types)),
        secret_ref: Set(secret_ref),
        enabled: Set(true),
        description: Set(b.description.filter(|d| !d.trim().is_empty())),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;

    crate::audit::record(
        &db,
        None,
        crate::audit::actions::WEBHOOK_SUB_CREATE,
        Some("webhook_subscription"),
        Some(saved.id.to_string()),
        Some(principal.tenant_id),
        Some(json!({
            "api_token_id": principal.token_id,
            "url": saved.url,
            "event_types": saved.event_types,
        })),
    )
    .await;

    Ok(Json(CreateSubscriptionResp {
        subscription: SubscriptionDto::from(saved),
        secret,
    }))
}

/// `PATCH /api/v1/webhooks/<id>` — update the URL, event types (re-validated
/// against scopes), enabled flag, or description.
#[rocket_okapi::openapi(tag = "Vendor API")]
#[patch("/api/v1/webhooks/<id>", data = "<body>")]
pub async fn update_subscription(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    principal: ApiPrincipal,
    id: &str,
    body: Json<UpdateSubscriptionReq>,
) -> ApiResult<Json<SubscriptionDto>> {
    let sub = own_subscription(&db, &principal, id).await?;
    let b = body.into_inner();
    let mut am: entity::webhook_subscription::ActiveModel = sub.into();
    if let Some(url) = b.url {
        validate_url(&url)?;
        am.url = Set(url.trim().to_string());
    }
    if let Some(events) = b.event_types {
        webhooks_out::validate_event_types(&events, &principal.scopes)
            .map_err(ApiError::Forbidden)?;
        am.event_types = Set(json!(events));
    }
    if let Some(enabled) = b.enabled {
        am.enabled = Set(enabled);
    }
    if let Some(desc) = b.description {
        am.description = Set(Some(desc).filter(|d| !d.trim().is_empty()));
    }
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&db).await?;

    crate::audit::record(
        &db,
        None,
        crate::audit::actions::WEBHOOK_SUB_UPDATE,
        Some("webhook_subscription"),
        Some(saved.id.to_string()),
        Some(principal.tenant_id),
        Some(json!({ "enabled": saved.enabled, "event_types": saved.event_types })),
    )
    .await;

    Ok(Json(SubscriptionDto::from(saved)))
}

/// `DELETE /api/v1/webhooks/<id>` — remove a subscription (and its vaulted
/// signing secret). Delivery history is kept.
#[rocket_okapi::openapi(tag = "Vendor API")]
#[delete("/api/v1/webhooks/<id>")]
pub async fn delete_subscription(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    principal: ApiPrincipal,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    let sub = own_subscription(&db, &principal, id).await?;
    let sub_id = sub.id;
    let secret_ref = sub.secret_ref.clone();
    sub.delete(&db).await?;
    let _ = crate::secrets::remove(&db, Some(principal.tenant_id), &secret_ref).await;

    crate::audit::record(
        &db,
        None,
        crate::audit::actions::WEBHOOK_SUB_DELETE,
        Some("webhook_subscription"),
        Some(sub_id.to_string()),
        Some(principal.tenant_id),
        None,
    )
    .await;

    Ok(Json(json!({ "deleted": true })))
}

/// `GET /api/v1/webhooks/<id>/deliveries` — delivery history, newest first:
/// status, attempts, subscriber response, and the last error.
#[rocket_okapi::openapi(tag = "Vendor API")]
#[get("/api/v1/webhooks/<id>/deliveries")]
pub async fn list_deliveries(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    principal: ApiPrincipal,
    id: &str,
) -> ApiResult<Json<Vec<DeliveryDto>>> {
    let sub = own_subscription(&db, &principal, id).await?;
    let rows = WebhookDelivery::find()
        .filter(entity::webhook_delivery::Column::TenantId.eq(principal.tenant_id))
        .filter(entity::webhook_delivery::Column::SubscriptionId.eq(sub.id))
        .order_by_desc(entity::webhook_delivery::Column::CreatedAt)
        .limit(100)
        .all(&db)
        .await?;
    Ok(Json(rows.into_iter().map(DeliveryDto::from).collect()))
}

/// `POST /api/v1/webhooks/<id>/deliveries/<delivery_id>/replay` — re-send one
/// delivery as a fresh attempt (a new delivery row, so history stays honest).
#[rocket_okapi::openapi(tag = "Vendor API")]
#[post("/api/v1/webhooks/<id>/deliveries/<delivery_id>/replay")]
pub async fn replay_delivery(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    principal: ApiPrincipal,
    id: &str,
    delivery_id: &str,
) -> ApiResult<Json<DeliveryDto>> {
    let sub = own_subscription(&db, &principal, id).await?;
    let did =
        Uuid::parse_str(delivery_id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let original = WebhookDelivery::find_by_id(did)
        .filter(entity::webhook_delivery::Column::TenantId.eq(principal.tenant_id))
        .filter(entity::webhook_delivery::Column::SubscriptionId.eq(sub.id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("delivery not found".into()))?;

    let now = Utc::now();
    let replay = entity::webhook_delivery::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(principal.tenant_id),
        subscription_id: Set(sub.id),
        event_type: Set(original.event_type.clone()),
        payload: Set(original.payload.clone()),
        status: Set("pending".into()),
        attempts: Set(0),
        response_status: Set(None),
        last_error: Set(None),
        delivered_at: Set(None),
        created_at: Set(now.into()),
    }
    .insert(&db)
    .await?;
    crate::scheduler::enqueue(
        &db,
        principal.tenant_id,
        webhooks_out::DELIVER_JOB_KIND,
        json!({ "delivery_id": replay.id }),
        0,
    )
    .await?;

    crate::audit::record(
        &db,
        None,
        crate::audit::actions::WEBHOOK_REPLAY,
        Some("webhook_delivery"),
        Some(replay.id.to_string()),
        Some(principal.tenant_id),
        Some(json!({ "original_delivery_id": original.id })),
    )
    .await;

    Ok(Json(DeliveryDto::from(replay)))
}
