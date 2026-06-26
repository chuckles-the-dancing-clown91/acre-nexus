use super::dto::MemberDto;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::*;
use rocket::get;
use rocket::serde::json::Json;
use rocket::State;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

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
