use super::dto::{CreateViolationReq, UpdateViolationReq, ViolationDto};
use super::{load_association, load_member, MODULE_KEY};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::HoaViolation;
use rocket::serde::json::Json;
use rocket::{get, patch, post, State};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

const STATUSES: [&str; 4] = ["open", "cured", "fined", "closed"];

/// `POST /hoa/associations/<association_id>/violations` — log a CC&R violation.
#[rocket_okapi::openapi(tag = "HOA")]
#[post("/hoa/associations/<association_id>/violations", data = "<body>")]
pub async fn create(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    association_id: &str,
    body: Json<CreateViolationReq>,
) -> ApiResult<Json<ViolationDto>> {
    user.require(Permission::HoaManage)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, MODULE_KEY).await?;
    let assoc = load_association(&db, scope.tenant_id, association_id).await?;
    let b = body.into_inner();
    load_member(&db, scope.tenant_id, assoc.id, b.member_id).await?;
    let kind = b.kind.trim().to_string();
    if kind.is_empty() {
        return Err(ApiError::BadRequest("kind is required".into()));
    }

    let m = entity::hoa_violation::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        association_id: Set(assoc.id),
        member_id: Set(b.member_id),
        kind: Set(kind),
        description: Set(b.description.unwrap_or_default()),
        status: Set("open".into()),
        fine_cents: Set(0),
        resolved_at: Set(None),
        created_at: Set(Utc::now().into()),
    }
    .insert(&db)
    .await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::HOA_VIOLATION_CREATE,
        Some("hoa_violation"),
        Some(m.id.to_string()),
        Some(scope.tenant_id),
        None,
    )
    .await;

    Ok(Json(ViolationDto::from(m)))
}

/// `PATCH /hoa/violations/<id>` — advance the enforcement lifecycle / set a fine.
#[rocket_okapi::openapi(tag = "HOA")]
#[patch("/hoa/violations/<id>", data = "<body>")]
pub async fn update(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateViolationReq>,
) -> ApiResult<Json<ViolationDto>> {
    user.require(Permission::HoaManage)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, MODULE_KEY).await?;
    let vid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let v = HoaViolation::find_by_id(vid)
        .one(&db)
        .await?
        .filter(|x| x.tenant_id == scope.tenant_id)
        .ok_or_else(|| ApiError::NotFound("violation not found".into()))?;

    let b = body.into_inner();
    let mut m = v.into_active_model();
    if let Some(s) = b.status {
        if !STATUSES.contains(&s.as_str()) {
            return Err(ApiError::BadRequest(format!("invalid status: {s}")));
        }
        // Cured/closed stamps a resolution time; reopening clears it.
        m.resolved_at = Set(if s == "cured" || s == "closed" {
            Some(Utc::now().into())
        } else {
            None
        });
        m.status = Set(s);
    }
    if let Some(f) = b.fine_cents {
        m.fine_cents = Set(f.max(0));
    }
    let saved = m.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::HOA_VIOLATION_UPDATE,
        Some("hoa_violation"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "status": saved.status, "fine_cents": saved.fine_cents })),
    )
    .await;

    Ok(Json(ViolationDto::from(saved)))
}

/// `GET /hoa/associations/<association_id>/violations` — violation log.
#[rocket_okapi::openapi(tag = "HOA")]
#[get("/hoa/associations/<association_id>/violations")]
pub async fn list(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    association_id: &str,
) -> ApiResult<Json<Vec<ViolationDto>>> {
    user.require(Permission::HoaRead)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, MODULE_KEY).await?;
    let assoc = load_association(&db, scope.tenant_id, association_id).await?;
    let rows = HoaViolation::find()
        .filter(entity::hoa_violation::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::hoa_violation::Column::AssociationId.eq(assoc.id))
        .order_by_desc(entity::hoa_violation::Column::CreatedAt)
        .all(&db)
        .await?;
    Ok(Json(rows.into_iter().map(ViolationDto::from).collect()))
}
