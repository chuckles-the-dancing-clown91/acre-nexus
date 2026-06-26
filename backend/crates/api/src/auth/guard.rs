use crate::error::ApiError;
use crate::rbac::{Grants, Permission};
use crate::state::AppState;
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};
use uuid::Uuid;

use super::jwt::decode_access_token;

/// An authenticated human principal, extracted from a `Bearer` JWT.
#[derive(Clone, Debug)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub tenant_id: Option<Uuid>,
    pub is_staff: bool,
    pub grants: Grants,
}

impl AuthUser {
    /// Assert the principal holds a permission, else `403`.
    pub fn require(&self, p: Permission) -> Result<(), ApiError> {
        self.require_key(p.as_str())
    }

    /// Assert the principal holds a string-keyed permission (built-in or custom).
    pub fn require_key(&self, key: &str) -> Result<(), ApiError> {
        if self.grants.has_key(key) {
            Ok(())
        } else {
            Err(ApiError::Forbidden(format!("missing permission: {key}")))
        }
    }
}

fn bearer(req: &Request<'_>) -> Option<String> {
    req.headers()
        .get_one("Authorization")
        .and_then(|h| h.strip_prefix("Bearer "))
        .map(|s| s.to_string())
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthUser {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let state = match req.rocket().state::<AppState>() {
            Some(s) => s,
            None => return Outcome::Error((Status::InternalServerError, ())),
        };
        let token = match bearer(req) {
            Some(t) => t,
            None => return Outcome::Error((Status::Unauthorized, ())),
        };
        match decode_access_token(&state.config, &token) {
            Some(c) => Outcome::Success(AuthUser {
                user_id: c.sub,
                tenant_id: c.tid,
                is_staff: c.staff,
                grants: Grants::from_iter(c.perms),
            }),
            None => Outcome::Error((Status::Unauthorized, ())),
        }
    }
}
