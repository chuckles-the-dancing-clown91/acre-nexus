//! `/my/applications` — the **renter portal** door into the application
//! pipeline: an authenticated user applies through their own profile and
//! tracks their applications' progress. No staff permission required — the
//! data is scoped to the signed-in user (their linked applications, plus any
//! older ones submitted with the same email through the public site).

use super::dto::{ApplicationResp, PortalApplyReq};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{Application, Listing, User, UserProfile};
use rocket::serde::json::Json;
use rocket::{get, post, State};
use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder};

/// `GET /my/applications` — the signed-in user's applications, newest first.
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[get("/my/applications")]
pub async fn my_applications(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<ApplicationResp>>> {
    let me = User::find_by_id(user.user_id)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("user not found".into()))?;
    let rows = Application::find()
        .filter(entity::application::Column::TenantId.eq(scope.tenant_id))
        .filter(
            Condition::any()
                .add(entity::application::Column::ApplicantUserId.eq(user.user_id))
                .add(entity::application::Column::Email.eq(me.email.to_lowercase())),
        )
        .order_by_desc(entity::application::Column::CreatedAt)
        .all(&db)
        .await?;
    Ok(Json(rows.into_iter().map(ApplicationResp::from).collect()))
}

/// `POST /my/applications` — apply as the signed-in user. Identity comes from
/// the account (email is always the account's; name/phone fall back to the
/// user's profile), and the application is linked via `applicant_user_id` so
/// the portal can track it.
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[post("/my/applications", data = "<body>")]
pub async fn my_apply(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<PortalApplyReq>,
) -> ApiResult<Json<ApplicationResp>> {
    let b = body.into_inner();
    let me = User::find_by_id(user.user_id)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("user not found".into()))?;
    let profile = UserProfile::find()
        .filter(entity::user_profile::Column::UserId.eq(user.user_id))
        .one(&db)
        .await?;

    // A referenced listing must be this workspace's.
    if let Some(lid) = b.listing_id {
        Listing::find_by_id(lid)
            .filter(entity::listing::Column::TenantId.eq(scope.tenant_id))
            .one(&db)
            .await?
            .ok_or_else(|| ApiError::NotFound("listing not found".into()))?;
    }

    // Reuse works for portal applicants exactly like the public funnel.
    let email = me.email.to_lowercase();
    let reused_from = match super::reuse::reuse_cutoff(&db, scope.tenant_id).await {
        Some(cutoff) => {
            Application::find()
                .filter(entity::application::Column::TenantId.eq(scope.tenant_id))
                .filter(entity::application::Column::Email.eq(email.clone()))
                .filter(entity::application::Column::Status.eq("Approved"))
                .filter(entity::application::Column::CreatedAt.gte(cutoff))
                .order_by_desc(entity::application::Column::CreatedAt)
                .one(&db)
                .await?
        }
        None => None,
    };

    let name = b
        .applicant_name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| me.name.clone());
    let phone = b
        .phone
        .filter(|p| !p.trim().is_empty())
        .or_else(|| profile.and_then(|p| p.phone))
        .unwrap_or_default();

    let (saved, _job) = super::intake(
        &db,
        scope.tenant_id,
        super::IntakeInput {
            listing_id: b.listing_id,
            applicant_name: name,
            email,
            phone,
            annual_income_cents: b.annual_income_cents.unwrap_or(0),
            credit_score: b.credit_score,
            move_in: b.move_in.unwrap_or_default(),
            has_pet: b.has_pet.unwrap_or(false),
            pet_details: b.pet_details,
            is_military: b.is_military.unwrap_or(false),
        },
        "portal",
        Some(user.user_id),
        Some(user.user_id),
        reused_from.as_ref(),
    )
    .await?;

    Ok(Json(ApplicationResp::from(saved)))
}
