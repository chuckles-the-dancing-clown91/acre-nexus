//! `/my/profile` — **self-service profile**: the signed-in person maintains
//! their own record (contact details, pets, military status, income, and
//! write-only government ID / SSN) and the white-glove application flow reads
//! everything from it. Staff edit the same data through the IAM admin routes
//! (`PUT /admin/users/<id>/profile`); this is the renter-facing mirror, scoped
//! to the caller with no admin permission required.

use super::dto::{ProfileDto, ProfileInput};
use super::helpers::{profile_fields_touched, upsert_profile_inner};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::routes::vehicles::dto::VehicleDto;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{User, UserProfile, Vehicle};
use rocket::serde::json::Json;
use rocket::{get, put, State};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::Serialize;
use uuid::Uuid;

/// Everything the "My profile" page needs in one fetch.
#[derive(Serialize, schemars::JsonSchema)]
pub struct MyProfileView {
    /// Account display name.
    pub name: String,
    /// Account email (identity — applications always use this).
    pub email: String,
    /// The profile record (sensitive fields masked to last-4).
    pub profile: ProfileDto,
    /// The person's vehicles (auto-attached to portal applications).
    pub vehicles: Vec<VehicleDto>,
}

/// The signed-in user's own vehicles in this workspace, newest first.
pub(crate) async fn own_vehicles(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<entity::vehicle::Model>, sea_orm::DbErr> {
    Vehicle::find()
        .filter(entity::vehicle::Column::TenantId.eq(tenant_id))
        .filter(entity::vehicle::Column::UserId.eq(user_id))
        .order_by_desc(entity::vehicle::Column::CreatedAt)
        .all(db)
        .await
}

async fn build_view(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    user_id: Uuid,
) -> ApiResult<MyProfileView> {
    let me = User::find_by_id(user_id)
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("user not found".into()))?;
    let profile = UserProfile::find_by_id(user_id)
        .one(db)
        .await?
        .map(ProfileDto::from)
        .unwrap_or_default();
    let vehicles = own_vehicles(db, tenant_id, user_id)
        .await?
        .into_iter()
        .map(VehicleDto::from)
        .collect();
    Ok(MyProfileView {
        name: me.name,
        email: me.email,
        profile,
        vehicles,
    })
}

/// `GET /my/profile` — the signed-in user's profile + vehicles.
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[get("/my/profile")]
pub async fn my_profile(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<MyProfileView>> {
    Ok(Json(build_view(&db, scope.tenant_id, user.user_id).await?))
}

/// `PUT /my/profile` — self-service update. SSN / government ID are write-only
/// (encrypted at rest, surfaced as last-4 like the admin view).
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[put("/my/profile", data = "<body>")]
pub async fn update_my_profile(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<ProfileInput>,
) -> ApiResult<Json<MyProfileView>> {
    let input = body.into_inner();
    let fields = profile_fields_touched(&input);
    upsert_profile_inner(&db, &state.config.pii_key, user.user_id, &input).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::PROFILE_WRITE,
        Some("user"),
        Some(user.user_id.to_string()),
        user.tenant_id,
        Some(serde_json::json!({ "fields_set": fields, "self_service": true })),
    )
    .await;

    Ok(Json(build_view(&db, scope.tenant_id, user.user_id).await?))
}
