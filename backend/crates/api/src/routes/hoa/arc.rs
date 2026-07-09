use super::dto::{ArcDto, CreateArcReq, DecideArcReq};
use super::{load_association, load_member, MODULE_KEY};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::HoaArcRequest;
use rocket::serde::json::Json;
use rocket::{get, post, State};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

const DECISIONS: [&str; 3] = ["approved", "denied", "withdrawn"];

/// `POST /hoa/associations/<association_id>/arc-requests` — submit an
/// architectural-review request.
#[rocket_okapi::openapi(tag = "HOA")]
#[post("/hoa/associations/<association_id>/arc-requests", data = "<body>")]
pub async fn create(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    association_id: &str,
    body: Json<CreateArcReq>,
) -> ApiResult<Json<ArcDto>> {
    user.require(Permission::HoaManage)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, MODULE_KEY).await?;
    let assoc = load_association(&db, scope.tenant_id, association_id).await?;
    let b = body.into_inner();
    load_member(&db, scope.tenant_id, assoc.id, b.member_id).await?;
    let title = b.title.trim().to_string();
    if title.is_empty() {
        return Err(ApiError::BadRequest("title is required".into()));
    }

    let m = entity::hoa_arc_request::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        association_id: Set(assoc.id),
        member_id: Set(b.member_id),
        title: Set(title),
        description: Set(b.description.unwrap_or_default()),
        status: Set("submitted".into()),
        decision_note: Set(None),
        decided_by: Set(None),
        decided_at: Set(None),
        created_at: Set(Utc::now().into()),
    }
    .insert(&db)
    .await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::HOA_ARC_CREATE,
        Some("hoa_arc_request"),
        Some(m.id.to_string()),
        Some(scope.tenant_id),
        None,
    )
    .await;

    Ok(Json(ArcDto::from(m)))
}

/// `POST /hoa/arc-requests/<id>/decide` — approve / deny / withdraw a request.
#[rocket_okapi::openapi(tag = "HOA")]
#[post("/hoa/arc-requests/<id>/decide", data = "<body>")]
pub async fn decide(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<DecideArcReq>,
) -> ApiResult<Json<ArcDto>> {
    user.require(Permission::HoaManage)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, MODULE_KEY).await?;
    let aid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let req = HoaArcRequest::find_by_id(aid)
        .one(&db)
        .await?
        .filter(|x| x.tenant_id == scope.tenant_id)
        .ok_or_else(|| ApiError::NotFound("ARC request not found".into()))?;

    let b = body.into_inner();
    if !DECISIONS.contains(&b.decision.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "invalid decision: {}",
            b.decision
        )));
    }
    if req.status != "submitted" {
        return Err(ApiError::BadRequest(format!(
            "request is already {}",
            req.status
        )));
    }

    let mut m = req.into_active_model();
    m.status = Set(b.decision);
    m.decision_note = Set(b.note);
    m.decided_by = Set(Some(user.user_id));
    m.decided_at = Set(Some(Utc::now().into()));
    let saved = m.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::HOA_ARC_DECIDE,
        Some("hoa_arc_request"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "status": saved.status })),
    )
    .await;

    Ok(Json(ArcDto::from(saved)))
}

/// `GET /hoa/associations/<association_id>/arc-requests` — ARC request log.
#[rocket_okapi::openapi(tag = "HOA")]
#[get("/hoa/associations/<association_id>/arc-requests")]
pub async fn list(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    association_id: &str,
) -> ApiResult<Json<Vec<ArcDto>>> {
    user.require(Permission::HoaRead)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, MODULE_KEY).await?;
    let assoc = load_association(&db, scope.tenant_id, association_id).await?;
    let rows = HoaArcRequest::find()
        .filter(entity::hoa_arc_request::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::hoa_arc_request::Column::AssociationId.eq(assoc.id))
        .order_by_desc(entity::hoa_arc_request::Column::CreatedAt)
        .all(&db)
        .await?;
    Ok(Json(rows.into_iter().map(ArcDto::from).collect()))
}
