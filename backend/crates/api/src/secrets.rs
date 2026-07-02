//! Encrypted **integration-credential storage** (roadmap issue #15).
//!
//! Every external integration needs somewhere safe to keep a credential — an
//! ESP API key, a payment-processor secret, a webhook signing secret. Values
//! are sealed with the same AES-256-GCM primitives as PII ([`crate::pii`]) but
//! under the **dedicated** `SECRETS_ENC_KEY` ([`crate::config`]), so the two
//! blast radii stay independently rotatable.
//!
//! * Writes go through [`store`] (set + rotate share one path) and [`remove`].
//! * Reads go through [`reveal`], **server-side only** — plaintext is never
//!   serialized into an HTTP response; the settings UI sees only `last4`.
//! * A tenant row shadows a platform-wide (`tenant_id IS NULL`) row with the
//!   same key, so platform defaults can be overridden per tenant.

use crate::config::Config;
use crate::pii;
use chrono::Utc;
use entity::prelude::Secret;
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// The outcome of a [`store`] call — the caller audits `secret.set` vs
/// `secret.rotate` accordingly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoreOutcome {
    Created,
    Rotated,
}

/// Encrypt and persist `value` under `(tenant_id, key)`, creating the row or
/// rotating an existing one. Returns the masked `last4` for display plus
/// whether this was a create or a rotate.
pub async fn store(
    db: &impl ConnectionTrait,
    tenant_id: Option<Uuid>,
    key: &str,
    value: &str,
    created_by: Option<Uuid>,
) -> anyhow::Result<(String, StoreOutcome)> {
    let sealed = pii::encrypt(&Config::global().secrets_key, value)?;
    let last4 = pii::last4(value);
    let now = Utc::now();

    let existing = find_row(db, tenant_id, key).await?;
    match existing {
        Some(row) => {
            let mut am: entity::secret::ActiveModel = row.into();
            am.ciphertext = Set(sealed.ciphertext);
            am.nonce = Set(sealed.nonce);
            am.last4 = Set(last4.clone());
            am.rotated_at = Set(Some(now.into()));
            am.update(db).await?;
            Ok((last4, StoreOutcome::Rotated))
        }
        None => {
            entity::secret::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                key: Set(key.to_string()),
                ciphertext: Set(sealed.ciphertext),
                nonce: Set(sealed.nonce),
                last4: Set(last4.clone()),
                created_by: Set(created_by),
                created_at: Set(now.into()),
                rotated_at: Set(None),
            }
            .insert(db)
            .await?;
            Ok((last4, StoreOutcome::Created))
        }
    }
}

/// Decrypt the credential stored under `key` for `tenant_id`, falling back to
/// the platform-wide (`tenant_id IS NULL`) row. **Server-side use only** — the
/// returned plaintext must never be serialized into an API response.
pub async fn reveal(
    db: &impl ConnectionTrait,
    tenant_id: Option<Uuid>,
    key: &str,
) -> anyhow::Result<Option<String>> {
    let row = match find_row(db, tenant_id, key).await? {
        Some(r) => Some(r),
        // Fall back to the platform-wide default when the tenant has no row.
        None if tenant_id.is_some() => find_row(db, None, key).await?,
        None => None,
    };
    match row {
        Some(r) => {
            let value = pii::decrypt(&Config::global().secrets_key, &r.ciphertext, &r.nonce)?;
            Ok(Some(value))
        }
        None => Ok(None),
    }
}

/// Delete the credential stored under `(tenant_id, key)`. Returns whether a row
/// existed.
pub async fn remove(
    db: &impl ConnectionTrait,
    tenant_id: Option<Uuid>,
    key: &str,
) -> anyhow::Result<bool> {
    match find_row(db, tenant_id, key).await? {
        Some(row) => {
            let am: entity::secret::ActiveModel = row.into();
            am.delete(db).await?;
            Ok(true)
        }
        None => Ok(false),
    }
}

async fn find_row(
    db: &impl ConnectionTrait,
    tenant_id: Option<Uuid>,
    key: &str,
) -> anyhow::Result<Option<entity::secret::Model>> {
    let mut q = Secret::find().filter(entity::secret::Column::Key.eq(key));
    q = match tenant_id {
        Some(t) => q.filter(entity::secret::Column::TenantId.eq(t)),
        None => q.filter(entity::secret::Column::TenantId.is_null()),
    };
    Ok(q.one(db).await?)
}
