//! The **equipment registry** — AC units, water heaters, appliances and
//! other serviceable utilities per property (optionally per unit), with
//! make/model/serial and warranty tracking. Work orders reference the asset
//! being serviced; manuals and photos ride the document service
//! (`owner_type = "asset"`).

use super::dto::{AssetDto, CreateAssetReq, UpdateAssetReq};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::{NaiveDate, Utc};
use entity::prelude::{Asset, Property, Unit};
use rocket::serde::json::Json;
use rocket::{get, patch, post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use uuid::Uuid;

/// Asset kinds the API accepts.
pub const KINDS: &[&str] = &[
    "hvac",
    "appliance",
    "plumbing",
    "electrical",
    "safety",
    "structural",
    "other",
];

fn normalize_kind(raw: Option<String>) -> Result<String, ApiError> {
    match raw.as_deref().map(str::trim) {
        None | Some("") => Ok("other".into()),
        Some(k) if KINDS.contains(&k) => Ok(k.to_string()),
        Some(k) => Err(ApiError::BadRequest(format!(
            "invalid kind: {k} (expected one of {})",
            KINDS.join(", ")
        ))),
    }
}

fn valid_date(label: &str, d: &Option<String>) -> Result<Option<String>, ApiError> {
    match d.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        Some(d) => NaiveDate::parse_from_str(d, "%Y-%m-%d")
            .map(|_| Some(d.to_string()))
            .map_err(|_| ApiError::BadRequest(format!("{label} must be YYYY-MM-DD"))),
    }
}

fn clean(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// `GET /assets?property_id&unit_id&status` — the equipment registry.
#[rocket_okapi::openapi(tag = "Maintenance")]
#[get("/assets?<property_id>&<unit_id>&<status>")]
pub async fn list_assets(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    property_id: Option<String>,
    unit_id: Option<String>,
    status: Option<String>,
) -> ApiResult<Json<Vec<AssetDto>>> {
    user.require(Permission::MaintenanceRead)?;
    let mut q = Asset::find().filter(entity::asset::Column::TenantId.eq(scope.tenant_id));
    if let Some(pid) = property_id.filter(|s| !s.trim().is_empty()) {
        let pid = Uuid::parse_str(&pid)
            .map_err(|_| ApiError::BadRequest("invalid property_id".into()))?;
        q = q.filter(entity::asset::Column::PropertyId.eq(pid));
    }
    if let Some(uid) = unit_id.filter(|s| !s.trim().is_empty()) {
        let uid =
            Uuid::parse_str(&uid).map_err(|_| ApiError::BadRequest("invalid unit_id".into()))?;
        q = q.filter(entity::asset::Column::UnitId.eq(uid));
    }
    if let Some(s) = status.filter(|s| !s.trim().is_empty()) {
        q = q.filter(entity::asset::Column::Status.eq(s.trim().to_lowercase()));
    }
    let rows = q.order_by_asc(entity::asset::Column::Name).all(&db).await?;
    Ok(Json(rows.into_iter().map(AssetDto::from).collect()))
}

/// `POST /assets` — register a piece of equipment.
#[rocket_okapi::openapi(tag = "Maintenance")]
#[post("/assets", data = "<body>")]
pub async fn create_asset(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<CreateAssetReq>,
) -> ApiResult<Json<AssetDto>> {
    user.require(Permission::MaintenanceManage)?;
    let b = body.into_inner();
    let name = b.name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::BadRequest("name is required".into()));
    }
    let kind = normalize_kind(b.kind)?;
    let install_date = valid_date("install_date", &b.install_date)?;
    let warranty_expires = valid_date("warranty_expires", &b.warranty_expires)?;
    Property::find_by_id(b.property_id)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    if let Some(uid) = b.unit_id {
        Unit::find_by_id(uid)
            .filter(entity::unit::Column::TenantId.eq(scope.tenant_id))
            .filter(entity::unit::Column::PropertyId.eq(b.property_id))
            .one(&db)
            .await?
            .ok_or_else(|| ApiError::NotFound("unit not found on this property".into()))?;
    }

    let now = Utc::now();
    let saved = entity::asset::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        property_id: Set(b.property_id),
        unit_id: Set(b.unit_id),
        kind: Set(kind),
        name: Set(name),
        make: Set(clean(b.make)),
        model: Set(clean(b.model)),
        serial_number: Set(clean(b.serial_number)),
        install_date: Set(install_date),
        warranty_expires: Set(warranty_expires),
        notes: Set(clean(b.notes)),
        status: Set("active".into()),
        created_by: Set(Some(user.user_id)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::ASSET_CREATE,
        Some("asset"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({
            "property_id": saved.property_id,
            "kind": saved.kind,
            "name": saved.name,
        })),
    )
    .await;

    Ok(Json(AssetDto::from(saved)))
}

/// `PATCH /assets/<id>` — edit or retire a piece of equipment.
#[rocket_okapi::openapi(tag = "Maintenance")]
#[patch("/assets/<id>", data = "<body>")]
pub async fn update_asset(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateAssetReq>,
) -> ApiResult<Json<AssetDto>> {
    user.require(Permission::MaintenanceManage)?;
    let aid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let asset = Asset::find_by_id(aid)
        .filter(entity::asset::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("asset not found".into()))?;
    let b = body.into_inner();

    let mut am: entity::asset::ActiveModel = asset.into();
    if let Some(v) = b.name.map(|s| s.trim().to_string()) {
        if v.is_empty() {
            return Err(ApiError::BadRequest("name cannot be empty".into()));
        }
        am.name = Set(v);
    }
    if b.kind.is_some() {
        am.kind = Set(normalize_kind(b.kind)?);
    }
    if let Some(v) = b.make {
        am.make = Set(clean(Some(v)));
    }
    if let Some(v) = b.model {
        am.model = Set(clean(Some(v)));
    }
    if let Some(v) = b.serial_number {
        am.serial_number = Set(clean(Some(v)));
    }
    if b.install_date.is_some() {
        am.install_date = Set(valid_date("install_date", &b.install_date)?);
    }
    if b.warranty_expires.is_some() {
        am.warranty_expires = Set(valid_date("warranty_expires", &b.warranty_expires)?);
    }
    if let Some(v) = b.notes {
        am.notes = Set(clean(Some(v)));
    }
    if let Some(v) = b.status.map(|s| s.trim().to_lowercase()) {
        if !matches!(v.as_str(), "active" | "retired") {
            return Err(ApiError::BadRequest("status must be active|retired".into()));
        }
        am.status = Set(v);
    }
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::ASSET_UPDATE,
        Some("asset"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "status": saved.status })),
    )
    .await;

    Ok(Json(AssetDto::from(saved)))
}
