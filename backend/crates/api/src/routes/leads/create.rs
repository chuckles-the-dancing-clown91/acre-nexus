//! `POST /leads` — manually enter a prospect into the CRM (walk-in, phone,
//! referral). Inbound leasing email creates leads automatically; this is the
//! front-desk door for everything else.

use super::dto::{CreateLeadReq, LeadDto, SOURCES};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, Set};
use serde_json::json;
use uuid::Uuid;

/// `POST /leads` — create a lead.
#[rocket_okapi::openapi(tag = "Leads")]
#[post("/leads", data = "<body>")]
pub async fn create_lead(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<CreateLeadReq>,
) -> ApiResult<Json<LeadDto>> {
    user.require(Permission::ApplicationWrite)?;
    let b = body.into_inner();

    let name = b.name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::BadRequest("lead name is required".into()));
    }
    let email = b.email.trim().to_lowercase();
    if !email.contains('@') {
        return Err(ApiError::BadRequest(format!("invalid lead email '{email}'")));
    }
    let source = b.source.unwrap_or_else(|| "manual".into());
    if !SOURCES.contains(&source.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "invalid source '{source}' (expected one of {})",
            SOURCES.join(", ")
        )));
    }

    let now = Utc::now();
    let saved = entity::lead::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        name: Set(name.clone()),
        email: Set(email),
        phone: Set(b.phone.filter(|p| !p.trim().is_empty())),
        source: Set(source),
        status: Set("new".into()),
        notes: Set(b.notes.filter(|n| !n.trim().is_empty())),
        last_message: Set(None),
        application_id: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::LEAD_CREATE,
        Some("lead"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(json!({ "name": name, "source": saved.source })),
    )
    .await;

    Ok(Json(LeadDto::from(saved)))
}
