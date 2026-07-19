//! `POST /leads/<id>/tour` — schedule a showing for a lead: drop a `tour`
//! reminder on the calendar (notified ahead through the substrate) and move the
//! lead forward in the pipeline.

use super::dto::{LeadDto, ScheduleTourReq, ScheduleTourResp};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::routes::reminders::dto::ReminderDto;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::{NaiveDate, Utc};
use entity::prelude::Lead;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::json;
use uuid::Uuid;

/// `POST /leads/<id>/tour` — schedule a tour for a lead.
#[rocket_okapi::openapi(tag = "Leads")]
#[post("/leads/<id>/tour", data = "<body>")]
pub async fn schedule_tour(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<ScheduleTourReq>,
) -> ApiResult<Json<ScheduleTourResp>> {
    user.require(Permission::ApplicationWrite)?;
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let b = body.into_inner();

    let lead = Lead::find_by_id(lid)
        .filter(entity::lead::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("lead not found".into()))?;

    NaiveDate::parse_from_str(&b.date, "%Y-%m-%d")
        .map_err(|_| ApiError::BadRequest("date must be YYYY-MM-DD".into()))?;

    let lead_days = match b.lead_days {
        Some(mut days) => {
            days.retain(|d| (0..=365).contains(d));
            days.sort_unstable();
            days.dedup();
            days.reverse();
            if days.is_empty() {
                return Err(ApiError::BadRequest(
                    "lead_days must contain at least one value in 0..=365".into(),
                ));
            }
            days
        }
        None => crate::reminders::parse_lead_days(
            &crate::settings::get_string(
                &db,
                scope.tenant_id,
                crate::settings::CALENDAR_DEFAULT_LEAD_DAYS,
            )
            .await,
        ),
    };

    let now = Utc::now();
    let description = b
        .notes
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| format!("Showing for {} ({})", lead.name, lead.email));
    let reminder = entity::reminder::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        subject_type: Set("tour".into()),
        subject_id: Set(Some(lead.id)),
        title: Set(format!("Tour — {}", lead.name)),
        description: Set(Some(description)),
        due_date: Set(b.date.clone()),
        lead_days: Set(json!(lead_days)),
        recipients: Set(json!([])),
        fired: Set(json!([])),
        status: Set("active".into()),
        completed_at: Set(None),
        created_by: Set(Some(user.user_id)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;

    // A booked tour means the prospect is engaged — move a brand-new lead into
    // the pipeline (never downgrade a more-advanced one).
    let lead = if lead.status == "new" {
        let mut am: entity::lead::ActiveModel = lead.into();
        am.status = Set("contacted".into());
        am.updated_at = Set(now.into());
        am.update(&db).await?
    } else {
        lead
    };

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::LEAD_TOUR_SCHEDULE,
        Some("lead"),
        Some(lead.id.to_string()),
        Some(scope.tenant_id),
        Some(json!({ "reminder_id": reminder.id, "date": b.date })),
    )
    .await;

    Ok(Json(ScheduleTourResp {
        lead: LeadDto::from(lead),
        reminder: ReminderDto::from_model(reminder, now.date_naive()),
    }))
}
