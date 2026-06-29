//! `GET /platform/staff` — the platform-plane roster (Acre employees). Staff hold
//! no tenant membership; they administer the platform itself and enter tenants
//! only via [`super::impersonate`].

use super::dto::PlatformStaffSummary;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use entity::prelude::{PlatformStaff, User};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::EntityTrait;
use std::collections::HashMap;
use uuid::Uuid;

/// `GET /platform/staff` — list platform-plane members.
#[rocket_okapi::openapi(tag = "Platform Admin")]
#[get("/platform/staff")]
pub async fn staff(
    state: &State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Vec<PlatformStaffSummary>>> {
    user.require(Permission::PlatformAdmin)?;
    let rows = PlatformStaff::find().all(&state.db).await?;

    let mut users: HashMap<Uuid, (String, String)> = HashMap::new();
    for u in User::find().all(&state.db).await? {
        users.insert(u.id, (u.email, u.name));
    }

    let out = rows
        .into_iter()
        .map(|s| {
            let (email, name) = users
                .get(&s.user_id)
                .cloned()
                .unwrap_or_else(|| (String::new(), String::new()));
            PlatformStaffSummary {
                id: s.id,
                user_id: s.user_id,
                email,
                name,
                status: s.status,
            }
        })
        .collect();
    Ok(Json(out))
}
