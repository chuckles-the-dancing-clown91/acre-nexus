//! Resolving the **principal** behind a request, for the audit fairing.
//!
//! Mirrors the auth guards' header handling but without failing the request: a
//! best-effort decode of the JWT or API key so the audit entry can attribute the
//! action. Falls back to an anonymous `public` actor when no credentials are
//! presented (or they're invalid).

use crate::auth;
use crate::state::AppState;
use crate::tokens::TOKEN_PREFIX;
use entity::prelude::ApiToken;
use rocket::request::Request;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

/// Who is behind a request, resolved for auditing.
#[derive(Clone, Debug)]
pub struct ResolvedActor {
    /// The acting user, for JWT-authenticated requests.
    pub user_id: Option<Uuid>,
    /// Workspace/tenant context, when derivable from the credential.
    pub tenant_id: Option<Uuid>,
    /// `user`, `api_token`, or `public`.
    pub kind: &'static str,
}

impl ResolvedActor {
    /// An unauthenticated / anonymous principal.
    pub fn public() -> Self {
        ResolvedActor {
            user_id: None,
            tenant_id: None,
            kind: "public",
        }
    }
}

/// Best-effort resolution of the principal from the request headers.
pub async fn resolve(req: &Request<'_>, state: &AppState) -> ResolvedActor {
    // `Authorization: Bearer …` carries either a vendor API key or a JWT.
    if let Some(value) = req
        .headers()
        .get_one("Authorization")
        .and_then(|h| h.strip_prefix("Bearer "))
    {
        if value.starts_with(TOKEN_PREFIX) {
            return resolve_api_token(state, value).await;
        }
        if let Some(claims) = auth::decode_access_token(&state.config, value) {
            return ResolvedActor {
                user_id: Some(claims.sub),
                tenant_id: claims.tid,
                kind: "user",
            };
        }
    }

    // `X-Api-Key: acre_live_…` is the alternative vendor-key header.
    if let Some(key) = req.headers().get_one("X-Api-Key") {
        if key.starts_with(TOKEN_PREFIX) {
            return resolve_api_token(state, key).await;
        }
    }

    ResolvedActor::public()
}

/// Resolve a vendor API key to its tenant (no user). Unknown keys are recorded as
/// anonymous so a probing request is still logged.
async fn resolve_api_token(state: &AppState, raw: &str) -> ResolvedActor {
    let hash = auth::hash_secret(raw);
    match ApiToken::find()
        .filter(entity::api_token::Column::TokenHash.eq(hash))
        .one(&state.db)
        .await
    {
        Ok(Some(token)) => ResolvedActor {
            user_id: None,
            tenant_id: Some(token.tenant_id),
            kind: "api_token",
        },
        _ => ResolvedActor::public(),
    }
}
