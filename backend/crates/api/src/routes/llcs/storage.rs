//! Per-tenant **storage configuration** endpoints. A workspace can keep the
//! platform-managed default or point document storage at its own Local / S3 / GCS
//! bucket. Credentials are sealed (AES-256-GCM) before persistence and never
//! returned.

use super::dto::{StorageConfigDto, UpdateStorageConfigReq};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::TenantStorageConfig;
use rocket::serde::json::Json;
use rocket::{get, put, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

const PROVIDERS: &[&str] = &["platform", "local", "s3", "gcs"];

fn to_dto(row: Option<entity::tenant_storage_config::Model>) -> StorageConfigDto {
    match row {
        Some(c) => StorageConfigDto {
            provider: c.provider,
            bucket: c.bucket,
            region: c.region,
            prefix: c.prefix,
            endpoint: c.endpoint,
            has_credentials: c.secret_ciphertext.is_some(),
            is_default: false,
        },
        None => StorageConfigDto {
            provider: "platform".into(),
            bucket: None,
            region: None,
            prefix: None,
            endpoint: None,
            has_credentials: false,
            is_default: true,
        },
    }
}

/// `GET /storage/config` — the workspace's storage backend (platform default if unset).
#[rocket_okapi::openapi(tag = "Settings")]
#[get("/storage/config")]
pub async fn get_storage_config(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<StorageConfigDto>> {
    user.require(Permission::StorageManage)?;
    let row = TenantStorageConfig::find()
        .filter(entity::tenant_storage_config::Column::TenantId.eq(scope.tenant_id))
        .one(&state.user_db)
        .await?;
    Ok(Json(to_dto(row)))
}

/// `PUT /storage/config` — configure the workspace's storage backend.
#[rocket_okapi::openapi(tag = "Settings")]
#[put("/storage/config", data = "<body>")]
pub async fn put_storage_config(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    body: Json<UpdateStorageConfigReq>,
) -> ApiResult<Json<StorageConfigDto>> {
    user.require(Permission::StorageManage)?;
    let req = body.into_inner();
    if !PROVIDERS.contains(&req.provider.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "unknown provider '{}'; expected one of platform|local|s3|gcs",
            req.provider
        )));
    }

    // Seal a freshly provided credential blob, if any.
    let sealed = match req.secret.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(secret) => Some(
            crate::pii::encrypt(&state.config.pii_key, secret)
                .map_err(ApiError::Internal)?,
        ),
        None => None,
    };

    let existing = TenantStorageConfig::find()
        .filter(entity::tenant_storage_config::Column::TenantId.eq(scope.tenant_id))
        .one(&state.user_db)
        .await?;
    let now = Utc::now();

    let saved = match existing {
        Some(e) => {
            let mut am: entity::tenant_storage_config::ActiveModel = e.into();
            am.provider = Set(req.provider.clone());
            am.bucket = Set(req.bucket.clone());
            am.region = Set(req.region.clone());
            am.prefix = Set(req.prefix.clone());
            am.endpoint = Set(req.endpoint.clone());
            // Only replace the sealed secret when a new one was supplied.
            if let Some(s) = &sealed {
                am.secret_ciphertext = Set(Some(s.ciphertext.clone()));
                am.secret_nonce = Set(Some(s.nonce.clone()));
            }
            am.updated_at = Set(now.into());
            am.update(&state.user_db).await?
        }
        None => entity::tenant_storage_config::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(scope.tenant_id),
            provider: Set(req.provider.clone()),
            bucket: Set(req.bucket.clone()),
            region: Set(req.region.clone()),
            prefix: Set(req.prefix.clone()),
            endpoint: Set(req.endpoint.clone()),
            secret_ciphertext: Set(sealed.as_ref().map(|s| s.ciphertext.clone())),
            secret_nonce: Set(sealed.as_ref().map(|s| s.nonce.clone())),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&state.user_db)
        .await?,
    };

    crate::audit::record(
        &state.user_db,
        Some(user.user_id),
        crate::audit::actions::STORAGE_CONFIG_UPDATE,
        Some("tenant_storage_config"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "provider": saved.provider })),
    )
    .await;

    Ok(Json(to_dto(Some(saved))))
}
