//! Application **reuse** — when the `application_reuse` setting is on, a recent
//! application can be reused for any property in the firm without re-applying.
//!
//! * `GET /applications/reusable?email=` lists an applicant's recent reusable
//!   applications (within the configured window).
//! * `POST /applications/reuse` clones one onto a new listing/property, carrying
//!   the screening result so the applicant isn't re-screened.

use super::dto::ApplicationResp;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::settings;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::{Duration, Utc};
use entity::prelude::Application;
use rocket::serde::json::Json;
use rocket::{get, post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ReuseReq {
    pub source_application_id: Uuid,
    /// The listing the reused application should target (optional).
    pub listing_id: Option<Uuid>,
}

/// Whether reuse is enabled and, if so, the cutoff timestamp for "recent".
/// Returns `None` when reuse is disabled or the window is non-positive.
pub(crate) async fn reuse_cutoff(
    db: &impl sea_orm::ConnectionTrait,
    tenant_id: Uuid,
) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    if !settings::get_bool(db, tenant_id, settings::APPLICATION_REUSE_ENABLED).await {
        return None;
    }
    let window = settings::get_i64(db, tenant_id, settings::APPLICATION_REUSE_WINDOW_DAYS).await;
    if window <= 0 {
        return None;
    }
    Some((Utc::now() - Duration::days(window)).into())
}

/// A prior application is reusable if it isn't a dead end (Declined/Withdrawn).
fn is_reusable_status(status: &str) -> bool {
    !matches!(status, "Declined" | "Withdrawn")
}

/// The applicant's most recent **approved** application inside the reuse
/// window, if the workspace allows reuse. The public and portal doors carry
/// its screening result forward so a returning applicant isn't re-screened.
pub(crate) async fn latest_reusable_approved(
    db: &impl sea_orm::ConnectionTrait,
    tenant_id: Uuid,
    email: &str,
) -> Result<Option<entity::application::Model>, sea_orm::DbErr> {
    let Some(cutoff) = reuse_cutoff(db, tenant_id).await else {
        return Ok(None);
    };
    Application::find()
        .filter(entity::application::Column::TenantId.eq(tenant_id))
        .filter(entity::application::Column::Email.eq(email))
        .filter(entity::application::Column::Status.eq("Approved"))
        .filter(entity::application::Column::CreatedAt.gte(cutoff))
        .order_by_desc(entity::application::Column::CreatedAt)
        .one(db)
        .await
}

/// `GET /applications/reusable?email=` — an applicant's recent reusable apps.
#[rocket_okapi::openapi(tag = "Applications")]
#[get("/applications/reusable?<email>")]
pub async fn reusable(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    email: &str,
) -> ApiResult<Json<Vec<ApplicationResp>>> {
    user.require(Permission::ApplicationRead)?;
    let Some(cutoff) = reuse_cutoff(&db, scope.tenant_id).await else {
        // Reuse disabled → nothing is reusable.
        return Ok(Json(vec![]));
    };
    let rows = Application::find()
        .filter(entity::application::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::application::Column::Email.eq(email.trim().to_lowercase()))
        .filter(entity::application::Column::CreatedAt.gte(cutoff))
        .order_by_desc(entity::application::Column::CreatedAt)
        .all(&db)
        .await?;
    let out = rows
        .into_iter()
        .filter(|a| is_reusable_status(&a.status))
        .map(ApplicationResp::from)
        .collect();
    Ok(Json(out))
}

/// `POST /applications/reuse` — clone a recent application onto a new listing.
#[rocket_okapi::openapi(tag = "Applications")]
#[post("/applications/reuse", data = "<body>")]
pub async fn reuse(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<ReuseReq>,
) -> ApiResult<Json<ApplicationResp>> {
    user.require(Permission::ApplicationWrite)?;
    let b = body.into_inner();

    let cutoff = reuse_cutoff(&db, scope.tenant_id).await.ok_or_else(|| {
        ApiError::BadRequest("application reuse is not enabled for this workspace".into())
    })?;

    let src = Application::find_by_id(b.source_application_id)
        .filter(entity::application::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("source application not found".into()))?;

    if src.created_at < cutoff {
        return Err(ApiError::BadRequest(
            "that application is older than the reuse window".into(),
        ));
    }
    if !is_reusable_status(&src.status) {
        return Err(ApiError::BadRequest(format!(
            "an application with status '{}' can't be reused",
            src.status
        )));
    }

    // Carry the screening outcome: a previously approved applicant stays approved
    // for the new property; otherwise they re-enter screening (data preserved).
    let new_status = if src.status == "Approved" {
        "Approved"
    } else {
        "Screening"
    };
    let new_id = Uuid::new_v4();
    let now = Utc::now();
    let saved = entity::application::ActiveModel {
        id: Set(new_id),
        tenant_id: Set(scope.tenant_id),
        listing_id: Set(b.listing_id),
        applicant_name: Set(src.applicant_name.clone()),
        email: Set(src.email.clone()),
        phone: Set(src.phone.clone()),
        annual_income_cents: Set(src.annual_income_cents),
        credit_score: Set(src.credit_score),
        status: Set(new_status.to_string()),
        move_in: Set(src.move_in.clone()),
        has_pet: Set(src.has_pet),
        pet_details: Set(src.pet_details.clone()),
        is_military: Set(src.is_military),
        // Staff-initiated reuse: same door metadata + screening carry-over as
        // the source application.
        source: Set("back_office".into()),
        applicant_user_id: Set(src.applicant_user_id),
        screening_status: Set(src.screening_status.clone()),
        screened_at: Set(src.screened_at),
        created_at: Set(now.into()),
    }
    .insert(&db)
    .await?;

    // Seed the new application's workflow history with the reuse event.
    entity::application_event::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        application_id: Set(new_id),
        from_status: Set(None),
        to_status: Set(new_status.to_string()),
        note: Set(Some(format!("Reused from application {}", src.id))),
        actor_user_id: Set(Some(user.user_id)),
        created_at: Set(now.into()),
    }
    .insert(&db)
    .await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::APPLICATION_REUSE,
        Some("application"),
        Some(new_id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({
            "source_application_id": src.id,
            "listing_id": b.listing_id,
            "status": new_status,
        })),
    )
    .await;

    Ok(Json(ApplicationResp::from(saved)))
}
