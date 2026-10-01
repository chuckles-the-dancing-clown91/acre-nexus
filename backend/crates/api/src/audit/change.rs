//! **Change events** — the audit writer for "who changed what".
//!
//! [`change`] takes an entity's state *before* and *after* an update (both are
//! serializable models), diffs them field by field, and writes one
//! `audit_log` row carrying the changes, the property it concerns, and whether
//! a Vantedge employee made it on a customer's behalf. [`created`] and
//! [`removed`] record the other two events. All three are best-effort like the
//! rest of the audit layer: a failed write is logged, never propagated.
//!
//! The diff drops noise (timestamps, ids), and masks anything that looks like a
//! secret, so a password hash or token can never land in the trail.

use crate::auth::AuthUser;
use crate::tenancy::TenantScope;
use sea_orm::{ActiveModelTrait, ConnectionTrait, NotSet, Set};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

/// Who is acting, and in which workspace.
#[derive(Clone, Copy, Debug)]
pub struct Ctx {
    pub actor: Option<Uuid>,
    pub tenant_id: Uuid,
    /// A Vantedge employee acting on a customer's workspace.
    pub support: bool,
}

impl Ctx {
    pub fn new(user: &AuthUser, scope: &TenantScope) -> Ctx {
        Ctx {
            actor: Some(user.user_id),
            tenant_id: scope.tenant_id,
            support: user.is_staff || scope.impersonated,
        }
    }

    /// A background job or the system acting on a workspace.
    #[allow(dead_code)]
    pub fn system(tenant_id: Uuid) -> Ctx {
        Ctx {
            actor: None,
            tenant_id,
            support: false,
        }
    }
}

/// One field that changed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Change {
    pub field: String,
    pub from: Value,
    pub to: Value,
}

/// Fields never worth showing a person.
const NOISE: &[&str] = &[
    "id",
    "tenant_id",
    "created_at",
    "updated_at",
    "created_by",
    "updated_by",
];
/// Field names containing any of these are masked, not shown.
const SECRET_HINTS: &[&str] = &[
    "password",
    "secret",
    "token",
    "ciphertext",
    "nonce",
    "ssn",
    "api_key",
    "_hash",
    // Contact details are personal data: the trail says they changed, not what to.
    "email",
    "phone",
];

fn is_secret(field: &str) -> bool {
    let f = field.to_lowercase();
    SECRET_HINTS.iter().any(|h| f.contains(h))
}

fn masked() -> Value {
    json!("(hidden)")
}

/// The field-level difference between two JSON objects (pure). Values that
/// compare equal produce no change; secrets are masked on both sides and only
/// reported as changed.
pub fn diff(before: &Value, after: &Value) -> Vec<Change> {
    let (Some(b), Some(a)) = (before.as_object(), after.as_object()) else {
        return Vec::new();
    };
    let mut keys: Vec<&String> = b.keys().chain(a.keys()).collect();
    keys.sort();
    keys.dedup();
    let mut out = Vec::new();
    for k in keys {
        if NOISE.contains(&k.as_str()) {
            continue;
        }
        let (from, to) = (
            b.get(k).cloned().unwrap_or(Value::Null),
            a.get(k).cloned().unwrap_or(Value::Null),
        );
        if from == to {
            continue;
        }
        if is_secret(k) {
            out.push(Change {
                field: k.clone(),
                from: masked(),
                to: masked(),
            });
        } else {
            out.push(Change {
                field: k.clone(),
                from,
                to,
            });
        }
    }
    out
}

/// A short human summary of a change set: "changed rent, status".
pub fn summary(changes: &[Change]) -> String {
    match changes.len() {
        0 => "saved with no changes".into(),
        1..=4 => format!(
            "changed {}",
            changes
                .iter()
                .map(|c| c.field.replace('_', " "))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        n => format!("changed {n} fields"),
    }
}

#[allow(clippy::too_many_arguments)]
async fn write(
    db: &impl ConnectionTrait,
    ctx: Ctx,
    action: &str,
    target_type: &str,
    target_id: String,
    property_id: Option<Uuid>,
    metadata: Value,
) {
    let entry = entity::audit_log::ActiveModel {
        id: Set(Uuid::new_v4()),
        actor_user_id: Set(ctx.actor),
        action: Set(action.to_string()),
        target_type: Set(Some(target_type.to_string())),
        target_id: Set(Some(target_id)),
        tenant_id: Set(Some(ctx.tenant_id)),
        metadata: Set(Some(metadata)),
        method: NotSet,
        path: NotSet,
        status_code: NotSet,
        request_id: NotSet,
        ip: NotSet,
        duration_ms: NotSet,
        principal_kind: Set(Some(
            if ctx.actor.is_some() {
                "user"
            } else {
                "system"
            }
            .to_string(),
        )),
        property_id: Set(property_id),
        support: Set(ctx.support),
        created_at: Set(chrono::Utc::now().into()),
    };
    if let Err(e) = entry.insert(db).await {
        tracing::error!("audit change write failed for '{action}': {e}");
    }
}

/// Record an update: the diff of `before` → `after`. Writes nothing when no
/// field changed, so a no-op save leaves no noise in the trail.
#[allow(clippy::too_many_arguments)]
pub async fn change<T: Serialize>(
    db: &impl ConnectionTrait,
    ctx: Ctx,
    action: &str,
    target_type: &str,
    target_id: Uuid,
    property_id: Option<Uuid>,
    label: &str,
    before: &T,
    after: &T,
) {
    let (Ok(b), Ok(a)) = (serde_json::to_value(before), serde_json::to_value(after)) else {
        return;
    };
    let changes = diff(&b, &a);
    if changes.is_empty() {
        return;
    }
    write(
        db,
        ctx,
        action,
        target_type,
        target_id.to_string(),
        property_id,
        json!({ "label": label, "summary": summary(&changes), "changes": changes }),
    )
    .await;
}

/// Record that something was created (`label` is what a person would call it).
pub async fn created(
    db: &impl ConnectionTrait,
    ctx: Ctx,
    action: &str,
    target_type: &str,
    target_id: Uuid,
    property_id: Option<Uuid>,
    label: &str,
) {
    write(
        db,
        ctx,
        action,
        target_type,
        target_id.to_string(),
        property_id,
        json!({ "label": label, "summary": "created", "changes": [] }),
    )
    .await;
}

/// Record an event with a plain-language summary (bulk edits, imports).
#[allow(clippy::too_many_arguments)]
pub async fn noted(
    db: &impl ConnectionTrait,
    ctx: Ctx,
    action: &str,
    target_type: &str,
    target_id: Uuid,
    property_id: Option<Uuid>,
    label: &str,
    summary: &str,
) {
    write(
        db,
        ctx,
        action,
        target_type,
        target_id.to_string(),
        property_id,
        json!({ "label": label, "summary": summary, "changes": [] }),
    )
    .await;
}

/// Record that something was removed or retired, with a reason when there is one.
#[allow(dead_code, clippy::too_many_arguments)]
pub async fn removed(
    db: &impl ConnectionTrait,
    ctx: Ctx,
    action: &str,
    target_type: &str,
    target_id: Uuid,
    property_id: Option<Uuid>,
    label: &str,
    reason: Option<&str>,
) {
    write(
        db,
        ctx,
        action,
        target_type,
        target_id.to_string(),
        property_id,
        json!({ "label": label, "summary": "removed", "changes": [], "reason": reason }),
    )
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_reports_only_what_changed_and_skips_noise() {
        let before = json!({ "id": 1, "name": "Elm", "rent_cents": 120000, "updated_at": "a", "note": null });
        let after = json!({ "id": 1, "name": "Elm", "rent_cents": 130000, "updated_at": "b", "note": "hi" });
        let d = diff(&before, &after);
        assert_eq!(d.len(), 2);
        assert_eq!(
            d[0],
            Change {
                field: "note".into(),
                from: Value::Null,
                to: json!("hi")
            }
        );
        assert_eq!(d[1].field, "rent_cents");
        assert_eq!(d[1].to, json!(130000));
        assert_eq!(summary(&d), "changed note, rent cents");
    }

    #[test]
    fn secrets_are_masked_never_shown() {
        let d = diff(
            &json!({ "password_hash": "abc", "api_token": "x" }),
            &json!({ "password_hash": "def", "api_token": "y" }),
        );
        assert_eq!(d.len(), 2);
        assert!(d.iter().all(|c| c.from == masked() && c.to == masked()));
    }

    #[test]
    fn identical_states_have_no_changes() {
        let v = json!({ "a": 1, "b": [1, 2] });
        assert!(diff(&v, &v).is_empty());
        assert_eq!(summary(&[]), "saved with no changes");
        assert!(diff(&json!(1), &json!(2)).is_empty());
    }

    #[test]
    fn a_long_change_set_is_counted_not_listed() {
        let b = json!({ "a": 1, "b": 1, "c": 1, "d": 1, "e": 1 });
        let a = json!({ "a": 2, "b": 2, "c": 2, "d": 2, "e": 2 });
        assert_eq!(summary(&diff(&b, &a)), "changed 5 fields");
    }
}
