use super::dto::{InviteMemberReq, MemberDto, NewMembership};
use super::helpers::add_membership_inner;
use crate::auth::{hash_password, random_secret, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::rbac::{self, Permission};
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::*;
use rocket::post;
use rocket::serde::json::Json;
use rocket::State;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait};
use uuid::Uuid;

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
        .one(&state.user_db)
        .await?;
    let txn = state.user_db.begin().await?;
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
