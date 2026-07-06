//! `/my/applications` — the **renter portal** door into the application
//! pipeline: an authenticated user applies through their own profile and
//! tracks their applications' progress. No staff permission required — the
//! data is scoped to the signed-in user (their linked applications, plus any
//! older ones submitted with the same email through the public site).
//!
//! **White-glove**: everything auto-fills from the profile — name, phone,
//! pets, military status, stated income — and the person's vehicles are
//! snapshotted onto the application (parking / garage amenities / lease
//! verbiage need them). The tenant only has to keep their profile current;
//! staff can correct any of it through the IAM profile routes.

use super::dto::{ApplicationResp, PortalApplyReq};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{Application, User, UserProfile, Vehicle};
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

    // Reuse works for portal applicants exactly like the public funnel.
    let email = me.email.to_lowercase();
    let reused_from = super::reuse::latest_reusable_approved(&db, scope.tenant_id, &email).await?;

    // White-glove auto-fill: explicit values win, the profile fills the rest.
    let profile_name = profile.as_ref().and_then(|p| {
        p.preferred_name
            .clone()
            .or_else(|| match (&p.legal_first_name, &p.legal_last_name) {
                (Some(f), Some(l)) => Some(format!("{f} {l}")),
                _ => None,
            })
    });
    let name = b
        .applicant_name
        .filter(|n| !n.trim().is_empty())
        .or(profile_name)
        .unwrap_or_else(|| me.name.clone());
    let phone = b
        .phone
        .filter(|p| !p.trim().is_empty())
        .or_else(|| profile.as_ref().and_then(|p| p.phone.clone()))
        .unwrap_or_default();
    let has_pet = b
        .has_pet
        .unwrap_or_else(|| profile.as_ref().map(|p| p.has_pet).unwrap_or(false));
    let pet_details = b
        .pet_details
        .filter(|d| !d.trim().is_empty())
        .or_else(|| profile.as_ref().and_then(|p| p.pet_details.clone()));
    let is_military = b
        .is_military
        .unwrap_or_else(|| profile.as_ref().map(|p| p.is_military).unwrap_or(false));
    let annual_income_cents = b
        .annual_income_cents
        .or_else(|| profile.as_ref().and_then(|p| p.annual_income_cents))
        .unwrap_or(0);

    let (saved, _job) = super::intake(
        &db,
        scope.tenant_id,
        super::IntakeInput {
            listing_id: b.listing_id,
            applicant_name: name,
            email,
            phone,
            annual_income_cents,
            credit_score: b.credit_score,
            move_in: b.move_in.unwrap_or_default(),
            has_pet,
            pet_details,
            is_military,
            screening_consent: b.screening_consent.unwrap_or(false),
        },
        "portal",
        Some(user.user_id),
        Some(user.user_id),
        reused_from.as_ref(),
    )
    .await?;

    // Snapshot the person's vehicles onto the application: convert re-links
    // application vehicles to the lease, so these copies flow all the way into
    // parking amenities and lease verbiage. The profile rows stay the master.
    let own =
        crate::routes::iam::self_profile::own_vehicles(&db, scope.tenant_id, user.user_id).await?;
    let now = chrono::Utc::now();
    let copies: Vec<entity::vehicle::ActiveModel> = own
        .into_iter()
        .map(|v| entity::vehicle::ActiveModel {
            id: sea_orm::Set(uuid::Uuid::new_v4()),
            tenant_id: sea_orm::Set(scope.tenant_id),
            lease_id: sea_orm::Set(None),
            application_id: sea_orm::Set(Some(saved.id)),
            user_id: sea_orm::Set(None),
            make: sea_orm::Set(v.make),
            model: sea_orm::Set(v.model),
            year: sea_orm::Set(v.year),
            color: sea_orm::Set(v.color),
            license_plate: sea_orm::Set(v.license_plate),
            plate_state: sea_orm::Set(v.plate_state),
            notes: sea_orm::Set(v.notes),
            created_at: sea_orm::Set(now.into()),
            updated_at: sea_orm::Set(now.into()),
        })
        .collect();
    let snapshotted = copies.len();
    if let Err(e) = Vehicle::insert_many(copies)
        .on_empty_do_nothing()
        .exec(&db)
        .await
    {
        tracing::warn!("failed to snapshot vehicles onto application: {e}");
    } else if snapshotted > 0 {
        crate::audit::record(
            &db,
            Some(user.user_id),
            crate::audit::actions::VEHICLE_CREATE,
            Some("application"),
            Some(saved.id.to_string()),
            Some(scope.tenant_id),
            Some(serde_json::json!({ "snapshotted": snapshotted, "source": "profile" })),
        )
        .await;
    }

    Ok(Json(ApplicationResp::from(saved)))
}
