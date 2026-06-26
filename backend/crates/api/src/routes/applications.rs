//! Landlord/PM application management (tenant-scoped, RBAC-gated).

use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::scheduler;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Application;
use rocket::serde::json::Json;
use rocket::{get, patch, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

#[derive(Serialize)]
pub struct ApplicationResp {
    pub id: Uuid,
    pub listing_id: Option<Uuid>,
    pub applicant_name: String,
    pub email: String,
    pub phone: String,
    pub annual_income_label: String,
    pub credit_score: Option<i32>,
    pub status: String,
    pub move_in: String,
}

impl From<entity::application::Model> for ApplicationResp {
    fn from(a: entity::application::Model) -> Self {
        ApplicationResp {
            annual_income_label: usd(a.annual_income_cents),
            id: a.id,
            listing_id: a.listing_id,
            applicant_name: a.applicant_name,
            email: a.email,
            phone: a.phone,
            credit_score: a.credit_score,
            status: a.status,
            move_in: a.move_in,
        }
    }
}

/// `GET /applications` — applications for the active tenant.
#[get("/applications")]
pub async fn list(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<ApplicationResp>>> {
    user.require(Permission::ApplicationRead)?;
    let rows = Application::find()
        .filter(entity::application::Column::TenantId.eq(scope.tenant_id))
        .order_by_desc(entity::application::Column::CreatedAt)
        .all(&state.db)
        .await?;
    Ok(Json(rows.into_iter().map(ApplicationResp::from).collect()))
}

#[derive(Deserialize)]
pub struct UpdateApplicationReq {
    /// `New` | `Screening` | `Approved` | `Declined`.
    pub status: String,
}

/// `PATCH /applications/<id>` — advance an application's status.
///
/// Approving an application enqueues an automated welcome email via the scheduler.
#[patch("/applications/<id>", data = "<body>")]
pub async fn update_status(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateApplicationReq>,
) -> ApiResult<Json<ApplicationResp>> {
    user.require(Permission::ApplicationWrite)?;
    let aid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let a = Application::find_by_id(aid)
        .filter(entity::application::Column::TenantId.eq(scope.tenant_id))
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("application not found".into()))?;

    let new_status = body.into_inner().status;
    let mut am: entity::application::ActiveModel = a.clone().into();
    am.status = Set(new_status.clone());
    let saved = am.update(&state.db).await?;

    if new_status == "Approved" {
        let _ = scheduler::enqueue(
            &state.db,
            scope.tenant_id,
            "auto_email",
            json!({ "template": "application_approved", "to": saved.email }),
            0,
        )
        .await;
    }

    Ok(Json(ApplicationResp::from(saved)))
}
