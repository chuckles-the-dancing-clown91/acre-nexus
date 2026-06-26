//! Vendor API-token management. Tenants create scoped keys to access the
//! `/api/v1` vendor API or resell services. The raw secret is returned **once**.

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use crate::tokens;
use chrono::Utc;
use entity::prelude::ApiToken;
use rocket::serde::json::Json;
use rocket::{delete, get, post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct TokenSummary {
    pub id: Uuid,
    pub name: String,
    pub prefix: String,
    pub scopes: Vec<String>,
    pub last_used_at: Option<String>,
    pub revoked: bool,
    pub created_at: String,
}

impl From<entity::api_token::Model> for TokenSummary {
    fn from(t: entity::api_token::Model) -> Self {
        TokenSummary {
            id: t.id,
            name: t.name,
            prefix: t.prefix,
            scopes: serde_json::from_value(t.scopes).unwrap_or_default(),
            last_used_at: t.last_used_at.map(|d| d.to_rfc3339()),
            revoked: t.revoked_at.is_some(),
            created_at: t.created_at.to_rfc3339(),
        }
    }
}

/// `GET /api-tokens` — list the active tenant's API tokens (no secrets).
#[rocket_okapi::openapi(tag = "API Tokens")]
#[get("/api-tokens")]
pub async fn list(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<TokenSummary>>> {
    user.require(Permission::ApiTokenManage)?;
    let rows = ApiToken::find()
        .filter(entity::api_token::Column::TenantId.eq(scope.tenant_id))
        .order_by_desc(entity::api_token::Column::CreatedAt)
        .all(&state.db)
        .await?;
    Ok(Json(rows.into_iter().map(TokenSummary::from).collect()))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateTokenReq {
    pub name: String,
    /// Permission scopes, e.g. `["listing:read","property:read"]`.
    pub scopes: Vec<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct CreateTokenResp {
    #[serde(flatten)]
    pub summary: TokenSummary,
    /// The raw secret — shown exactly once, store it securely.
    pub token: String,
}

/// `POST /api-tokens` — mint a new scoped API token.
#[rocket_okapi::openapi(tag = "API Tokens")]
#[post("/api-tokens", data = "<body>")]
pub async fn create(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    body: Json<CreateTokenReq>,
) -> ApiResult<Json<CreateTokenResp>> {
    user.require(Permission::ApiTokenManage)?;
    let b = body.into_inner();
    let minted = tokens::mint();
    let now = Utc::now();
    let model = entity::api_token::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        name: Set(b.name),
        prefix: Set(minted.prefix.clone()),
        token_hash: Set(minted.hash),
        scopes: Set(serde_json::to_value(&b.scopes).unwrap_or_default()),
        last_used_at: Set(None),
        expires_at: Set(None),
        revoked_at: Set(None),
        created_at: Set(now.into()),
    };
    let saved = model.insert(&state.db).await?;
    Ok(Json(CreateTokenResp {
        summary: TokenSummary::from(saved),
        token: minted.raw,
    }))
}

/// `DELETE /api-tokens/<id>` — revoke a token immediately.
#[rocket_okapi::openapi(tag = "API Tokens")]
#[delete("/api-tokens/<id>")]
pub async fn revoke(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::ApiTokenManage)?;
    let tid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let t = ApiToken::find_by_id(tid)
        .filter(entity::api_token::Column::TenantId.eq(scope.tenant_id))
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("token not found".into()))?;
    let mut am: entity::api_token::ActiveModel = t.into();
    am.revoked_at = Set(Some(Utc::now().into()));
    am.update(&state.db).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
