//! **System settings** — a per-tenant, code-defined configuration catalog.
//!
//! Like the RBAC and workflow catalogs, the *set* of settings is defined in code
//! ([`CATALOG`]) — each with a key, type, default, and human label/group — while
//! the *values* are stored per tenant in the `setting` table. Absence of a row
//! means "use the default", so a fresh tenant is fully configured out of the box
//! and adding a new setting never needs a data backfill.
//!
//! Handlers read settings with the typed helpers ([`get_bool`], [`get_i64`]),
//! which validate the key against the catalog and fall back to its default. The
//! `routes::settings` endpoints expose the merged catalog+values and let a tenant
//! admin (`tenant:manage`) override them.

use crate::error::{ApiError, ApiResult};
use chrono::Utc;
use entity::prelude::Setting;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set as ActiveSet,
};
use serde_json::{json, Value};
use uuid::Uuid;

// ---- Known setting keys ----------------------------------------------------

/// Allow reusing a recent application for any property in the firm.
pub const APPLICATION_REUSE_ENABLED: &str = "application_reuse.enabled";
/// How many days a prior application stays reusable.
pub const APPLICATION_REUSE_WINDOW_DAYS: &str = "application_reuse.window_days";

/// The value type of a setting (drives validation + the UI control).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingKind {
    Bool,
    Int,
    /// A free-text setting. Reserved for future catalog entries.
    #[allow(dead_code)]
    Text,
}

impl SettingKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SettingKind::Bool => "bool",
            SettingKind::Int => "int",
            SettingKind::Text => "text",
        }
    }

    /// Whether `v` is a valid JSON value for this kind.
    fn validate(&self, v: &Value) -> bool {
        match self {
            SettingKind::Bool => v.is_boolean(),
            SettingKind::Int => v.is_i64() || v.is_u64(),
            SettingKind::Text => v.is_string(),
        }
    }
}

/// One entry in the settings catalog.
pub struct SettingDef {
    pub key: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub group: &'static str,
    pub kind: SettingKind,
    /// Default value when the tenant has no override row.
    pub default: fn() -> Value,
}

/// Every recognized setting. Add new tenant-configurable knobs here.
pub const CATALOG: &[SettingDef] = &[
    SettingDef {
        key: APPLICATION_REUSE_ENABLED,
        label: "Reusable applications",
        description: "Let a recent application be reused for any property in the \
                      workspace, so applicants don't re-apply per listing.",
        group: "Applications",
        kind: SettingKind::Bool,
        default: || json!(false),
    },
    SettingDef {
        key: APPLICATION_REUSE_WINDOW_DAYS,
        label: "Reuse window (days)",
        description: "How many days a prior application stays reusable.",
        group: "Applications",
        kind: SettingKind::Int,
        default: || json!(30),
    },
];

/// Look up a catalog entry by key.
pub fn def(key: &str) -> Option<&'static SettingDef> {
    CATALOG.iter().find(|d| d.key == key)
}

/// The effective JSON value for `key` in `tenant_id` (override row or default).
pub async fn get_value(db: &impl ConnectionTrait, tenant_id: Uuid, key: &str) -> Value {
    let default = def(key).map(|d| (d.default)()).unwrap_or(Value::Null);
    match Setting::find()
        .filter(entity::setting::Column::TenantId.eq(tenant_id))
        .filter(entity::setting::Column::Key.eq(key))
        .one(db)
        .await
    {
        Ok(Some(row)) => row.value,
        Ok(None) => default,
        Err(e) => {
            tracing::error!("setting lookup failed for {key}: {e}");
            default
        }
    }
}

/// Typed accessor: a boolean setting (false if missing/mistyped).
pub async fn get_bool(db: &impl ConnectionTrait, tenant_id: Uuid, key: &str) -> bool {
    get_value(db, tenant_id, key)
        .await
        .as_bool()
        .unwrap_or(false)
}

/// Typed accessor: an integer setting (0 if missing/mistyped).
pub async fn get_i64(db: &impl ConnectionTrait, tenant_id: Uuid, key: &str) -> i64 {
    get_value(db, tenant_id, key).await.as_i64().unwrap_or(0)
}

/// Validate + upsert a setting override. Rejects unknown keys and type mismatches.
pub async fn set_value(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    key: &str,
    value: Value,
) -> ApiResult<Value> {
    let d = def(key).ok_or_else(|| ApiError::BadRequest(format!("unknown setting: {key}")))?;
    if !d.kind.validate(&value) {
        return Err(ApiError::BadRequest(format!(
            "setting '{key}' expects a {} value",
            d.kind.as_str()
        )));
    }
    let now = Utc::now();
    match Setting::find()
        .filter(entity::setting::Column::TenantId.eq(tenant_id))
        .filter(entity::setting::Column::Key.eq(key))
        .one(db)
        .await?
    {
        Some(row) => {
            let mut am: entity::setting::ActiveModel = row.into();
            am.value = ActiveSet(value.clone());
            am.updated_at = ActiveSet(now.into());
            am.update(db).await?;
        }
        None => {
            entity::setting::ActiveModel {
                id: ActiveSet(Uuid::new_v4()),
                tenant_id: ActiveSet(tenant_id),
                key: ActiveSet(key.to_string()),
                value: ActiveSet(value.clone()),
                updated_at: ActiveSet(now.into()),
            }
            .insert(db)
            .await?;
        }
    }
    Ok(value)
}
