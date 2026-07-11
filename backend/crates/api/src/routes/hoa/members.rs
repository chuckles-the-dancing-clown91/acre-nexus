use super::dto::{CreateMemberReq, MemberDto};
use super::{load_association, MODULE_KEY};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::HoaMember;
use rocket::serde::json::Json;
use rocket::{get, post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use uuid::Uuid;

/// `GET /hoa/associations/<association_id>/members` — homeowners in the association.
#[rocket_okapi::openapi(tag = "HOA")]
#[get("/hoa/associations/<association_id>/members")]
pub async fn list(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    association_id: &str,
) -> ApiResult<Json<Vec<MemberDto>>> {
    user.require(Permission::HoaRead)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, MODULE_KEY).await?;
    let assoc = load_association(&db, scope.tenant_id, association_id).await?;
    let rows = HoaMember::find()
        .filter(entity::hoa_member::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::hoa_member::Column::AssociationId.eq(assoc.id))
        .order_by_asc(entity::hoa_member::Column::Name)
        .all(&db)
        .await?;
    Ok(Json(rows.into_iter().map(MemberDto::from).collect()))
}

/// `POST /hoa/associations/<association_id>/members` — add a homeowner.
#[rocket_okapi::openapi(tag = "HOA")]
#[post("/hoa/associations/<association_id>/members", data = "<body>")]
pub async fn create(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    association_id: &str,
    body: Json<CreateMemberReq>,
) -> ApiResult<Json<MemberDto>> {
    user.require(Permission::HoaManage)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, MODULE_KEY).await?;
    let assoc = load_association(&db, scope.tenant_id, association_id).await?;
    let b = body.into_inner();
    let name = b.name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::BadRequest("name is required".into()));
    }

    let m = entity::hoa_member::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        association_id: Set(assoc.id),
        name: Set(name),
        unit_label: Set(b.unit_label),
        email: Set(b.email),
        phone: Set(b.phone),
        status: Set("active".into()),
        created_at: Set(Utc::now().into()),
    }
    .insert(&db)
    .await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::HOA_MEMBER_CREATE,
        Some("hoa_member"),
        Some(m.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "association_id": assoc.id })),
    )
    .await;

    Ok(Json(MemberDto::from(m)))
}
