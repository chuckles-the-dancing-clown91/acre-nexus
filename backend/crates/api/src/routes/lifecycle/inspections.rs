//! Move-in / move-out inspection routes. Staff manage the checklist
//! (`lease:read` / `lease:manage`); residents get a read-only view of their
//! own lease's inspections via `GET /my/inspections`.

use super::dto::{
    inspection_dto, AddInspectionItemReq, CreateInspectionReq, InspectionDetailDto, InspectionDto,
    InspectionItemDto, UpdateInspectionItemReq, UpdateInspectionReq,
};
use super::{CONDITIONS, DEFAULT_CHECKLIST};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{Inspection, InspectionItem, Lease};
use rocket::serde::json::Json;
use rocket::{delete, get, patch, post, State};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, ModelTrait, QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

/// A tenant-scoped lease, or 404.
async fn find_lease(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    id: &str,
) -> ApiResult<entity::lease::Model> {
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    Lease::find_by_id(lid)
        .filter(entity::lease::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("lease not found".into()))
}

/// A tenant-scoped inspection, or 404.
async fn find_inspection(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    id: &str,
) -> ApiResult<entity::inspection::Model> {
    let iid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    Inspection::find_by_id(iid)
        .filter(entity::inspection::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("inspection not found".into()))
}

/// All items for one inspection, in checklist order.
async fn items_for(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    inspection_id: Uuid,
) -> ApiResult<Vec<entity::inspection_item::Model>> {
    Ok(InspectionItem::find()
        .filter(entity::inspection_item::Column::TenantId.eq(tenant_id))
        .filter(entity::inspection_item::Column::InspectionId.eq(inspection_id))
        .order_by_asc(entity::inspection_item::Column::SortOrder)
        .all(db)
        .await?)
}

fn detail(
    inspection: entity::inspection::Model,
    items: Vec<entity::inspection_item::Model>,
) -> InspectionDetailDto {
    let rated = items.iter().filter(|i| i.condition != "unrated").count() as i64;
    InspectionDetailDto {
        inspection: inspection_dto(inspection, items.len() as i64, rated),
        items: items.into_iter().map(InspectionItemDto::from).collect(),
    }
}

/// `POST /leases/<id>/inspections` — open a move-in / move-out inspection on
/// a lease, pre-populated with the standard checklist (pass `blank: true` to
/// start empty).
#[rocket_okapi::openapi(tag = "Lifecycle")]
#[post("/leases/<id>/inspections", data = "<body>")]
pub async fn create_inspection(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<CreateInspectionReq>,
) -> ApiResult<Json<InspectionDetailDto>> {
    user.require(Permission::LeaseManage)?;
    let lease = find_lease(&db, scope.tenant_id, id).await?;
    let b = body.into_inner();
    let kind = b.kind.trim().to_lowercase();
    if !matches!(kind.as_str(), "move_in" | "move_out") {
        return Err(ApiError::BadRequest("kind must be move_in|move_out".into()));
    }

    let now = Utc::now();
    let inspection = entity::inspection::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        lease_id: Set(lease.id),
        property_id: Set(lease.property_id),
        unit_id: Set(lease.unit_id),
        kind: Set(kind.clone()),
        status: Set("draft".into()),
        scheduled_date: Set(b.scheduled_date.filter(|d| !d.trim().is_empty())),
        completed_at: Set(None),
        completed_by: Set(None),
        notes: Set(b.notes.filter(|n| !n.trim().is_empty())),
        created_by: Set(Some(user.user_id)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;

    if !b.blank.unwrap_or(false) {
        for (idx, (area, item)) in DEFAULT_CHECKLIST.iter().enumerate() {
            entity::inspection_item::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(scope.tenant_id),
                inspection_id: Set(inspection.id),
                area: Set((*area).to_string()),
                item: Set((*item).to_string()),
                condition: Set("unrated".into()),
                notes: Set(None),
                sort_order: Set(idx as i32),
                created_at: Set(now.into()),
            }
            .insert(&db)
            .await?;
        }
    }

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::INSPECTION_CREATE,
        Some("inspection"),
        Some(inspection.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "lease_id": lease.id, "kind": kind })),
    )
    .await;

    let items = items_for(&db, scope.tenant_id, inspection.id).await?;
    Ok(Json(detail(inspection, items)))
}

/// `GET /leases/<id>/inspections` — the lease's inspections, newest first.
#[rocket_okapi::openapi(tag = "Lifecycle")]
#[get("/leases/<id>/inspections")]
pub async fn list_lease_inspections(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Vec<InspectionDto>>> {
    user.require(Permission::LeaseRead)?;
    let lease = find_lease(&db, scope.tenant_id, id).await?;
    let rows = Inspection::find()
        .filter(entity::inspection::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::inspection::Column::LeaseId.eq(lease.id))
        .order_by_desc(entity::inspection::Column::CreatedAt)
        .all(&db)
        .await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let items = items_for(&db, scope.tenant_id, r.id).await?;
        let rated = items.iter().filter(|i| i.condition != "unrated").count() as i64;
        out.push(inspection_dto(r, items.len() as i64, rated));
    }
    Ok(Json(out))
}

/// `GET /inspections/<id>` — one inspection with its full checklist.
#[rocket_okapi::openapi(tag = "Lifecycle")]
#[get("/inspections/<id>")]
pub async fn get_inspection(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<InspectionDetailDto>> {
    user.require(Permission::LeaseRead)?;
    let inspection = find_inspection(&db, scope.tenant_id, id).await?;
    let items = items_for(&db, scope.tenant_id, inspection.id).await?;
    Ok(Json(detail(inspection, items)))
}

/// `PATCH /inspections/<id>` — update schedule/notes on a draft inspection.
#[rocket_okapi::openapi(tag = "Lifecycle")]
#[patch("/inspections/<id>", data = "<body>")]
pub async fn update_inspection(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateInspectionReq>,
) -> ApiResult<Json<InspectionDetailDto>> {
    user.require(Permission::LeaseManage)?;
    let inspection = find_inspection(&db, scope.tenant_id, id).await?;
    if inspection.status != "draft" {
        return Err(ApiError::BadRequest(
            "a completed inspection is read-only".into(),
        ));
    }
    let b = body.into_inner();
    let iid = inspection.id;
    let mut am: entity::inspection::ActiveModel = inspection.into();
    if let Some(v) = b.scheduled_date {
        am.scheduled_date = Set(Some(v).filter(|d| !d.trim().is_empty()));
    }
    if let Some(v) = b.notes {
        am.notes = Set(Some(v).filter(|n| !n.trim().is_empty()));
    }
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::INSPECTION_UPDATE,
        Some("inspection"),
        Some(iid.to_string()),
        Some(scope.tenant_id),
        None,
    )
    .await;

    let items = items_for(&db, scope.tenant_id, saved.id).await?;
    Ok(Json(detail(saved, items)))
}

/// `POST /inspections/<id>/complete` — freeze the inspection: stamps
/// completion and makes the checklist read-only.
#[rocket_okapi::openapi(tag = "Lifecycle")]
#[post("/inspections/<id>/complete")]
pub async fn complete_inspection(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<InspectionDetailDto>> {
    user.require(Permission::LeaseManage)?;
    let inspection = find_inspection(&db, scope.tenant_id, id).await?;
    if inspection.status != "draft" {
        return Err(ApiError::BadRequest(
            "inspection is already completed".into(),
        ));
    }
    let iid = inspection.id;
    let now = Utc::now();
    let mut am: entity::inspection::ActiveModel = inspection.into();
    am.status = Set("completed".into());
    am.completed_at = Set(Some(now.into()));
    am.completed_by = Set(Some(user.user_id));
    am.updated_at = Set(now.into());
    let saved = am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::INSPECTION_COMPLETE,
        Some("inspection"),
        Some(iid.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "kind": saved.kind })),
    )
    .await;

    let items = items_for(&db, scope.tenant_id, saved.id).await?;
    Ok(Json(detail(saved, items)))
}

/// `POST /inspections/<id>/items` — add a checklist line.
#[rocket_okapi::openapi(tag = "Lifecycle")]
#[post("/inspections/<id>/items", data = "<body>")]
pub async fn add_item(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<AddInspectionItemReq>,
) -> ApiResult<Json<InspectionItemDto>> {
    user.require(Permission::LeaseManage)?;
    let inspection = find_inspection(&db, scope.tenant_id, id).await?;
    if inspection.status != "draft" {
        return Err(ApiError::BadRequest(
            "a completed inspection is read-only".into(),
        ));
    }
    let b = body.into_inner();
    let area = b.area.trim().to_string();
    let item = b.item.trim().to_string();
    if area.is_empty() || item.is_empty() {
        return Err(ApiError::BadRequest("area and item are required".into()));
    }
    let next_sort = items_for(&db, scope.tenant_id, inspection.id)
        .await?
        .last()
        .map(|i| i.sort_order + 1)
        .unwrap_or(0);

    let saved = entity::inspection_item::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        inspection_id: Set(inspection.id),
        area: Set(area),
        item: Set(item),
        condition: Set("unrated".into()),
        notes: Set(None),
        sort_order: Set(next_sort),
        created_at: Set(Utc::now().into()),
    }
    .insert(&db)
    .await?;
    Ok(Json(InspectionItemDto::from(saved)))
}

/// `PATCH /inspection-items/<id>` — rate a checklist line / note damage.
#[rocket_okapi::openapi(tag = "Lifecycle")]
#[patch("/inspection-items/<id>", data = "<body>")]
pub async fn update_item(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateInspectionItemReq>,
) -> ApiResult<Json<InspectionItemDto>> {
    user.require(Permission::LeaseManage)?;
    let iid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let item = InspectionItem::find_by_id(iid)
        .filter(entity::inspection_item::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("item not found".into()))?;
    let inspection = Inspection::find_by_id(item.inspection_id)
        .filter(entity::inspection::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("inspection not found".into()))?;
    if inspection.status != "draft" {
        return Err(ApiError::BadRequest(
            "a completed inspection is read-only".into(),
        ));
    }
    let b = body.into_inner();
    let mut am: entity::inspection_item::ActiveModel = item.into();
    if let Some(c) = b.condition {
        let c = c.trim().to_lowercase();
        if !CONDITIONS.contains(&c.as_str()) {
            return Err(ApiError::BadRequest(format!(
                "invalid condition: {c} (expected one of {})",
                CONDITIONS.join(", ")
            )));
        }
        am.condition = Set(c);
    }
    if let Some(n) = b.notes {
        am.notes = Set(Some(n).filter(|n| !n.trim().is_empty()));
    }
    let saved = am.update(&db).await?;
    Ok(Json(InspectionItemDto::from(saved)))
}

/// `DELETE /inspection-items/<id>` — remove a checklist line.
#[rocket_okapi::openapi(tag = "Lifecycle")]
#[delete("/inspection-items/<id>")]
pub async fn delete_item(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::LeaseManage)?;
    let iid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let item = InspectionItem::find_by_id(iid)
        .filter(entity::inspection_item::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("item not found".into()))?;
    let inspection = Inspection::find_by_id(item.inspection_id)
        .filter(entity::inspection::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("inspection not found".into()))?;
    if inspection.status != "draft" {
        return Err(ApiError::BadRequest(
            "a completed inspection is read-only".into(),
        ));
    }
    item.delete(&db).await?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}

/// `GET /my/inspections` — the resident's own lease's inspections (read-only),
/// checklist included.
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[get("/my/inspections")]
pub async fn my_inspections(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<InspectionDetailDto>>> {
    let lease = crate::payments::lease_for_user(&db, scope.tenant_id, user.user_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("no lease found for your account".into()))?;
    let rows = Inspection::find()
        .filter(entity::inspection::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::inspection::Column::LeaseId.eq(lease.id))
        .order_by_desc(entity::inspection::Column::CreatedAt)
        .all(&db)
        .await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let items = items_for(&db, scope.tenant_id, r.id).await?;
        out.push(detail(r, items));
    }
    Ok(Json(out))
}
