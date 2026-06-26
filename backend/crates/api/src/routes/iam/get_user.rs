use super::dto::UserDetail;
use super::helpers::load_user_detail;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use rocket::get;
use rocket::serde::json::Json;
use rocket::State;
use uuid::Uuid;

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
