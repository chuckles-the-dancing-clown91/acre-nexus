//! Insurance policies on a property.

use super::{cents, date, ids_of, money, one_of, parse_id, property_documents, property_in, text};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::InsurancePolicy;
use rocket::serde::json::Json;
use rocket::{delete, get, post, put, State};
use schemars::JsonSchema;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const KINDS: &[&str] = &[
    "property",
    "liability",
    "flood",
    "earthquake",
    "umbrella",
    "builders_risk",
    "rent_loss",
    "other",
];
pub const STATUSES: &[&str] = &["active", "cancelled", "expired"];

#[derive(Deserialize, JsonSchema)]
pub struct PolicyReq {
    pub carrier: String,
    pub kind: Option<String>,
    pub status: Option<String>,
    pub policy_number: Option<String>,
    pub effective_on: Option<String>,
    pub expires_on: Option<String>,
    pub premium_cents: Option<i64>,
    pub coverage_cents: Option<i64>,
    pub deductible_cents: Option<i64>,
    pub agent_name: Option<String>,
    pub agent_phone: Option<String>,
    pub agent_email: Option<String>,
    #[serde(default)]
    pub document_ids: Vec<Uuid>,
    pub notes: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct PolicyDto {
    pub id: Uuid,
    pub property_id: Uuid,
    pub kind: String,
    pub carrier: String,
    pub policy_number: Option<String>,
    pub status: String,
    pub effective_on: Option<String>,
    pub expires_on: Option<String>,
    pub premium_cents: Option<i64>,
    pub premium_label: Option<String>,
    pub coverage_cents: Option<i64>,
    pub coverage_label: Option<String>,
    pub deductible_cents: Option<i64>,
    pub deductible_label: Option<String>,
    pub agent_name: Option<String>,
    pub agent_phone: Option<String>,
    pub agent_email: Option<String>,
    pub document_ids: Vec<Uuid>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<entity::insurance_policy::Model> for PolicyDto {
    fn from(p: entity::insurance_policy::Model) -> Self {
        PolicyDto {
            document_ids: ids_of(&p.document_ids),
            premium_label: money(p.premium_cents),
            coverage_label: money(p.coverage_cents),
            deductible_label: money(p.deductible_cents),
            id: p.id,
            property_id: p.property_id,
            kind: p.kind,
            carrier: p.carrier,
            policy_number: p.policy_number,
            status: p.status,
            effective_on: p.effective_on,
            expires_on: p.expires_on,
            premium_cents: p.premium_cents,
            coverage_cents: p.coverage_cents,
            deductible_cents: p.deductible_cents,
            agent_name: p.agent_name,
            agent_phone: p.agent_phone,
            agent_email: p.agent_email,
            notes: p.notes,
            created_at: p.created_at.to_rfc3339(),
            updated_at: p.updated_at.to_rfc3339(),
        }
    }
}

/// `GET /properties/<id>/insurance` — active policies first, soonest renewal
/// first.
#[rocket_okapi::openapi(tag = "Property Profile")]
#[get("/properties/<id>/insurance")]
pub async fn list_policies(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Vec<PolicyDto>>> {
    user.require(Permission::PropertyRead)?;
    let property = property_in(&db, scope.tenant_id, id).await?;
    let mut rows: Vec<PolicyDto> = InsurancePolicy::find()
        .filter(entity::insurance_policy::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::insurance_policy::Column::PropertyId.eq(property.id))
        .order_by_asc(entity::insurance_policy::Column::ExpiresOn)
        .all(&db)
        .await?
        .into_iter()
        .map(PolicyDto::from)
        .collect();
    rows.sort_by_key(|p| p.status != "active");
    Ok(Json(rows))
}

async fn fill(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    property_id: Uuid,
    am: &mut entity::insurance_policy::ActiveModel,
    b: PolicyReq,
) -> ApiResult<()> {
    let carrier = text(Some(b.carrier))
        .ok_or_else(|| ApiError::BadRequest("name the insurance carrier".into()))?;
    let effective_on = date("start date", b.effective_on)?;
    let expires_on = date("renewal date", b.expires_on)?;
    if let (Some(a), Some(z)) = (&effective_on, &expires_on) {
        if z < a {
            return Err(ApiError::BadRequest(
                "the renewal date is before the start date".into(),
            ));
        }
    }
    if let Some(e) = text(b.agent_email.clone()) {
        if !e.contains('@') {
            return Err(ApiError::BadRequest(
                "the agent's email doesn't look right".into(),
            ));
        }
    }
    am.carrier = Set(carrier);
    am.kind = Set(one_of("kind", b.kind, KINDS, "property")?);
    am.status = Set(one_of("status", b.status, STATUSES, "active")?);
    am.policy_number = Set(text(b.policy_number));
    am.effective_on = Set(effective_on);
    am.expires_on = Set(expires_on);
    am.premium_cents = Set(cents("premium", b.premium_cents)?);
    am.coverage_cents = Set(cents("coverage", b.coverage_cents)?);
    am.deductible_cents = Set(cents("deductible", b.deductible_cents)?);
    am.agent_name = Set(text(b.agent_name));
    am.agent_phone = Set(text(b.agent_phone));
    am.agent_email = Set(text(b.agent_email));
    am.document_ids = Set(property_documents(db, tenant_id, property_id, &b.document_ids).await?);
    am.notes = Set(text(b.notes));
    am.updated_at = Set(Utc::now().into());
    Ok(())
}

/// `POST /properties/<id>/insurance` — record a policy.
#[rocket_okapi::openapi(tag = "Property Profile")]
#[post("/properties/<id>/insurance", data = "<body>")]
pub async fn create_policy(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<PolicyReq>,
) -> ApiResult<Json<PolicyDto>> {
    user.require(Permission::PropertyWrite)?;
    let property = property_in(&db, scope.tenant_id, id).await?;
    let mut am = entity::insurance_policy::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        property_id: Set(property.id),
        created_by: Set(Some(user.user_id)),
        created_at: Set(Utc::now().into()),
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
    Ok(Json(PolicyDto::from(am.insert(&db).await?)))
}

async fn policy_of(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    property_id: Uuid,
    policy_id: &str,
) -> ApiResult<entity::insurance_policy::Model> {
    InsurancePolicy::find_by_id(parse_id(policy_id)?)
        .filter(entity::insurance_policy::Column::TenantId.eq(tenant_id))
        .filter(entity::insurance_policy::Column::PropertyId.eq(property_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("policy not found".into()))
}

/// `PUT /properties/<id>/insurance/<policy_id>` — change a policy.
#[rocket_okapi::openapi(tag = "Property Profile")]
#[put("/properties/<id>/insurance/<policy_id>", data = "<body>")]
pub async fn update_policy(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    policy_id: &str,
    body: Json<PolicyReq>,
) -> ApiResult<Json<PolicyDto>> {
    user.require(Permission::PropertyWrite)?;
    let property = property_in(&db, scope.tenant_id, id).await?;
    let row = policy_of(&db, scope.tenant_id, property.id, policy_id).await?;
    let mut am: entity::insurance_policy::ActiveModel = row.into();
    fill(
        &db,
        scope.tenant_id,
        property.id,
        &mut am,
        body.into_inner(),
    )
    .await?;
    Ok(Json(PolicyDto::from(am.update(&db).await?)))
}

/// `DELETE /properties/<id>/insurance/<policy_id>`
#[rocket_okapi::openapi(tag = "Property Profile")]
#[delete("/properties/<id>/insurance/<policy_id>")]
pub async fn delete_policy(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    policy_id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::PropertyWrite)?;
    let property = property_in(&db, scope.tenant_id, id).await?;
    let row = policy_of(&db, scope.tenant_id, property.id, policy_id).await?;
    InsurancePolicy::delete_by_id(row.id).exec(&db).await?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}
