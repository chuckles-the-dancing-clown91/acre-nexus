use super::dto::{CreateTokenReq, CreateTokenResp, TokenSummary};
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use crate::tokens;
use chrono::Utc;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, Set};
use uuid::Uuid;

/// `POST /api-tokens` — mint a new scoped API token.
#[rocket_okapi::openapi(tag = "API Tokens")]
#[post("/api-tokens", data = "<body>")]
pub async fn create(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
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
    let saved = model.insert(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::TOKEN_CREATE,
        Some("api_token"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "name": saved.name, "scopes": saved.scopes })),
    )
    .await;
    Ok(Json(CreateTokenResp {
        summary: TokenSummary::from(saved),
        token: minted.raw,
    }))
}
