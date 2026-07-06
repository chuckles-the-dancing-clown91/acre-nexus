//! Screening report + adverse action for one application (Phase 4, epic #8).
//!
//! * `GET /applications/<id>/screening` — the stored consumer report
//!   (`screening:read`; more sensitive than the application itself).
//! * `POST /applications/<id>/adverse-action` — send + file the FCRA §615(a)
//!   adverse-action notice for a declined application (`application:write`).

use super::dto::ApplicationResp;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{Application, ScreeningReport};
use rocket::serde::json::Json;
use rocket::{get, post, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::Serialize;
use uuid::Uuid;

/// The stored screening (consumer) report for an application.
#[derive(Serialize, schemars::JsonSchema)]
pub struct ScreeningReportResp {
    pub id: Uuid,
    pub application_id: Uuid,
    /// Provider key (`checkr`).
    pub provider: String,
    /// `pending` | `in_progress` | `complete` | `failed`.
    pub status: String,
    pub include_credit: bool,
    pub include_criminal: bool,
    pub include_eviction: bool,
    /// When the applicant authorized the report (FCRA §604(b)).
    pub consent_at: Option<String>,
    pub credit_score: Option<i32>,
    pub criminal_records: Option<i32>,
    pub eviction_records: Option<i32>,
    /// Provider assessment: `clear` | `consider`.
    pub recommendation: Option<String>,
    /// Policy verdict once landed: `cleared` | `failed`.
    pub result: Option<String>,
    /// The policy checks that tripped (empty when cleared).
    pub reasons: Vec<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
}

impl From<entity::screening_report::Model> for ScreeningReportResp {
    fn from(r: entity::screening_report::Model) -> Self {
        let reasons = r
            .reasons
            .as_ref()
            .and_then(|v| v.as_array().cloned())
            .map(|a| {
                a.into_iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        ScreeningReportResp {
            id: r.id,
            application_id: r.application_id,
            provider: r.provider,
            status: r.status,
            include_credit: r.include_credit,
            include_criminal: r.include_criminal,
            include_eviction: r.include_eviction,
            consent_at: r.consent_at.map(|x| x.to_rfc3339()),
            credit_score: r.credit_score,
            criminal_records: r.criminal_records,
            eviction_records: r.eviction_records,
            recommendation: r.recommendation,
            result: r.result,
            reasons,
            completed_at: r.completed_at.map(|x| x.to_rfc3339()),
            created_at: r.created_at.to_rfc3339(),
        }
    }
}

/// `GET /applications/<id>/screening` — the application's screening report.
#[rocket_okapi::openapi(tag = "Applications")]
#[get("/applications/<id>/screening")]
pub async fn get_screening(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<ScreeningReportResp>> {
    user.require(Permission::ScreeningRead)?;
    let aid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    // The application anchors tenancy — a report is never served across it.
    Application::find_by_id(aid)
        .filter(entity::application::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("application not found".into()))?;
    let report = ScreeningReport::find()
        .filter(entity::screening_report::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::screening_report::Column::ApplicationId.eq(aid))
        .one(&db)
        .await?
        .ok_or_else(|| {
            ApiError::NotFound("no screening report exists for this application".into())
        })?;
    Ok(Json(ScreeningReportResp::from(report)))
}

/// `POST /applications/<id>/adverse-action` — send + file the FCRA §615(a)
/// notice for a declined application whose report carried adverse information.
#[rocket_okapi::openapi(tag = "Applications")]
#[post("/applications/<id>/adverse-action")]
pub async fn adverse_action(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<ApplicationResp>> {
    user.require(Permission::ApplicationWrite)?;
    let aid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let app = Application::find_by_id(aid)
        .filter(entity::application::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("application not found".into()))?;
    if app.status != "Declined" {
        return Err(ApiError::BadRequest(
            "adverse-action notices apply to declined applications".into(),
        ));
    }
    let saved =
        crate::screening::send_adverse_action(&db, scope.tenant_id, Some(user.user_id), app)
            .await?;
    Ok(Json(ApplicationResp::from(saved)))
}
