use super::dto::{MembershipDto, NewMembership, ProfileDto, ProfileInput, UserDetail, UserRoleDto};
use crate::error::{ApiError, ApiResult};
use crate::pii;
use crate::rbac;
use crate::state::AppState;
use chrono::{NaiveDate, Utc};
use entity::prelude::*;
use rocket::serde::json::Json;
use rocket::State;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

pub(crate) async fn role_permissions(
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

pub(crate) async fn replace_role_permissions(
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

/// Reject permission keys not present in the catalog (keeps roles coherent).
pub(crate) fn validate_permissions(perms: &[String]) -> Result<(), ApiError> {
    let known: std::collections::HashSet<&str> =
        rbac::PERMISSION_CATALOG.iter().map(|p| p.key).collect();
    for p in perms {
        if !known.contains(p.as_str()) {
            return Err(ApiError::BadRequest(format!("unknown permission: {p}")));
        }
    }
    Ok(())
}

pub(crate) async fn load_user_detail(
    state: &State<AppState>,
    uid: Uuid,
) -> ApiResult<Json<UserDetail>> {
    let u = User::find_by_id(uid)
        .one(&state.user_db)
        .await?
        .ok_or_else(|| ApiError::NotFound("user not found".into()))?;

    let profile = UserProfile::find_by_id(uid)
        .one(&state.user_db)
        .await?
        .map(ProfileDto::from);

    let memberships = Membership::find()
        .filter(entity::membership::Column::UserId.eq(uid))
        .all(&state.user_db)
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
        .all(&state.user_db)
        .await?;
    let mut roles = Vec::new();
    for ur in urs {
        if let Some(r) = Role::find_by_id(ur.role_id).one(&state.user_db).await? {
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

/// Insert a membership and grant the persona's default role. Validates the
/// persona against the catalog and that platform/tenant scope matches.
pub(crate) async fn add_membership_inner<C: sea_orm::ConnectionTrait>(
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
pub(crate) async fn upsert_profile_inner<C: sea_orm::ConnectionTrait>(
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
pub(crate) type SealedColumns = (Option<String>, Option<String>, Option<String>);

/// Seal an optional secret → `(ciphertext, nonce, last4)`.
pub(crate) fn seal_optional(key: &[u8], value: Option<&str>) -> Result<SealedColumns, ApiError> {
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
