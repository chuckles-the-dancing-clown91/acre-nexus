//! Token-based authentication for the **vendor API** (`/api/v1/...`).
//!
//! Vendors authenticate with a long-lived, scoped, revocable key
//! (`acre_live_<secret>`). Only a SHA-256 hash is stored; scope checks gate
//! access to individual resources so services can be sold à la carte.

use crate::auth::{hash_secret, random_secret};
use crate::error::ApiError;
use crate::rbac::Permission;
use crate::state::AppState;
use chrono::Utc;
use entity::prelude::ApiToken;
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// Prefix that distinguishes a vendor key from a JWT in the `Authorization` header.
pub const TOKEN_PREFIX: &str = "acre_live_";

/// A freshly minted token — the raw secret is returned to the caller exactly once.
pub struct MintedToken {
    pub raw: String,
    pub prefix: String,
    pub hash: String,
}

/// Mint a new vendor token secret + its stored hash.
pub fn mint() -> MintedToken {
    let secret = random_secret(24);
    let raw = format!("{TOKEN_PREFIX}{secret}");
    // Visible prefix shown in dashboards for identification.
    let prefix = format!("{TOKEN_PREFIX}{}", &secret[..6]);
    let hash = hash_secret(&raw);
    MintedToken { raw, prefix, hash }
}

/// An authenticated vendor principal derived from a valid API token.
#[derive(Clone, Debug)]
pub struct ApiPrincipal {
    pub token_id: Uuid,
    pub tenant_id: Uuid,
    pub scopes: Vec<String>,
}

impl ApiPrincipal {
    /// Assert the token carries a scope, else `403`.
    pub fn require(&self, p: Permission) -> Result<(), ApiError> {
        if self.scopes.iter().any(|s| s == p.as_str()) {
            Ok(())
        } else {
            Err(ApiError::Forbidden(format!(
                "token missing scope: {}",
                p.as_str()
            )))
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for ApiPrincipal {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let state = match req.rocket().state::<AppState>() {
            Some(s) => s,
            None => return Outcome::Error((Status::InternalServerError, ())),
        };
        // Accept "Authorization: Bearer acre_live_..." or "X-Api-Key: acre_live_...".
        let raw = req
            .headers()
            .get_one("Authorization")
            .and_then(|h| h.strip_prefix("Bearer "))
            .map(|s| s.to_string())
            .or_else(|| req.headers().get_one("X-Api-Key").map(|s| s.to_string()));

        let raw = match raw {
            Some(r) if r.starts_with(TOKEN_PREFIX) => r,
            _ => return Outcome::Error((Status::Unauthorized, ())),
        };

        let hash = hash_secret(&raw);
        let found = ApiToken::find()
            .filter(entity::api_token::Column::TokenHash.eq(hash))
            .one(&state.db)
            .await;

        let model = match found {
            Ok(Some(m)) => m,
            _ => return Outcome::Error((Status::Unauthorized, ())),
        };

        // Reject revoked / expired tokens.
        let now = Utc::now();
        if model.revoked_at.is_some() {
            return Outcome::Error((Status::Unauthorized, ()));
        }
        if let Some(exp) = model.expires_at {
            if exp < now {
                return Outcome::Error((Status::Unauthorized, ()));
            }
        }

        let scopes: Vec<String> = serde_json::from_value(model.scopes.clone()).unwrap_or_default();

        // Best-effort last-used stamp (ignore failures).
        let mut am: entity::api_token::ActiveModel = model.clone().into();
        am.last_used_at = Set(Some(now.into()));
        let _ = am.update(&state.db).await;

        Outcome::Success(ApiPrincipal {
            token_id: model.id,
            tenant_id: model.tenant_id,
            scopes,
        })
    }
}
