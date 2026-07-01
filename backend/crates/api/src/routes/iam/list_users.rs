use super::dto::UserListItem;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use entity::prelude::*;
use rocket::get;
use rocket::serde::json::Json;
use rocket::State;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

/// `GET /admin/users?tenant_id=&q=` — directory of user accounts.
#[rocket_okapi::openapi(tag = "IAM")]
#[get("/admin/users?<tenant_id>&<q>")]
pub async fn list_users(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
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
        .all(&db)
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
