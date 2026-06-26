//! Identity & Access Management routes — the back-end for the Acre employee
//! dashboard and for client workspace member management.
//!
//! Two audiences share this code:
//! * **Acre staff** operate `/admin/*` (gated by `user:*`, `role:*`,
//!   `profile:*`, `member:manage`; platform admins hold all). They manage users,
//!   profiles (incl. sensitive PII), personas, roles, and permissions across any
//!   tenant.
//! * **Client admins** operate `/members*` scoped to their active tenant
//!   (gated by `member:manage` / `member:read`) to run their own landlords,
//!   back-office staff, leasing agents, etc.
//!
//! Roles and their permission grants are stored in the DB, so everything here is
//! editable at runtime — no redeploy to add a role or change a permission.

use crate::auth::{hash_password, random_secret, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::pii;
use crate::rbac::{self, Permission};
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::{NaiveDate, Utc};
use entity::prelude::*;
use rocket::serde::json::Json;
use rocket::{delete, get, patch, post, put, State};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ===========================================================================
// Catalogs
// ===========================================================================

#[derive(Serialize, schemars::JsonSchema)]
pub struct PermissionDto {
    pub key: String,
    pub category: String,
    pub label: String,
    pub description: String,
    pub scope: String,
}

/// `GET /admin/permissions` — the permission catalog (for the role editor).
#[rocket_okapi::openapi(tag = "IAM")]
#[get("/admin/permissions")]
pub async fn permissions(
    state: &State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Vec<PermissionDto>>> {
    user.require(Permission::RoleRead)?;
    let rows = entity::permission::Entity::find()
        .order_by_asc(entity::permission::Column::Category)
        .order_by_asc(entity::permission::Column::Key)
        .all(&state.db)
        .await?;
    Ok(Json(
        rows.into_iter()
            .map(|p| PermissionDto {
                key: p.key,
                category: p.category,
                label: p.label,
                description: p.description,
                scope: p.scope,
            })
            .collect(),
    ))
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ProfileTypeDto {
    pub key: String,
    pub scope: String,
    pub label: String,
    pub description: String,
    pub default_role: String,
}

/// `GET /admin/profile-types` — the persona catalog.
#[rocket_okapi::openapi(tag = "IAM")]
#[get("/admin/profile-types")]
pub async fn profile_types(
    state: &State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Vec<ProfileTypeDto>>> {
    user.require(Permission::MemberRead)?;
    let rows = entity::profile_type::Entity::find()
        .order_by_asc(entity::profile_type::Column::Scope)
        .all(&state.db)
        .await?;
    Ok(Json(
        rows.into_iter()
            .map(|p| ProfileTypeDto {
                key: p.key,
                scope: p.scope,
                label: p.label,
                description: p.description,
                default_role: p.default_role,
            })
            .collect(),
    ))
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct AuditEntry {
    pub id: Uuid,
    pub actor_user_id: Option<Uuid>,
    pub actor_name: Option<String>,
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub tenant_id: Option<Uuid>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: String,
}

/// `GET /admin/audit?limit=&action=` — recent security audit entries, newest
/// first, with the actor's display name resolved.
#[rocket_okapi::openapi(tag = "IAM")]
#[get("/admin/audit?<limit>&<action>")]
pub async fn list_audit(
    state: &State<AppState>,
    user: AuthUser,
    limit: Option<u64>,
    action: Option<String>,
) -> ApiResult<Json<Vec<AuditEntry>>> {
    user.require(Permission::AuditRead)?;
    let mut q = AuditLog::find().order_by_desc(entity::audit_log::Column::CreatedAt);
    if let Some(a) = action.filter(|s| !s.is_empty()) {
        q = q.filter(entity::audit_log::Column::Action.eq(a));
    }
    let rows = q
        .limit(limit.unwrap_or(100).min(500))
        .all(&state.db)
        .await?;
    let mut out = Vec::new();
    for r in rows {
        let actor_name = match r.actor_user_id {
            Some(aid) => User::find_by_id(aid).one(&state.db).await?.map(|u| u.name),
            None => None,
        };
        out.push(AuditEntry {
            id: r.id,
            actor_user_id: r.actor_user_id,
            actor_name,
            action: r.action,
            target_type: r.target_type,
            target_id: r.target_id,
            tenant_id: r.tenant_id,
            metadata: r.metadata,
            created_at: r.created_at.to_rfc3339(),
        });
    }
    Ok(Json(out))
}

// ===========================================================================
// Roles
// ===========================================================================

#[derive(Serialize, schemars::JsonSchema)]
pub struct RoleDto {
    pub id: Uuid,
    pub scope: String,
    pub tenant_id: Option<Uuid>,
    pub key: String,
    pub name: String,
    pub description: String,
    pub is_system: bool,
    pub permissions: Vec<String>,
}

async fn role_permissions(
    db: &sea_orm::DatabaseConnection,
    role_id: Uuid,
) -> Result<Vec<String>, ApiError> {
    Ok(RolePermission::find()
        .filter(entity::role_permission::Column::RoleId.eq(role_id))
        .all(db)
        .await?
        .into_iter()
        .map(|p| p.permission)
        .collect())
}

async fn replace_role_permissions(
    db: &sea_orm::DatabaseConnection,
    role_id: Uuid,
    perms: &[String],
) -> Result<(), ApiError> {
    RolePermission::delete_many()
        .filter(entity::role_permission::Column::RoleId.eq(role_id))
        .exec(db)
        .await?;
    for p in perms {
        entity::role_permission::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            role_id: Set(role_id),
            permission: Set(p.clone()),
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

/// `GET /admin/roles?tenant_id=&scope=` — roles (system + custom).
#[rocket_okapi::openapi(tag = "IAM")]
#[get("/admin/roles?<tenant_id>&<scope>")]
pub async fn list_roles(
    state: &State<AppState>,
    user: AuthUser,
    tenant_id: Option<String>,
    scope: Option<String>,
) -> ApiResult<Json<Vec<RoleDto>>> {
    user.require(Permission::RoleRead)?;
    let mut q = Role::find();
    if let Some(s) = &scope {
        q = q.filter(entity::role::Column::Scope.eq(s.clone()));
    }
    if let Some(tid) = tenant_id.as_deref().and_then(|s| Uuid::parse_str(s).ok()) {
        q = q.filter(
            entity::role::Column::TenantId
                .eq(tid)
                .or(entity::role::Column::TenantId.is_null()),
        );
    }
    let roles = q
        .order_by_asc(entity::role::Column::Name)
        .all(&state.db)
        .await?;
    let mut out = Vec::new();
    for r in roles {
        let perms = role_permissions(&state.db, r.id).await?;
        out.push(RoleDto {
            id: r.id,
            scope: r.scope,
            tenant_id: r.tenant_id,
            key: r.key,
            name: r.name,
            description: r.description,
            is_system: r.is_system,
            permissions: perms,
        });
    }
    Ok(Json(out))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateRoleReq {
    pub scope: String,
    pub tenant_id: Option<Uuid>,
    pub key: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub permissions: Vec<String>,
}

/// `POST /admin/roles` — create a custom role with a permission set.
#[rocket_okapi::openapi(tag = "IAM")]
#[post("/admin/roles", data = "<body>")]
pub async fn create_role(
    state: &State<AppState>,
    user: AuthUser,
    body: Json<CreateRoleReq>,
) -> ApiResult<Json<RoleDto>> {
    user.require(Permission::RoleManage)?;
    let body = body.into_inner();
    if body.scope != rbac::SCOPE_PLATFORM && body.scope != rbac::SCOPE_TENANT {
        return Err(ApiError::BadRequest(
            "scope must be 'platform' or 'tenant'".into(),
        ));
    }
    validate_permissions(&body.permissions)?;
    let id = Uuid::new_v4();
    entity::role::ActiveModel {
        id: Set(id),
        tenant_id: Set(body.tenant_id),
        scope: Set(body.scope.clone()),
        key: Set(body.key.clone()),
        name: Set(body.name.clone()),
        description: Set(body.description.clone()),
        is_system: Set(false),
    }
    .insert(&state.db)
    .await?;
    replace_role_permissions(&state.db, id, &body.permissions).await?;
    crate::audit::record(
        &state.db,
        Some(user.user_id),
        "role.create",
        Some("role"),
        Some(id.to_string()),
        body.tenant_id,
        Some(serde_json::json!({ "key": body.key, "permissions": body.permissions.len() })),
    )
    .await;
    Ok(Json(RoleDto {
        id,
        scope: body.scope,
        tenant_id: body.tenant_id,
        key: body.key,
        name: body.name,
        description: body.description,
        is_system: false,
        permissions: body.permissions,
    }))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateRoleReq {
    pub name: Option<String>,
    pub description: Option<String>,
    /// When present, fully replaces the role's permission set.
    pub permissions: Option<Vec<String>>,
}

/// `PATCH /admin/roles/<id>` — rename / re-describe and/or replace permissions.
#[rocket_okapi::openapi(tag = "IAM")]
#[patch("/admin/roles/<id>", data = "<body>")]
pub async fn update_role(
    state: &State<AppState>,
    user: AuthUser,
    id: &str,
    body: Json<UpdateRoleReq>,
) -> ApiResult<Json<RoleDto>> {
    user.require(Permission::RoleManage)?;
    let rid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid role id".into()))?;
    let role = Role::find_by_id(rid)
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("role not found".into()))?;
    let body = body.into_inner();

    let mut am: entity::role::ActiveModel = role.clone().into();
    if let Some(name) = body.name.clone() {
        am.name = Set(name);
    }
    if let Some(desc) = body.description.clone() {
        am.description = Set(desc);
    }
    am.update(&state.db).await?;

    if let Some(perms) = &body.permissions {
        validate_permissions(perms)?;
        replace_role_permissions(&state.db, rid, perms).await?;
    }

    crate::audit::record(
        &state.db,
        Some(user.user_id),
        "role.update",
        Some("role"),
        Some(rid.to_string()),
        role.tenant_id,
        None,
    )
    .await;
    let updated = Role::find_by_id(rid).one(&state.db).await?.unwrap();
    let perms = role_permissions(&state.db, rid).await?;
    Ok(Json(RoleDto {
        id: updated.id,
        scope: updated.scope,
        tenant_id: updated.tenant_id,
        key: updated.key,
        name: updated.name,
        description: updated.description,
        is_system: updated.is_system,
        permissions: perms,
    }))
}

/// `DELETE /admin/roles/<id>` — delete a custom role (system roles are protected).
#[rocket_okapi::openapi(tag = "IAM")]
#[delete("/admin/roles/<id>")]
pub async fn delete_role(
    state: &State<AppState>,
    user: AuthUser,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::RoleManage)?;
    let rid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid role id".into()))?;
    let role = Role::find_by_id(rid)
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("role not found".into()))?;
    if role.is_system {
        return Err(ApiError::Forbidden("system roles cannot be deleted".into()));
    }
    RolePermission::delete_many()
        .filter(entity::role_permission::Column::RoleId.eq(rid))
        .exec(&state.db)
        .await?;
    UserRole::delete_many()
        .filter(entity::user_role::Column::RoleId.eq(rid))
        .exec(&state.db)
        .await?;
    Role::delete_by_id(rid).exec(&state.db).await?;
    crate::audit::record(
        &state.db,
        Some(user.user_id),
        "role.delete",
        Some("role"),
        Some(rid.to_string()),
        role.tenant_id,
        None,
    )
    .await;
    Ok(Json(serde_json::json!({ "deleted": true })))
}

/// Reject permission keys not present in the catalog (keeps roles coherent).
fn validate_permissions(perms: &[String]) -> Result<(), ApiError> {
    let known: std::collections::HashSet<&str> =
        rbac::PERMISSION_CATALOG.iter().map(|p| p.key).collect();
    for p in perms {
        if !known.contains(p.as_str()) {
            return Err(ApiError::BadRequest(format!("unknown permission: {p}")));
        }
    }
    Ok(())
}

// ===========================================================================
// Users + profiles + memberships
// ===========================================================================

#[derive(Serialize, schemars::JsonSchema)]
pub struct UserListItem {
    pub id: Uuid,
    pub email: String,
    pub username: Option<String>,
    pub name: String,
    pub status: String,
    pub is_platform_staff: bool,
    pub tenant_id: Option<Uuid>,
}

/// `GET /admin/users?tenant_id=&q=` — directory of user accounts.
#[rocket_okapi::openapi(tag = "IAM")]
#[get("/admin/users?<tenant_id>&<q>")]
pub async fn list_users(
    state: &State<AppState>,
    user: AuthUser,
    tenant_id: Option<String>,
    q: Option<String>,
) -> ApiResult<Json<Vec<UserListItem>>> {
    user.require(Permission::UserRead)?;
    let mut query = User::find();
    if let Some(tid) = tenant_id.as_deref().and_then(|s| Uuid::parse_str(s).ok()) {
        query = query.filter(entity::user::Column::TenantId.eq(tid));
    }
    if let Some(term) = q.filter(|s| !s.is_empty()) {
        let like = format!("%{}%", term.to_lowercase());
        query = query.filter(
            entity::user::Column::Email
                .contains(&like)
                .or(entity::user::Column::Name.contains(&term)),
        );
    }
    let users = query
        .order_by_asc(entity::user::Column::Name)
        .all(&state.db)
        .await?;
    Ok(Json(
        users
            .into_iter()
            .map(|u| UserListItem {
                id: u.id,
                email: u.email,
                username: u.username,
                name: u.name,
                status: u.status,
                is_platform_staff: u.is_platform_staff,
                tenant_id: u.tenant_id,
            })
            .collect(),
    ))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct NewMembership {
    pub scope: String,
    pub tenant_id: Option<Uuid>,
    pub profile_type: String,
    pub title: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct ProfileInput {
    pub legal_first_name: Option<String>,
    pub legal_middle_name: Option<String>,
    pub legal_last_name: Option<String>,
    pub preferred_name: Option<String>,
    /// ISO date `YYYY-MM-DD`.
    pub date_of_birth: Option<String>,
    pub phone: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub city: Option<String>,
    pub region: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    /// Plaintext SSN — encrypted before storage, never returned.
    pub ssn: Option<String>,
    pub gov_id_type: Option<String>,
    /// Plaintext government-ID number — encrypted before storage.
    pub gov_id_number: Option<String>,
    pub photo_url: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateUserReq {
    pub email: String,
    pub username: Option<String>,
    pub name: String,
    /// Optional initial password; if omitted, the account is `invited` with a
    /// random password (an invite flow would set it later).
    pub password: Option<String>,
    pub membership: Option<NewMembership>,
    pub profile: Option<ProfileInput>,
}

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
        "user.create",
        Some("user"),
        Some(uid.to_string()),
        primary_tenant,
        None,
    )
    .await;

    load_user_detail(state, uid).await
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateUserReq {
    pub name: Option<String>,
    pub username: Option<String>,
    /// `active` | `invited` | `suspended` | `disabled`.
    pub status: Option<String>,
}

/// `PATCH /admin/users/<id>` — update identity fields / status.
#[rocket_okapi::openapi(tag = "IAM")]
#[patch("/admin/users/<id>", data = "<body>")]
pub async fn update_user(
    state: &State<AppState>,
    user: AuthUser,
    id: &str,
    body: Json<UpdateUserReq>,
) -> ApiResult<Json<UserDetail>> {
    user.require(Permission::UserManage)?;
    let uid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid user id".into()))?;
    let u = User::find_by_id(uid)
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("user not found".into()))?;
    let body = body.into_inner();
    let mut am: entity::user::ActiveModel = u.into();
    if let Some(name) = body.name {
        am.name = Set(name);
    }
    if let Some(username) = body.username {
        am.username = Set(Some(username));
    }
    if let Some(status) = body.status {
        am.status = Set(status);
    }
    am.update(&state.db).await?;
    load_user_detail(state, uid).await
}

// ---- Profiles ----

#[derive(Serialize, schemars::JsonSchema)]
pub struct ProfileDto {
    pub legal_first_name: Option<String>,
    pub legal_middle_name: Option<String>,
    pub legal_last_name: Option<String>,
    pub preferred_name: Option<String>,
    pub date_of_birth: Option<String>,
    pub phone: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub city: Option<String>,
    pub region: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    /// Masked — only the last four are ever returned here.
    pub ssn_last4: Option<String>,
    pub gov_id_type: Option<String>,
    pub gov_id_last4: Option<String>,
    pub photo_url: Option<String>,
    pub has_ssn: bool,
    pub has_gov_id: bool,
}

impl From<entity::user_profile::Model> for ProfileDto {
    fn from(p: entity::user_profile::Model) -> Self {
        ProfileDto {
            legal_first_name: p.legal_first_name,
            legal_middle_name: p.legal_middle_name,
            legal_last_name: p.legal_last_name,
            preferred_name: p.preferred_name,
            date_of_birth: p.date_of_birth.map(|d| d.to_string()),
            phone: p.phone,
            address_line1: p.address_line1,
            address_line2: p.address_line2,
            city: p.city,
            region: p.region,
            postal_code: p.postal_code,
            country: p.country,
            ssn_last4: p.ssn_last4,
            gov_id_type: p.gov_id_type,
            gov_id_last4: p.gov_id_last4,
            photo_url: p.photo_url,
            has_ssn: p.ssn_ciphertext.is_some(),
            has_gov_id: p.gov_id_ciphertext.is_some(),
        }
    }
}

/// `PUT /admin/users/<id>/profile` — upsert profile; SSN/gov-ID encrypted at rest.
#[rocket_okapi::openapi(tag = "IAM")]
#[put("/admin/users/<id>/profile", data = "<body>")]
pub async fn put_profile(
    state: &State<AppState>,
    user: AuthUser,
    id: &str,
    body: Json<ProfileInput>,
) -> ApiResult<Json<ProfileDto>> {
    user.require(Permission::ProfileWrite)?;
    let uid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid user id".into()))?;
    if User::find_by_id(uid).one(&state.db).await?.is_none() {
        return Err(ApiError::NotFound("user not found".into()));
    }
    upsert_profile_inner(&state.db, &state.config.pii_key, uid, &body.into_inner()).await?;
    let p = UserProfile::find_by_id(uid).one(&state.db).await?.unwrap();
    Ok(Json(p.into()))
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct PiiReveal {
    pub ssn: Option<String>,
    pub gov_id_number: Option<String>,
}

/// `GET /admin/users/<id>/pii` — decrypt and return sensitive PII. Requires the
/// dedicated `profile:read_pii` permission and is logged as an access event.
#[rocket_okapi::openapi(tag = "IAM")]
#[get("/admin/users/<id>/pii")]
pub async fn reveal_pii(
    state: &State<AppState>,
    user: AuthUser,
    id: &str,
) -> ApiResult<Json<PiiReveal>> {
    user.require(Permission::ProfilePiiRead)?;
    let uid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid user id".into()))?;
    let p = UserProfile::find_by_id(uid)
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("profile not found".into()))?;
    let key = &state.config.pii_key;
    let ssn = match (p.ssn_ciphertext, p.ssn_nonce) {
        (Some(ct), Some(n)) => Some(pii::decrypt(key, &ct, &n).map_err(ApiError::Internal)?),
        _ => None,
    };
    let gov = match (p.gov_id_ciphertext, p.gov_id_nonce) {
        (Some(ct), Some(n)) => Some(pii::decrypt(key, &ct, &n).map_err(ApiError::Internal)?),
        _ => None,
    };
    tracing::warn!(actor = %user.user_id, subject = %uid, "PII revealed (SSN/gov-id)");
    crate::audit::record(
        &state.db,
        Some(user.user_id),
        "pii.reveal",
        Some("user"),
        Some(uid.to_string()),
        None,
        Some(serde_json::json!({ "fields": ["ssn", "gov_id"] })),
    )
    .await;
    Ok(Json(PiiReveal {
        ssn,
        gov_id_number: gov,
    }))
}

// ---- Memberships & role assignment ----

#[derive(Serialize, schemars::JsonSchema)]
pub struct MembershipDto {
    pub id: Uuid,
    pub scope: String,
    pub tenant_id: Option<Uuid>,
    pub profile_type: String,
    pub title: Option<String>,
    pub status: String,
    pub is_primary: bool,
}

/// `POST /admin/users/<id>/memberships` — add a persona; auto-grants its default role.
#[rocket_okapi::openapi(tag = "IAM")]
#[post("/admin/users/<id>/memberships", data = "<body>")]
pub async fn add_membership(
    state: &State<AppState>,
    user: AuthUser,
    id: &str,
    body: Json<NewMembership>,
) -> ApiResult<Json<MembershipDto>> {
    user.require(Permission::MemberManage)?;
    let uid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid user id".into()))?;
    if User::find_by_id(uid).one(&state.db).await?.is_none() {
        return Err(ApiError::NotFound("user not found".into()));
    }
    let m = add_membership_inner(&state.db, uid, &body.into_inner(), false).await?;
    Ok(Json(MembershipDto {
        id: m.id,
        scope: m.scope,
        tenant_id: m.tenant_id,
        profile_type: m.profile_type,
        title: m.title,
        status: m.status,
        is_primary: m.is_primary,
    }))
}

/// `DELETE /admin/memberships/<id>` — remove a membership.
#[rocket_okapi::openapi(tag = "IAM")]
#[delete("/admin/memberships/<id>")]
pub async fn remove_membership(
    state: &State<AppState>,
    user: AuthUser,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::MemberManage)?;
    let mid =
        Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid membership id".into()))?;
    Membership::delete_by_id(mid).exec(&state.db).await?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct AssignRoleReq {
    pub role_id: Uuid,
    pub tenant_id: Option<Uuid>,
}

/// `POST /admin/users/<id>/roles` — grant a role to a user (optionally tenant-scoped).
#[rocket_okapi::openapi(tag = "IAM")]
#[post("/admin/users/<id>/roles", data = "<body>")]
pub async fn assign_role(
    state: &State<AppState>,
    user: AuthUser,
    id: &str,
    body: Json<AssignRoleReq>,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::RoleManage)?;
    let uid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid user id".into()))?;
    let body = body.into_inner();
    if Role::find_by_id(body.role_id)
        .one(&state.db)
        .await?
        .is_none()
    {
        return Err(ApiError::NotFound("role not found".into()));
    }
    entity::user_role::ActiveModel {
        id: sea_orm::ActiveValue::NotSet,
        user_id: Set(uid),
        role_id: Set(body.role_id),
        tenant_id: Set(body.tenant_id),
    }
    .insert(&state.db)
    .await?;
    Ok(Json(serde_json::json!({ "assigned": true })))
}

/// `DELETE /admin/user-roles/<id>` — revoke a role assignment.
#[rocket_okapi::openapi(tag = "IAM")]
#[delete("/admin/user-roles/<id>")]
pub async fn revoke_role(
    state: &State<AppState>,
    user: AuthUser,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::RoleManage)?;
    let urid: i64 = id
        .parse()
        .map_err(|_| ApiError::BadRequest("invalid assignment id".into()))?;
    UserRole::delete_by_id(urid).exec(&state.db).await?;
    Ok(Json(serde_json::json!({ "revoked": true })))
}

// ---- User detail ----

#[derive(Serialize, schemars::JsonSchema)]
pub struct UserRoleDto {
    pub id: i64,
    pub role_id: Uuid,
    pub role_key: String,
    pub role_name: String,
    pub tenant_id: Option<Uuid>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct UserDetail {
    pub id: Uuid,
    pub email: String,
    pub username: Option<String>,
    pub name: String,
    pub status: String,
    pub is_platform_staff: bool,
    pub tenant_id: Option<Uuid>,
    pub profile: Option<ProfileDto>,
    pub memberships: Vec<MembershipDto>,
    pub roles: Vec<UserRoleDto>,
}

/// `GET /admin/users/<id>` — full account view (identity + masked profile +
/// memberships + role assignments).
#[rocket_okapi::openapi(tag = "IAM")]
#[get("/admin/users/<id>")]
pub async fn get_user(
    state: &State<AppState>,
    user: AuthUser,
    id: &str,
) -> ApiResult<Json<UserDetail>> {
    user.require(Permission::UserRead)?;
    let uid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid user id".into()))?;
    load_user_detail(state, uid).await
}

async fn load_user_detail(state: &State<AppState>, uid: Uuid) -> ApiResult<Json<UserDetail>> {
    let u = User::find_by_id(uid)
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("user not found".into()))?;

    let profile = UserProfile::find_by_id(uid)
        .one(&state.db)
        .await?
        .map(ProfileDto::from);

    let memberships = Membership::find()
        .filter(entity::membership::Column::UserId.eq(uid))
        .all(&state.db)
        .await?
        .into_iter()
        .map(|m| MembershipDto {
            id: m.id,
            scope: m.scope,
            tenant_id: m.tenant_id,
            profile_type: m.profile_type,
            title: m.title,
            status: m.status,
            is_primary: m.is_primary,
        })
        .collect();

    // Roles, joined to their key/name.
    let urs = UserRole::find()
        .filter(entity::user_role::Column::UserId.eq(uid))
        .all(&state.db)
        .await?;
    let mut roles = Vec::new();
    for ur in urs {
        if let Some(r) = Role::find_by_id(ur.role_id).one(&state.db).await? {
            roles.push(UserRoleDto {
                id: ur.id,
                role_id: r.id,
                role_key: r.key,
                role_name: r.name,
                tenant_id: ur.tenant_id,
            });
        }
    }

    Ok(Json(UserDetail {
        id: u.id,
        email: u.email,
        username: u.username,
        name: u.name,
        status: u.status,
        is_platform_staff: u.is_platform_staff,
        tenant_id: u.tenant_id,
        profile,
        memberships,
        roles,
    }))
}

// ===========================================================================
// Tenant member management (client admins, scoped to the active tenant)
// ===========================================================================

/// `GET /members` — the active tenant's member directory (persona + status).
#[rocket_okapi::openapi(tag = "IAM")]
#[get("/members")]
pub async fn list_members(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<MemberDto>>> {
    user.require(Permission::MemberRead)?;
    let memberships = Membership::find()
        .filter(entity::membership::Column::TenantId.eq(scope.tenant_id))
        .all(&state.db)
        .await?;
    let mut out = Vec::new();
    for m in memberships {
        if let Some(u) = User::find_by_id(m.user_id).one(&state.db).await? {
            out.push(MemberDto {
                membership_id: m.id,
                user_id: u.id,
                name: u.name,
                email: u.email,
                profile_type: m.profile_type,
                title: m.title,
                status: m.status,
            });
        }
    }
    Ok(Json(out))
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct MemberDto {
    pub membership_id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub email: String,
    pub profile_type: String,
    pub title: Option<String>,
    pub status: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct InviteMemberReq {
    pub email: String,
    pub name: String,
    /// Tenant persona, e.g. `property_manager`, `back_office`, `landlord`.
    pub profile_type: String,
    pub title: Option<String>,
}

/// `POST /members` — invite a member into the active tenant with a persona; the
/// persona's default role is granted automatically.
#[rocket_okapi::openapi(tag = "IAM")]
#[post("/members", data = "<body>")]
pub async fn invite_member(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    body: Json<InviteMemberReq>,
) -> ApiResult<Json<MemberDto>> {
    user.require(Permission::MemberManage)?;
    let body = body.into_inner();
    let email = body.email.trim().to_lowercase();

    // Reuse or create the underlying user account.
    let existing = User::find()
        .filter(entity::user::Column::Email.eq(email.clone()))
        .one(&state.db)
        .await?;
    let txn = state.db.begin().await?;
    let uid = match existing {
        Some(u) => u.id,
        None => {
            let uid = Uuid::new_v4();
            let pw = hash_password(&random_secret(24)).map_err(ApiError::Internal)?;
            entity::user::ActiveModel {
                id: Set(uid),
                tenant_id: Set(Some(scope.tenant_id)),
                email: Set(email),
                username: Set(None),
                password_hash: Set(pw),
                name: Set(body.name.clone()),
                is_platform_staff: Set(false),
                status: Set("invited".into()),
                last_login_at: Set(None),
                created_at: Set(Utc::now().into()),
            }
            .insert(&txn)
            .await?;
            uid
        }
    };
    let m = NewMembership {
        scope: rbac::SCOPE_TENANT.to_string(),
        tenant_id: Some(scope.tenant_id),
        profile_type: body.profile_type.clone(),
        title: body.title.clone(),
    };
    let membership = add_membership_inner(&txn, uid, &m, false).await?;
    txn.commit().await?;

    Ok(Json(MemberDto {
        membership_id: membership.id,
        user_id: uid,
        name: body.name,
        email: body.email.trim().to_lowercase(),
        profile_type: body.profile_type,
        title: body.title,
        status: membership.status,
    }))
}

// ===========================================================================
// Shared internals
// ===========================================================================

/// Insert a membership and grant the persona's default role. Validates the
/// persona against the catalog and that platform/tenant scope matches.
async fn add_membership_inner<C: sea_orm::ConnectionTrait>(
    db: &C,
    user_id: Uuid,
    m: &NewMembership,
    is_primary: bool,
) -> Result<entity::membership::Model, ApiError> {
    let persona = rbac::PROFILE_TYPES
        .iter()
        .find(|p| p.key == m.profile_type)
        .ok_or_else(|| ApiError::BadRequest(format!("unknown profile_type: {}", m.profile_type)))?;
    if persona.scope != m.scope {
        return Err(ApiError::BadRequest(format!(
            "persona '{}' is scoped to '{}'",
            persona.key, persona.scope
        )));
    }
    if m.scope == rbac::SCOPE_TENANT && m.tenant_id.is_none() {
        return Err(ApiError::BadRequest(
            "tenant_id required for a tenant membership".into(),
        ));
    }

    let mid = Uuid::new_v4();
    let model = entity::membership::ActiveModel {
        id: Set(mid),
        user_id: Set(user_id),
        scope: Set(m.scope.clone()),
        tenant_id: Set(m.tenant_id),
        profile_type: Set(m.profile_type.clone()),
        title: Set(m.title.clone()),
        status: Set("active".into()),
        is_primary: Set(is_primary),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await?;

    // Grant the persona's default role for this scope, if it exists.
    if let Some(role_key) = rbac::default_role_for_persona(&m.profile_type) {
        if let Some(role) = Role::find()
            .filter(entity::role::Column::Key.eq(role_key))
            .filter(entity::role::Column::IsSystem.eq(true))
            .one(db)
            .await?
        {
            entity::user_role::ActiveModel {
                id: sea_orm::ActiveValue::NotSet,
                user_id: Set(user_id),
                role_id: Set(role.id),
                tenant_id: Set(m.tenant_id),
            }
            .insert(db)
            .await?;
        }
    }
    Ok(model)
}

/// Insert or update a user's profile, encrypting SSN / gov-ID when provided.
async fn upsert_profile_inner<C: sea_orm::ConnectionTrait>(
    db: &C,
    pii_key: &[u8],
    user_id: Uuid,
    input: &ProfileInput,
) -> Result<(), ApiError> {
    let dob = match &input.date_of_birth {
        Some(s) if !s.is_empty() => Some(
            NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map_err(|_| ApiError::BadRequest("date_of_birth must be YYYY-MM-DD".into()))?,
        ),
        _ => None,
    };

    // Seal SSN / gov-id if present.
    let (ssn_ct, ssn_nonce, ssn_last4) = seal_optional(pii_key, input.ssn.as_deref())?;
    let (gid_ct, gid_nonce, gid_last4) = seal_optional(pii_key, input.gov_id_number.as_deref())?;

    let now = Utc::now();
    let existing = UserProfile::find_by_id(user_id).one(db).await?;
    match existing {
        Some(p) => {
            let mut am: entity::user_profile::ActiveModel = p.into();
            am.legal_first_name = Set(input.legal_first_name.clone());
            am.legal_middle_name = Set(input.legal_middle_name.clone());
            am.legal_last_name = Set(input.legal_last_name.clone());
            am.preferred_name = Set(input.preferred_name.clone());
            am.date_of_birth = Set(dob);
            am.phone = Set(input.phone.clone());
            am.address_line1 = Set(input.address_line1.clone());
            am.address_line2 = Set(input.address_line2.clone());
            am.city = Set(input.city.clone());
            am.region = Set(input.region.clone());
            am.postal_code = Set(input.postal_code.clone());
            am.country = Set(input.country.clone());
            am.photo_url = Set(input.photo_url.clone());
            am.gov_id_type = Set(input.gov_id_type.clone());
            // Only overwrite sensitive fields when a new value was supplied.
            if input.ssn.as_deref().map(|s| !s.is_empty()).unwrap_or(false) {
                am.ssn_ciphertext = Set(ssn_ct);
                am.ssn_nonce = Set(ssn_nonce);
                am.ssn_last4 = Set(ssn_last4);
            }
            if input
                .gov_id_number
                .as_deref()
                .map(|s| !s.is_empty())
                .unwrap_or(false)
            {
                am.gov_id_ciphertext = Set(gid_ct);
                am.gov_id_nonce = Set(gid_nonce);
                am.gov_id_last4 = Set(gid_last4);
            }
            am.updated_at = Set(now.into());
            am.update(db).await?;
        }
        None => {
            entity::user_profile::ActiveModel {
                user_id: Set(user_id),
                legal_first_name: Set(input.legal_first_name.clone()),
                legal_middle_name: Set(input.legal_middle_name.clone()),
                legal_last_name: Set(input.legal_last_name.clone()),
                preferred_name: Set(input.preferred_name.clone()),
                date_of_birth: Set(dob),
                phone: Set(input.phone.clone()),
                address_line1: Set(input.address_line1.clone()),
                address_line2: Set(input.address_line2.clone()),
                city: Set(input.city.clone()),
                region: Set(input.region.clone()),
                postal_code: Set(input.postal_code.clone()),
                country: Set(input.country.clone()),
                ssn_ciphertext: Set(ssn_ct),
                ssn_nonce: Set(ssn_nonce),
                ssn_last4: Set(ssn_last4),
                gov_id_type: Set(input.gov_id_type.clone()),
                gov_id_ciphertext: Set(gid_ct),
                gov_id_nonce: Set(gid_nonce),
                gov_id_last4: Set(gid_last4),
                photo_url: Set(input.photo_url.clone()),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }
            .insert(db)
            .await?;
        }
    }
    Ok(())
}

/// `(ciphertext, nonce, last4)` for a sealed secret column trio.
type SealedColumns = (Option<String>, Option<String>, Option<String>);

/// Seal an optional secret → `(ciphertext, nonce, last4)`.
fn seal_optional(key: &[u8], value: Option<&str>) -> Result<SealedColumns, ApiError> {
    match value {
        Some(v) if !v.is_empty() => {
            let sealed = pii::encrypt(key, v).map_err(ApiError::Internal)?;
            Ok((
                Some(sealed.ciphertext),
                Some(sealed.nonce),
                Some(pii::last4(v)),
            ))
        }
        _ => Ok((None, None, None)),
    }
}
