use super::draws::build_draw_detail;
use super::dto::{CreateLienWaiverReq, RehabDrawDetailDto, UpdateLienWaiverReq};
use super::{store_waiver_pdf, waiver_body, WAIVER_TYPES};
use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{Counterparty, Property, RehabLienWaiver, RehabProject};
use rocket::serde::json::Json;
use rocket::{patch, post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set};
use uuid::Uuid;

/// `POST /rehab-draws/<id>/lien-waivers` — generate a lien waiver for the draw:
/// render the statutory waiver text to a PDF, file it in the document service,
/// and record the waiver. Defaults amount/contractor from the draw.
#[rocket_okapi::openapi(tag = "Rehab")]
#[post("/rehab-draws/<id>/lien-waivers", data = "<body>")]
pub async fn create(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<CreateLienWaiverReq>,
) -> ApiResult<Json<RehabDrawDetailDto>> {
    user.require(Permission::RehabManage)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "rehab").await?;
    let draw = super::load_draw(&db, scope.tenant_id, id).await?;
    let b = body.into_inner();
    if !WAIVER_TYPES.contains(&b.waiver_type.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "invalid waiver_type: {} (expected one of {})",
            b.waiver_type,
            WAIVER_TYPES.join(", ")
        )));
    }

    // Resolve the contractor: explicit id/name on the request, else the draw's.
    let contractor_id = b.contractor_id.or(draw.contractor_id);
    let contractor_name = match (&b.contractor_name, contractor_id) {
        (Some(n), _) if !n.trim().is_empty() => n.trim().to_string(),
        (_, Some(cid)) => Counterparty::find_by_id(cid)
            .filter(entity::counterparty::Column::TenantId.eq(scope.tenant_id))
            .one(&db)
            .await?
            .map(|c| c.name)
            .unwrap_or_else(|| "Contractor".into()),
        _ => "Contractor".into(),
    };
    let amount = b.amount_cents.unwrap_or(draw.amount_cents).max(0);

    // Property address for the waiver body.
    let project = RehabProject::find_by_id(draw.project_id)
        .filter(entity::rehab_project::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("rehab project not found".into()))?;
    let property_address = Property::find_by_id(project.property_id)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .map(|p| format!("{}, {}", p.address, p.city))
        .unwrap_or_default();

    let today = Utc::now().date_naive().format("%Y-%m-%d").to_string();
    let body_text = waiver_body(
        &b.waiver_type,
        &contractor_name,
        &property_address,
        &usd(amount),
        b.through_date.as_deref(),
        &today,
    );
    let filename = format!("lien-waiver-draw-{}-{}.pdf", draw.number, b.waiver_type);
    let document_id =
        store_waiver_pdf(&db, scope.tenant_id, draw.id, &filename, &body_text).await?;

    let waiver = entity::rehab_lien_waiver::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        draw_id: Set(draw.id),
        project_id: Set(draw.project_id),
        waiver_type: Set(b.waiver_type.clone()),
        contractor_id: Set(contractor_id),
        contractor_name: Set(contractor_name),
        amount_cents: Set(amount),
        through_date: Set(b.through_date),
        status: Set("generated".into()),
        document_id: Set(Some(document_id)),
        created_at: Set(Utc::now().into()),
    }
    .insert(&db)
    .await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::REHAB_LIEN_WAIVER,
        Some("rehab_lien_waiver"),
        Some(waiver.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "waiver_type": waiver.waiver_type, "document_id": document_id })),
    )
    .await;

    Ok(Json(build_draw_detail(&db, scope.tenant_id, &draw).await?))
}

/// `PATCH /rehab-lien-waivers/<id>` — mark a waiver's signed copy `received`.
#[rocket_okapi::openapi(tag = "Rehab")]
#[patch("/rehab-lien-waivers/<id>", data = "<body>")]
pub async fn update(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateLienWaiverReq>,
) -> ApiResult<Json<RehabDrawDetailDto>> {
    user.require(Permission::RehabManage)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "rehab").await?;
    let wid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid waiver id".into()))?;
    let waiver = RehabLienWaiver::find_by_id(wid)
        .filter(entity::rehab_lien_waiver::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("lien waiver not found".into()))?;
    let status = body.into_inner().status;
    if !["generated", "received"].contains(&status.as_str()) {
        return Err(ApiError::BadRequest(format!("invalid status: {status}")));
    }
    let draw_id = waiver.draw_id;
    let mut m = waiver.into_active_model();
    m.status = Set(status);
    m.update(&db).await?;

    let draw = super::load_draw(&db, scope.tenant_id, &draw_id.to_string()).await?;
    Ok(Json(build_draw_detail(&db, scope.tenant_id, &draw).await?))
}
