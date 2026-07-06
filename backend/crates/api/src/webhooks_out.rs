//! **Vendor API outbound webhooks** (issue #68) — vendors *subscribe* to
//! change events instead of polling `/api/v1`.
//!
//! * **Event catalog** — [`EVENTS`] maps each event type to the scope that
//!   gates it, so a token can only subscribe to data it can already read
//!   ([`validate_event_types`]).
//! * **Emission** — mutation sites call [`emit`] after the change persists.
//!   Each enabled, still-authorized subscription gets a `webhook_delivery`
//!   row and a durable `webhook_deliver` job.
//! * **Delivery** — the job signs the payload (HMAC-SHA256 over the raw
//!   body, `X-Acre-Signature`, verifiable with the secret returned once at
//!   subscription creation) and POSTs it via the sandbox-first
//!   [`crate::providers::webhook_out::WebhookOutProvider`]. Failures retry
//!   with the shared backoff until `max_attempts`, then the delivery
//!   dead-letters (`status = dead`) — visible (and replayable) through the
//!   vendor's delivery history.

use crate::modules::JobOutcome;
use crate::providers::webhook_out::{WebhookOutProvider, WebhookOutRequest};
use crate::providers::{webhook, ProviderCtx};
use crate::rbac::Permission;
use chrono::Utc;
use entity::prelude::{ApiToken, WebhookDelivery, WebhookSubscription};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    Set,
};
use serde_json::json;
use uuid::Uuid;

/// The background-job kind that delivers one webhook.
pub const DELIVER_JOB_KIND: &str = "webhook_deliver";

/// Every event type a vendor can subscribe to, with the scope that gates it —
/// the same permission strings that gate the corresponding `/api/v1` reads.
pub const EVENTS: &[(&str, Permission)] = &[
    ("listing.created", Permission::ListingRead),
    ("listing.updated", Permission::ListingRead),
    ("application.created", Permission::ApplicationRead),
    ("payment.recorded", Permission::PaymentRead),
    ("maintenance_ticket.created", Permission::MaintenanceRead),
];

/// The scope required to receive `event_type`, if it exists.
pub fn required_scope(event_type: &str) -> Option<Permission> {
    EVENTS
        .iter()
        .find(|(e, _)| *e == event_type)
        .map(|(_, p)| *p)
}

/// Validate a subscription's requested event types against the token's
/// scopes: unknown events and events the token can't already read are both
/// rejected.
pub fn validate_event_types(event_types: &[String], scopes: &[String]) -> Result<(), String> {
    if event_types.is_empty() {
        return Err("at least one event type is required".into());
    }
    for event in event_types {
        let Some(perm) = required_scope(event) else {
            let known: Vec<&str> = EVENTS.iter().map(|(e, _)| *e).collect();
            return Err(format!(
                "unknown event type '{event}' (expected one of {})",
                known.join(", ")
            ));
        };
        if !scopes.iter().any(|s| s == perm.as_str()) {
            return Err(format!(
                "token missing scope {} required to subscribe to {event}",
                perm.as_str()
            ));
        }
    }
    Ok(())
}

/// The vault key a subscription's signing secret lives under.
pub fn secret_ref(subscription_id: Uuid) -> String {
    format!("webhook_sub.{subscription_id}.secret")
}

fn json_string_vec(v: &serde_json::Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Emission
// ---------------------------------------------------------------------------

/// Fan one domain event out to every enabled, still-authorized subscription:
/// a `webhook_delivery` row each, driven by a durable `webhook_deliver` job.
/// Best-effort — an emission failure never fails the mutation that raised it.
pub async fn emit(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    event_type: &str,
    payload: serde_json::Value,
) {
    let Some(perm) = required_scope(event_type) else {
        tracing::error!("webhooks: emit called with unknown event type {event_type}");
        return;
    };
    let subscriptions = match WebhookSubscription::find()
        .filter(entity::webhook_subscription::Column::TenantId.eq(tenant_id))
        .filter(entity::webhook_subscription::Column::Enabled.eq(true))
        .all(db)
        .await
    {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("webhooks: subscription scan failed: {e}");
            return;
        }
    };
    let now = Utc::now();
    for sub in subscriptions {
        if !json_string_vec(&sub.event_types)
            .iter()
            .any(|e| e == event_type)
        {
            continue;
        }
        // The owning token must still be live and still hold the scope —
        // revoking a token (or narrowing it) silences its subscriptions.
        let token = ApiToken::find_by_id(sub.api_token_id).one(db).await;
        let authorized = match token {
            Ok(Some(t)) => {
                let live =
                    t.revoked_at.is_none() && t.expires_at.map(|exp| exp >= now).unwrap_or(true);
                let scopes: Vec<String> =
                    serde_json::from_value(t.scopes.clone()).unwrap_or_default();
                live && scopes.iter().any(|s| s == perm.as_str())
            }
            _ => false,
        };
        if !authorized {
            continue;
        }

        let delivery = entity::webhook_delivery::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            subscription_id: Set(sub.id),
            event_type: Set(event_type.to_string()),
            payload: Set(payload.clone()),
            status: Set("pending".into()),
            attempts: Set(0),
            response_status: Set(None),
            last_error: Set(None),
            delivered_at: Set(None),
            created_at: Set(now.into()),
        };
        let delivery = match delivery.insert(db).await {
            Ok(d) => d,
            Err(e) => {
                tracing::error!("webhooks: delivery row insert failed: {e}");
                continue;
            }
        };
        if let Err(e) = crate::scheduler::enqueue(
            db,
            tenant_id,
            DELIVER_JOB_KIND,
            json!({ "delivery_id": delivery.id }),
            0,
        )
        .await
        {
            tracing::error!("webhooks: delivery enqueue failed: {e}");
        }
    }
}

// ---------------------------------------------------------------------------
// Delivery
// ---------------------------------------------------------------------------

/// Advance one `webhook_deliver` job: sign the payload and POST it; update
/// the delivery row on every attempt so the vendor's history is live.
pub async fn handle_deliver_job(
    db: &DatabaseConnection,
    job: &entity::background_job::Model,
) -> JobOutcome {
    let Some(delivery_id) = job
        .payload
        .get("delivery_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    else {
        return JobOutcome::failed("webhook_deliver payload missing delivery_id");
    };
    let delivery = match WebhookDelivery::find_by_id(delivery_id)
        .filter(entity::webhook_delivery::Column::TenantId.eq(job.tenant_id))
        .one(db)
        .await
    {
        Ok(Some(d)) => d,
        Ok(None) => return JobOutcome::failed("webhook delivery not found"),
        Err(e) => {
            return JobOutcome::retry(
                crate::providers::backoff(job.attempts),
                format!("db error: {e}"),
            )
        }
    };
    if delivery.status == "delivered" {
        return JobOutcome::completed(json!({ "already_delivered": true }));
    }
    let subscription = match WebhookSubscription::find_by_id(delivery.subscription_id)
        .filter(entity::webhook_subscription::Column::TenantId.eq(job.tenant_id))
        .one(db)
        .await
    {
        Ok(Some(s)) => s,
        Ok(None) => {
            mark(db, &delivery, "dead", None, Some("subscription deleted")).await;
            return JobOutcome::failed("subscription deleted");
        }
        Err(e) => {
            return JobOutcome::retry(
                crate::providers::backoff(job.attempts),
                format!("db error: {e}"),
            )
        }
    };
    if !subscription.enabled {
        mark(db, &delivery, "dead", None, Some("subscription disabled")).await;
        return JobOutcome::completed(json!({ "skipped": "subscription disabled" }));
    }

    let secret =
        match crate::secrets::reveal(db, Some(job.tenant_id), &subscription.secret_ref).await {
            Ok(Some(s)) => s,
            Ok(None) => {
                mark(db, &delivery, "dead", None, Some("signing secret missing")).await;
                return JobOutcome::failed("signing secret missing");
            }
            Err(e) => {
                return JobOutcome::retry(
                    crate::providers::backoff(job.attempts),
                    format!("secret lookup failed: {e}"),
                )
            }
        };

    // The envelope subscribers receive; the signature covers these bytes.
    let body = json!({
        "id": delivery.id,
        "event": delivery.event_type,
        "created_at": delivery.created_at.to_rfc3339(),
        "data": delivery.payload,
    })
    .to_string();
    let req = WebhookOutRequest {
        url: subscription.url.clone(),
        signature: webhook::sign(&secret, body.as_bytes()),
        body,
        event_type: delivery.event_type.clone(),
        delivery_id: delivery.id.to_string(),
    };

    let ctx = ProviderCtx::new(db, job.tenant_id);
    match crate::providers::run(&WebhookOutProvider, &ctx, job, &req).await {
        Ok(resp) => {
            let now = Utc::now();
            let mut am: entity::webhook_delivery::ActiveModel = delivery.into();
            am.status = Set("delivered".into());
            am.attempts = Set(job.attempts + 1);
            am.response_status = Set(Some(resp.status_code as i32));
            am.last_error = Set(None);
            am.delivered_at = Set(Some(now.into()));
            if let Err(e) = am.update(db).await {
                tracing::error!("webhooks: delivered-state update failed: {e}");
            }
            JobOutcome::completed(json!({ "delivered": true, "status": resp.status_code }))
        }
        Err(outcome) => {
            // Terminal failure dead-letters the delivery; a transient retry
            // keeps it pending with the error on display.
            let terminal = !outcome.retry;
            mark(
                db,
                &delivery,
                if terminal { "dead" } else { "pending" },
                Some(job.attempts + 1),
                outcome.error.as_deref(),
            )
            .await;
            outcome
        }
    }
}

/// Update a delivery row's observable state (best-effort).
async fn mark(
    db: &impl ConnectionTrait,
    delivery: &entity::webhook_delivery::Model,
    status: &str,
    attempts: Option<i32>,
    error: Option<&str>,
) {
    let mut am: entity::webhook_delivery::ActiveModel = delivery.clone().into();
    am.status = Set(status.to_string());
    if let Some(a) = attempts {
        am.attempts = Set(a);
    }
    if let Some(e) = error {
        am.last_error = Set(Some(e.to_string()));
    }
    if let Err(e) = am.update(db).await {
        tracing::error!("webhooks: delivery-state update failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scopes(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn subscriptions_respect_token_scopes() {
        let ok = validate_event_types(
            &["listing.updated".into(), "payment.recorded".into()],
            &scopes(&["listing:read", "payment:read"]),
        );
        assert!(ok.is_ok());

        // A token can't subscribe to data it can't read…
        let err = validate_event_types(&["payment.recorded".into()], &scopes(&["listing:read"]))
            .unwrap_err();
        assert!(err.contains("payment:read"));

        // …and unknown events are named in the rejection.
        let err = validate_event_types(&["listing.deleted".into()], &scopes(&["listing:read"]))
            .unwrap_err();
        assert!(err.contains("unknown event type"));

        assert!(validate_event_types(&[], &scopes(&["listing:read"])).is_err());
    }

    #[test]
    fn every_catalog_event_maps_to_a_scope() {
        for (event, perm) in EVENTS {
            assert_eq!(required_scope(event), Some(*perm));
        }
        assert_eq!(required_scope("nope"), None);
    }

    #[test]
    fn secret_refs_are_stable() {
        let id = Uuid::from_u128(9);
        assert_eq!(secret_ref(id), format!("webhook_sub.{id}.secret"));
    }
}
