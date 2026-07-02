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

/// An empty profile for users who haven't filled anything in yet.
fn empty_profile() -> ProfileDto {
    ProfileDto {
        legal_first_name: None,
        legal_middle_name: None,
        legal_last_name: None,
        preferred_name: None,
        date_of_birth: None,
        phone: None,
        address_line1: None,
        address_line2: None,
        city: None,
        region: None,
        postal_code: None,
        country: None,
        ssn_last4: None,
        gov_id_type: None,
        gov_id_last4: None,
        photo_url: None,
        has_ssn: false,
        has_gov_id: false,
        has_pet: false,
        pet_details: None,
        is_military: false,
        annual_income_cents: None,
    }
}

/// The signed-in user's own vehicles, newest first.
pub(crate) async fn own_vehicles(
    db: &impl ConnectionTrait,
    user_id: Uuid,
) -> Result<Vec<entity::vehicle::Model>, sea_orm::DbErr> {
    Vehicle::find()
        .filter(entity::vehicle::Column::UserId.eq(user_id))
        .order_by_desc(entity::vehicle::Column::CreatedAt)
        .all(db)
        .await
}

async fn build_view(db: &crate::db::RequestDb, user_id: Uuid) -> ApiResult<MyProfileView> {
    let me = User::find_by_id(user_id)
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("user not found".into()))?;
    let profile = UserProfile::find_by_id(user_id)
        .one(db)
        .await?
        .map(ProfileDto::from)
        .unwrap_or_else(empty_profile);
    let vehicles = own_vehicles(db, user_id)
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
    _scope: TenantScope,
) -> ApiResult<Json<MyProfileView>> {
    Ok(Json(build_view(&db, user.user_id).await?))
}

/// `PUT /my/profile` — self-service update. SSN / government ID are write-only
/// (encrypted at rest, surfaced as last-4 like the admin view).
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[put("/my/profile", data = "<body>")]
pub async fn update_my_profile(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    _scope: TenantScope,
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

    Ok(Json(build_view(&db, user.user_id).await?))
}
