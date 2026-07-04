//! **Web Push subscription** management: the platform VAPID public key (what
//! the browser passes as `applicationServerKey`) and register/unregister for
//! the signed-in user's subscriptions.

use super::dto::PushSubscribeReq;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::notify::webpush;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::PushSubscription;
use rocket::serde::json::Json;
use rocket::{delete, get, post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, ModelTrait, QueryFilter, Set};
use uuid::Uuid;

/// `GET /notifications/vapid_key` — the platform's VAPID public key
/// (base64url), generated and vaulted on first use.
#[rocket_okapi::openapi(tag = "Notifications")]
#[get("/notifications/vapid_key")]
pub async fn vapid_key(
    state: &State<AppState>,
    _user: AuthUser,
) -> ApiResult<Json<serde_json::Value>> {
    // Uses the unclamped connection: the key is a platform-wide secret and may
    // need to be created on first call.
    let sk = webpush::vapid_signing_key(&state.db).await?;
    Ok(Json(serde_json::json!({
        "key": webpush::vapid_public_key_b64(&sk)
    })))
}

/// `POST /notifications/push_subscriptions` — register (or refresh) this
/// browser's push subscription for the signed-in user.
#[rocket_okapi::openapi(tag = "Notifications")]
#[post("/notifications/push_subscriptions", data = "<body>")]
pub async fn subscribe(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<PushSubscribeReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let b = body.into_inner();
    if !b.endpoint.starts_with("https://") {
        return Err(ApiError::BadRequest("endpoint must be https".into()));
    }
    if b.p256dh.trim().is_empty() || b.auth.trim().is_empty() {
        return Err(ApiError::BadRequest("p256dh and auth are required".into()));
    }

    // One row per endpoint: re-subscribing refreshes keys and ownership.
    let existing = PushSubscription::find()
        .filter(entity::push_subscription::Column::Endpoint.eq(b.endpoint.clone()))
        .one(&db)
        .await?;
    let id = match existing {
        Some(row) => {
            let id = row.id;
            let mut am: entity::push_subscription::ActiveModel = row.into();
            am.tenant_id = Set(scope.tenant_id);
            am.user_id = Set(user.user_id);
            am.p256dh = Set(b.p256dh.trim().to_string());
            am.auth = Set(b.auth.trim().to_string());
            am.update(&db).await?;
            id
        }
        None => {
            let id = Uuid::new_v4();
            entity::push_subscription::ActiveModel {
                id: Set(id),
                tenant_id: Set(scope.tenant_id),
                user_id: Set(user.user_id),
                endpoint: Set(b.endpoint.trim().to_string()),
                p256dh: Set(b.p256dh.trim().to_string()),
                auth: Set(b.auth.trim().to_string()),
                user_agent: Set(None),
                created_at: Set(Utc::now().into()),
            }
            .insert(&db)
            .await?;
            id
        }
    };

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::PUSH_SUBSCRIBE,
        Some("push_subscription"),
        Some(id.to_string()),
        Some(scope.tenant_id),
        None,
    )
    .await;

    Ok(Json(serde_json::json!({ "subscribed": true, "id": id })))
}

/// `POST /notifications/test_push` — enqueue a test Web Push to every
/// subscription the signed-in user holds (simulated unless `LIVE_PROVIDERS`
/// enables `push`).
#[rocket_okapi::openapi(tag = "Notifications")]
#[post("/notifications/test_push")]
pub async fn test_push(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<serde_json::Value>> {
    let me = entity::prelude::User::find_by_id(user.user_id)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("user not found".into()))?;
    let job_id = crate::scheduler::enqueue(
        &db,
        scope.tenant_id,
        "auto_push",
        serde_json::json!({
            "template": "test_notification",
            "to": me.email,
            "user_id": user.user_id.to_string(),
        }),
        0,
    )
    .await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::NOTIFICATION_TEST,
        Some("user"),
        Some(user.user_id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "channel": "push", "job_id": job_id })),
    )
    .await;
    Ok(Json(
        serde_json::json!({ "queued": true, "job_id": job_id }),
    ))
}

/// `DELETE /notifications/push_subscriptions?endpoint=…` — remove this
/// browser's subscription for the signed-in user.
#[rocket_okapi::openapi(tag = "Notifications")]
#[delete("/notifications/push_subscriptions?<endpoint>")]
pub async fn unsubscribe(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    endpoint: &str,
) -> ApiResult<Json<serde_json::Value>> {
    let row = PushSubscription::find()
        .filter(entity::push_subscription::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::push_subscription::Column::UserId.eq(user.user_id))
        .filter(entity::push_subscription::Column::Endpoint.eq(endpoint))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("subscription not found".into()))?;
    let id = row.id;
    row.delete(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::PUSH_UNSUBSCRIBE,
        Some("push_subscription"),
        Some(id.to_string()),
        Some(scope.tenant_id),
        None,
    )
    .await;

    Ok(Json(serde_json::json!({ "unsubscribed": true })))
}
