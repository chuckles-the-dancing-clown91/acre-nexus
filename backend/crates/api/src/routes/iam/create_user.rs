use super::dto::{CreateUserReq, UserDetail};
use super::helpers::{add_membership_inner, load_user_detail, upsert_profile_inner};
use crate::auth::{hash_password, random_secret, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::rbac::{self, Permission};
use crate::state::AppState;
use chrono::Utc;
use entity::prelude::*;
use rocket::post;
use rocket::serde::json::Json;
use rocket::State;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait};
use uuid::Uuid;

/// `POST /admin/users` — create an account, optionally with a persona + profile.
#[rocket_okapi::openapi(tag = "IAM")]
#[post("/admin/users", data = "<body>")]
pub async fn create_user(
    state: &State<AppState>,
    user: AuthUser,
    body: Json<CreateUserReq>,
) -> ApiResult<Json<UserDetail>> {
    user.require(Permission::UserManage)?;
    let body = body.into_inner();
    let email = body.email.trim().to_lowercase();
    if email.is_empty() {
        return Err(ApiError::BadRequest("email is required".into()));
    }
    if User::find()
        .filter(entity::user::Column::Email.eq(email.clone()))
        .one(&state.db)
        .await?
        .is_some()
    {
        return Err(ApiError::Conflict(
            "a user with that email already exists".into(),
        ));
    }

    let (status, pw_plain) = match &body.password {
        Some(p) if !p.is_empty() => ("active".to_string(), p.clone()),
        _ => ("invited".to_string(), random_secret(24)),
    };
    let pw_hash = hash_password(&pw_plain).map_err(ApiError::Internal)?;

    let is_platform = body
        .membership
        .as_ref()
        .map(|m| m.scope == rbac::SCOPE_PLATFORM)
        .unwrap_or(false);
    let primary_tenant = body
        .membership
        .as_ref()
        .filter(|m| m.scope == rbac::SCOPE_TENANT)
        .and_then(|m| m.tenant_id);

    let uid = Uuid::new_v4();
    let txn = state.db.begin().await?;
    entity::user::ActiveModel {
        id: Set(uid),
        tenant_id: Set(primary_tenant),
        email: Set(email),
        username: Set(body.username.clone()),
        password_hash: Set(pw_hash),
        name: Set(body.name.clone()),
        is_platform_staff: Set(is_platform),
        status: Set(status),
        last_login_at: Set(None),
        created_at: Set(Utc::now().into()),
    }
    .insert(&txn)
    .await?;

    if let Some(m) = &body.membership {
        add_membership_inner(&txn, uid, m, true).await?;
    }
    if let Some(p) = &body.profile {
        upsert_profile_inner(&txn, &state.config.pii_key, uid, p).await?;
    }
    txn.commit().await?;

    crate::audit::record(
        &state.db,
        Some(user.user_id),
        crate::audit::actions::USER_CREATE,
        Some("user"),
        Some(uid.to_string()),
        primary_tenant,
        None,
    )
    .await;

    load_user_detail(state, uid).await
}
