//! Permits pulled on a property.

use super::{cents, date, ids_of, money, one_of, parse_id, property_documents, property_in, text};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{PropertyPermit, Unit};
use rocket::serde::json::Json;
use rocket::{delete, get, post, put, State};
use schemars::JsonSchema;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const KINDS: &[&str] = &[
    "building",
    "electrical",
    "plumbing",
    "mechanical",
    "roofing",
    "demolition",
    "fence",
    "solar",
    "pool",
    "other",
];
pub const STATUSES: &[&str] = &[
    "applied",
    "issued",
    "inspection",
    "finaled",
    "expired",
    "void",
];

#[derive(Deserialize, JsonSchema)]
pub struct PermitReq {
    pub description: String,
    pub kind: Option<String>,
    pub status: Option<String>,
    pub permit_number: Option<String>,
    pub unit_id: Option<Uuid>,
    pub jurisdiction: Option<String>,
    pub applied_on: Option<String>,
    pub issued_on: Option<String>,
    pub expires_on: Option<String>,
    pub inspection_on: Option<String>,
    pub finaled_on: Option<String>,
    pub contractor_entity_id: Option<Uuid>,
    pub contractor_name: Option<String>,
    pub valuation_cents: Option<i64>,
    pub fee_cents: Option<i64>,
    pub ticket_id: Option<Uuid>,
    #[serde(default)]
    pub document_ids: Vec<Uuid>,
    pub notes: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct PermitDto {
    pub id: Uuid,
    pub property_id: Uuid,
    pub unit_id: Option<Uuid>,
    pub permit_number: Option<String>,
    pub kind: String,
    pub description: String,
    pub status: String,
    /// Still open with the building department (not finaled or void).
    pub open: bool,
    pub jurisdiction: Option<String>,
    pub applied_on: Option<String>,
    pub issued_on: Option<String>,
    pub expires_on: Option<String>,
    pub inspection_on: Option<String>,
    pub finaled_on: Option<String>,
    pub contractor_entity_id: Option<Uuid>,
    pub contractor_name: Option<String>,
    pub valuation_cents: Option<i64>,
    pub valuation_label: Option<String>,
    pub fee_cents: Option<i64>,
    pub fee_label: Option<String>,
    pub ticket_id: Option<Uuid>,
    pub document_ids: Vec<Uuid>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub fn is_open(status: &str) -> bool {
    !matches!(status, "finaled" | "void")
}

impl From<entity::property_permit::Model> for PermitDto {
    fn from(p: entity::property_permit::Model) -> Self {
        PermitDto {
            open: is_open(&p.status),
            document_ids: ids_of(&p.document_ids),
            valuation_label: money(p.valuation_cents),
            fee_label: money(p.fee_cents),
            id: p.id,
            property_id: p.property_id,
            unit_id: p.unit_id,
            permit_number: p.permit_number,
            kind: p.kind,
            description: p.description,
            status: p.status,
            jurisdiction: p.jurisdiction,
            applied_on: p.applied_on,
            issued_on: p.issued_on,
            expires_on: p.expires_on,
            inspection_on: p.inspection_on,
            finaled_on: p.finaled_on,
            contractor_entity_id: p.contractor_entity_id,
            contractor_name: p.contractor_name,
            valuation_cents: p.valuation_cents,
            fee_cents: p.fee_cents,
            ticket_id: p.ticket_id,
            notes: p.notes,
            created_at: p.created_at.to_rfc3339(),
            updated_at: p.updated_at.to_rfc3339(),
        }
    }
}

/// `GET /properties/<id>/permits` — open permits first, then newest.
#[rocket_okapi::openapi(tag = "Property Profile")]
#[get("/properties/<id>/permits")]
pub async fn list_permits(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Vec<PermitDto>>> {
    user.require(Permission::PropertyRead)?;
    let property = property_in(&db, scope.tenant_id, id).await?;
    let mut rows: Vec<PermitDto> = PropertyPermit::find()
        .filter(entity::property_permit::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::property_permit::Column::PropertyId.eq(property.id))
        .order_by_desc(entity::property_permit::Column::CreatedAt)
        .all(&db)
        .await?
        .into_iter()
        .map(PermitDto::from)
        .collect();
    rows.sort_by_key(|p| !p.open);
    Ok(Json(rows))
}

async fn fill(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    property_id: Uuid,
    am: &mut entity::property_permit::ActiveModel,
    b: PermitReq,
) -> ApiResult<()> {
    let description = text(Some(b.description))
        .ok_or_else(|| ApiError::BadRequest("say what the permit covers".into()))?;
    if let Some(u) = b.unit_id {
        Unit::find_by_id(u)
            .filter(entity::unit::Column::PropertyId.eq(property_id))
            .one(db)
            .await?
            .ok_or_else(|| ApiError::BadRequest("that unit isn't on this property".into()))?;
    }
    let status = one_of("status", b.status, STATUSES, "applied")?;
    let finaled_on = date("finaled date", b.finaled_on)?;
    am.unit_id = Set(b.unit_id);
    am.description = Set(description);
    am.kind = Set(one_of("kind", b.kind, KINDS, "building")?);
    am.permit_number = Set(text(b.permit_number));
    am.jurisdiction = Set(text(b.jurisdiction));
    am.applied_on = Set(date("applied date", b.applied_on)?);
    am.issued_on = Set(date("issued date", b.issued_on)?);
    am.expires_on = Set(date("expiry date", b.expires_on)?);
    am.inspection_on = Set(date("inspection date", b.inspection_on)?);
    // Finaled with no date means today.
    am.finaled_on = Set(match (&finaled_on, status.as_str()) {
        (None, "finaled") => Some(Utc::now().date_naive().to_string()),
        _ => finaled_on,
    });
    am.status = Set(status);
    am.contractor_entity_id = Set(b.contractor_entity_id);
    am.contractor_name = Set(text(b.contractor_name));
    am.valuation_cents = Set(cents("job value", b.valuation_cents)?);
    am.fee_cents = Set(cents("permit fee", b.fee_cents)?);
    am.ticket_id = Set(b.ticket_id);
    am.document_ids = Set(property_documents(db, tenant_id, property_id, &b.document_ids).await?);
    am.notes = Set(text(b.notes));
    am.updated_at = Set(Utc::now().into());
    Ok(())
}

/// `POST /properties/<id>/permits` — record a permit.
#[rocket_okapi::openapi(tag = "Property Profile")]
#[post("/properties/<id>/permits", data = "<body>")]
pub async fn create_permit(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<PermitReq>,
) -> ApiResult<Json<PermitDto>> {
    user.require(Permission::PropertyWrite)?;
    let property = property_in(&db, scope.tenant_id, id).await?;
    let now = Utc::now();
    let mut am = entity::property_permit::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        property_id: Set(property.id),
        created_by: Set(Some(user.user_id)),
        created_at: Set(now.into()),
        ..Default::default()
    };
    fill(
        &db,
        scope.tenant_id,
        property.id,
        &mut am,
        body.into_inner(),
    )
    .await?;
    let saved = am.insert(&db).await?;
    Ok(Json(PermitDto::from(saved)))
}

async fn permit_of(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    property_id: Uuid,
    permit_id: &str,
) -> ApiResult<entity::property_permit::Model> {
    PropertyPermit::find_by_id(parse_id(permit_id)?)
        .filter(entity::property_permit::Column::TenantId.eq(tenant_id))
        .filter(entity::property_permit::Column::PropertyId.eq(property_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("permit not found".into()))
}

/// `PUT /properties/<id>/permits/<permit_id>` — change a permit.
#[rocket_okapi::openapi(tag = "Property Profile")]
#[put("/properties/<id>/permits/<permit_id>", data = "<body>")]
pub async fn update_permit(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    permit_id: &str,
    body: Json<PermitReq>,
) -> ApiResult<Json<PermitDto>> {
    user.require(Permission::PropertyWrite)?;
    let property = property_in(&db, scope.tenant_id, id).await?;
    let row = permit_of(&db, scope.tenant_id, property.id, permit_id).await?;
    let mut am: entity::property_permit::ActiveModel = row.into();
    fill(
        &db,
        scope.tenant_id,
        property.id,
        &mut am,
        body.into_inner(),
    )
    .await?;
    Ok(Json(PermitDto::from(am.update(&db).await?)))
}

/// `DELETE /properties/<id>/permits/<permit_id>`
#[rocket_okapi::openapi(tag = "Property Profile")]
#[delete("/properties/<id>/permits/<permit_id>")]
pub async fn delete_permit(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    permit_id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::PropertyWrite)?;
    let property = property_in(&db, scope.tenant_id, id).await?;
    let row = permit_of(&db, scope.tenant_id, property.id, permit_id).await?;
    PropertyPermit::delete_by_id(row.id).exec(&db).await?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}
