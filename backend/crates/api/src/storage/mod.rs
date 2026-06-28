//! # Multi-provider object storage
//!
//! Uploaded logos and LLC documents are stored as objects in a backend that is
//! **configurable per tenant**: the platform-managed default (set via `STORAGE_*`
//! env) *or* a tenant's own bucket — local filesystem, AWS S3 (and S3-compatible
//! stores like MinIO / R2), or Google Cloud Storage. One [`object_store`] API
//! spans all of them.
//!
//! A tenant's choice lives in `tenant_storage_config`; bring-your-own credentials
//! are sealed at rest (AES-256-GCM, the same scheme as PII) and decrypted only
//! here when building the client.
//!
//! Object **keys** are content paths like
//! `tenants/<tenant>/llc/<llc>/<uuid>.<ext>`, optionally under a configured
//! prefix. The full key is persisted on the document row, and the bytes are read
//! back by re-resolving the tenant's store and fetching that key.

use crate::config::StorageSettings;
use crate::state::AppState;
use anyhow::{anyhow, Context};
use bytes::Bytes;
use object_store::path::Path as ObjPath;
use object_store::{ObjectStore, PutPayload};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use std::sync::Arc;
use uuid::Uuid;

/// A storage backend resolved for one tenant, plus the key prefix to apply when
/// minting new object keys.
pub struct ResolvedStore {
    /// Where bytes physically live, recorded on the document row: `platform` |
    /// `local` | `s3` | `gcs`.
    pub provider_label: String,
    key_prefix: String,
    store: Arc<dyn ObjectStore>,
}

impl ResolvedStore {
    /// Build the full object key for a freshly uploaded relative path, applying
    /// the configured prefix. Persist the returned key; reads pass it verbatim.
    pub fn object_key(&self, rel: &str) -> String {
        if self.key_prefix.is_empty() {
            rel.to_string()
        } else {
            format!("{}/{}", self.key_prefix.trim_end_matches('/'), rel)
        }
    }

    /// Write `bytes` at the (already-prefixed) `key`.
    pub async fn put(&self, key: &str, bytes: Vec<u8>) -> anyhow::Result<()> {
        let payload = PutPayload::from(Bytes::from(bytes));
        self.store
            .put(&ObjPath::from(key), payload)
            .await
            .with_context(|| format!("storage put failed for {key}"))?;
        Ok(())
    }

    /// Read the bytes at `key`.
    pub async fn get(&self, key: &str) -> anyhow::Result<Vec<u8>> {
        let res = self
            .store
            .get(&ObjPath::from(key))
            .await
            .with_context(|| format!("storage get failed for {key}"))?;
        Ok(res.bytes().await?.to_vec())
    }

    /// Best-effort delete; a missing object is not an error.
    pub async fn delete(&self, key: &str) -> anyhow::Result<()> {
        let _ = self.store.delete(&ObjPath::from(key)).await;
        Ok(())
    }
}

/// Resolve the storage backend for `tenant_id`: the tenant's own configuration if
/// present and not `platform`, otherwise the platform-managed default.
pub async fn resolve_for_tenant(state: &AppState, tenant_id: Uuid) -> anyhow::Result<ResolvedStore> {
    let cfg = entity::prelude::TenantStorageConfig::find()
        .filter(entity::tenant_storage_config::Column::TenantId.eq(tenant_id))
        .one(&state.user_db)
        .await?;

    match cfg {
        Some(c) if c.provider != "platform" => build_byo(state, &c),
        _ => build_platform(&state.config.storage),
    }
}

/// Build the platform-managed default store from `STORAGE_*` settings.
fn build_platform(s: &StorageSettings) -> anyhow::Result<ResolvedStore> {
    let (store, prefix): (Arc<dyn ObjectStore>, String) = match s.provider.as_str() {
        "s3" => (
            Arc::new(build_s3(
                s.bucket.clone(),
                s.region.clone(),
                s.endpoint.clone(),
                s.access_key_id.clone(),
                s.secret_access_key.clone(),
                s.allow_http,
            )?),
            s.prefix.clone().unwrap_or_default(),
        ),
        "gcs" => (
            Arc::new(build_gcs(s.bucket.clone(), s.gcs_service_account_json.clone())?),
            s.prefix.clone().unwrap_or_default(),
        ),
        _ => (Arc::new(build_local(&s.local_path)?), String::new()),
    };
    Ok(ResolvedStore {
        provider_label: "platform".into(),
        key_prefix: prefix,
        store,
    })
}

/// Build a tenant's bring-your-own store, decrypting any sealed credentials.
fn build_byo(
    state: &AppState,
    c: &entity::tenant_storage_config::Model,
) -> anyhow::Result<ResolvedStore> {
    let secret = match (&c.secret_ciphertext, &c.secret_nonce) {
        (Some(ct), Some(nonce)) => Some(crate::pii::decrypt(&state.config.pii_key, ct, nonce)?),
        _ => None,
    };

    let (store, prefix): (Arc<dyn ObjectStore>, String) = match c.provider.as_str() {
        "s3" => {
            let (access, secret_key) = parse_s3_secret(secret.as_deref())?;
            (
                Arc::new(build_s3(
                    c.bucket.clone(),
                    c.region.clone(),
                    c.endpoint.clone(),
                    access,
                    secret_key,
                    false,
                )?),
                c.prefix.clone().unwrap_or_default(),
            )
        }
        "gcs" => (
            Arc::new(build_gcs(c.bucket.clone(), secret)?),
            c.prefix.clone().unwrap_or_default(),
        ),
        "local" => {
            let base = c.prefix.clone().unwrap_or_else(|| "./.storage".into());
            (Arc::new(build_local(&base)?), String::new())
        }
        other => return Err(anyhow!("unknown storage provider '{other}'")),
    };

    Ok(ResolvedStore {
        provider_label: c.provider.clone(),
        key_prefix: prefix,
        store,
    })
}

fn build_local(path: &str) -> anyhow::Result<object_store::local::LocalFileSystem> {
    std::fs::create_dir_all(path).with_context(|| format!("create storage dir {path}"))?;
    object_store::local::LocalFileSystem::new_with_prefix(path)
        .with_context(|| format!("open local store at {path}"))
}

fn build_s3(
    bucket: Option<String>,
    region: Option<String>,
    endpoint: Option<String>,
    access_key_id: Option<String>,
    secret_access_key: Option<String>,
    allow_http: bool,
) -> anyhow::Result<object_store::aws::AmazonS3> {
    let mut b = object_store::aws::AmazonS3Builder::new()
        .with_bucket_name(bucket.ok_or_else(|| anyhow!("s3 storage requires a bucket"))?);
    if let Some(r) = region {
        b = b.with_region(r);
    }
    if let Some(e) = endpoint {
        b = b.with_endpoint(e);
    }
    if let Some(k) = access_key_id {
        b = b.with_access_key_id(k);
    }
    if let Some(s) = secret_access_key {
        b = b.with_secret_access_key(s);
    }
    if allow_http {
        b = b.with_allow_http(true);
    }
    Ok(b.build()?)
}

fn build_gcs(
    bucket: Option<String>,
    service_account_json: Option<String>,
) -> anyhow::Result<object_store::gcp::GoogleCloudStorage> {
    let mut b = object_store::gcp::GoogleCloudStorageBuilder::new()
        .with_bucket_name(bucket.ok_or_else(|| anyhow!("gcs storage requires a bucket"))?);
    if let Some(json) = service_account_json {
        b = b.with_service_account_key(json);
    }
    Ok(b.build()?)
}

/// A BYO S3 secret blob is `{"access_key_id":"…","secret_access_key":"…"}`.
fn parse_s3_secret(secret: Option<&str>) -> anyhow::Result<(Option<String>, Option<String>)> {
    match secret {
        None => Ok((None, None)),
        Some(raw) => {
            let v: serde_json::Value =
                serde_json::from_str(raw).context("s3 credential blob is not valid JSON")?;
            let access = v
                .get("access_key_id")
                .and_then(|x| x.as_str())
                .map(String::from);
            let secret_key = v
                .get("secret_access_key")
                .and_then(|x| x.as_str())
                .map(String::from);
            Ok((access, secret_key))
        }
    }
}
