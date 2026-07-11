use super::dto::{AssociationDto, CreateAssociationReq};
use super::{load_association, MODULE_KEY};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{HoaAssociation, HoaMember};
use rocket::serde::json::Json;
use rocket::{get, post, State};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

async fn member_count(db: &crate::db::RequestDb, tenant_id: Uuid, association_id: Uuid) -> i64 {
    HoaMember::find()
        .filter(entity::hoa_member::Column::TenantId.eq(tenant_id))
        .filter(entity::hoa_member::Column::AssociationId.eq(association_id))
        .count(db)
        .await
        .unwrap_or(0) as i64
}

fn dto(m: &entity::hoa_association::Model, members: i64) -> AssociationDto {
    AssociationDto {
        id: m.id,
        name: m.name.clone(),
        property_id: m.property_id,
        dues_cents: m.dues_cents,
        dues_frequency: m.dues_frequency.clone(),
        status: m.status.clone(),
        member_count: members,
    }
}

/// `GET /hoa/associations` — associations in the workspace.
#[rocket_okapi::openapi(tag = "HOA")]
#[get("/hoa/associations")]
pub async fn list(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<AssociationDto>>> {
    user.require(Permission::HoaRead)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, MODULE_KEY).await?;
    let rows = HoaAssociation::find()
        .filter(entity::hoa_association::Column::TenantId.eq(scope.tenant_id))
        .order_by_asc(entity::hoa_association::Column::Name)
        .all(&db)
        .await?;
    let mut out = Vec::with_capacity(rows.len());
    for a in &rows {
        let n = member_count(&db, scope.tenant_id, a.id).await;
        out.push(dto(a, n));
    }
    Ok(Json(out))
}

/// `POST /hoa/associations` — create a community association.
#[rocket_okapi::openapi(tag = "HOA")]
#[post("/hoa/associations", data = "<body>")]
pub async fn create(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<CreateAssociationReq>,
) -> ApiResult<Json<AssociationDto>> {
    user.require(Permission::HoaManage)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, MODULE_KEY).await?;
    let b = body.into_inner();
    let name = b.name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::BadRequest("name is required".into()));
    }
    let freq = match b.dues_frequency.as_deref().unwrap_or("monthly") {
        f @ ("monthly" | "quarterly" | "annual") => f.to_string(),
        other => {
            return Err(ApiError::BadRequest(format!(
                "invalid dues_frequency: {other}"
            )))
        }
    };

    let m = entity::hoa_association::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        name: Set(name.clone()),
        property_id: Set(b.property_id),
        dues_cents: Set(b.dues_cents.unwrap_or(0).max(0)),
        dues_frequency: Set(freq),
        status: Set("active".into()),
        created_at: Set(Utc::now().into()),
    }
    .insert(&db)
    .await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::HOA_ASSOCIATION_CREATE,
        Some("hoa_association"),
        Some(m.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "name": name })),
    )
    .await;

    // load_association re-validates tenant scope (defensive; also silences unused).
    let saved = load_association(&db, scope.tenant_id, &m.id.to_string()).await?;
    Ok(Json(dto(&saved, 0)))
}
