//! A vendor's compliance over HTTP: the W-9 (taxpayer id, encrypted; only the
//! last four are shown) and insurance certificates with expiry, plus a list of
//! vendors that need attention. Rules in [`crate::vendor_compliance`].

use crate::audit::actions as act;
use crate::audit::change::{self, Ctx};
use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use crate::vendor_compliance as vc;
use chrono::{Datelike, NaiveDate, Utc};
use entity::prelude::{Counterparty, VendorBill, VendorInsurance, VendorTaxProfile};
use rocket::serde::json::Json;
use rocket::{delete, get, post, put, State};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct W9Dto {
    pub legal_name: String,
    pub business_name: Option<String>,
    pub classification: String,
    pub tin_type: String,
    /// Only the last four digits, e.g. `•••-••-6789`.
    pub tin_masked: String,
    pub signed_on: Option<String>,
    pub document_id: Option<Uuid>,
    pub updated_at: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct InsuranceDto {
    pub id: Uuid,
    pub kind: String,
    pub carrier: String,
    pub policy_number: Option<String>,
    pub limit_cents: Option<i64>,
    pub limit_label: Option<String>,
    pub expires_on: String,
    /// `current` | `expiring` | `expired`.
    pub state: String,
    pub document_id: Option<Uuid>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ComplianceDto {
    pub counterparty_id: Uuid,
    pub name: String,
    pub w9: Option<W9Dto>,
    pub insurance: Vec<InsuranceDto>,
    /// General liability cover in force today.
    pub coi_current: bool,
    /// What needs doing, in words.
    pub problems: Vec<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct W9Req {
    pub legal_name: String,
    pub business_name: Option<String>,
    pub classification: String,
    pub tin_type: String,
    /// Nine digits, dashes allowed. Never returned.
    pub tin: String,
    pub signed_on: Option<String>,
    pub document_id: Option<Uuid>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct InsuranceReq {
    pub kind: String,
    pub carrier: String,
    pub policy_number: Option<String>,
    pub limit_cents: Option<i64>,
    pub expires_on: String,
    pub document_id: Option<Uuid>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct VendorAttention {
    pub counterparty_id: Uuid,
    pub name: String,
    pub paid_this_year_cents: i64,
    pub paid_this_year_label: String,
    pub problems: Vec<String>,
}

fn pid(raw: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(raw).map_err(|_| ApiError::BadRequest("invalid id".into()))
}

fn clean(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

async fn vendor(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    id: Uuid,
) -> ApiResult<entity::counterparty::Model> {
    Counterparty::find_by_id(id)
        .filter(entity::counterparty::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("vendor not found".into()))
}

async fn compliance(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    c: &entity::counterparty::Model,
) -> ApiResult<ComplianceDto> {
    let today = Utc::now().date_naive();
    let w9 = VendorTaxProfile::find()
        .filter(entity::vendor_tax_profile::Column::TenantId.eq(tenant_id))
        .filter(entity::vendor_tax_profile::Column::CounterpartyId.eq(c.id))
        .one(db)
        .await?
        .map(|p| W9Dto {
            tin_masked: vc::mask_tin(&p.tin_type, &p.tin_last4),
            legal_name: p.legal_name,
            business_name: p.business_name,
            classification: p.classification,
            tin_type: p.tin_type,
            signed_on: p.signed_on,
            document_id: p.document_id,
            updated_at: p.updated_at.to_rfc3339(),
        });
    let insurance: Vec<InsuranceDto> = VendorInsurance::find()
        .filter(entity::vendor_insurance::Column::TenantId.eq(tenant_id))
        .filter(entity::vendor_insurance::Column::CounterpartyId.eq(c.id))
        .order_by_desc(entity::vendor_insurance::Column::ExpiresOn)
        .all(db)
        .await?
        .into_iter()
        .map(|i| InsuranceDto {
            state: vc::insurance_state(&i.expires_on, today).into(),
            limit_label: i.limit_cents.map(usd),
            id: i.id,
            kind: i.kind,
            carrier: i.carrier,
            policy_number: i.policy_number,
            limit_cents: i.limit_cents,
            expires_on: i.expires_on,
            document_id: i.document_id,
        })
        .collect();
    let coi_current = insurance
        .iter()
        .any(|i| i.kind == "general_liability" && i.state != "expired");
    let mut problems = vec![];
    if w9.is_none() {
        problems.push("No W-9 on file".to_string());
    }
    if !coi_current {
        problems.push("No current general liability insurance".to_string());
    } else if insurance
        .iter()
        .filter(|i| i.kind == "general_liability" && i.state != "expired")
        .all(|i| i.state == "expiring")
    {
        problems.push("Liability insurance ends within 30 days".to_string());
    }
    Ok(ComplianceDto {
        counterparty_id: c.id,
        name: c.name.clone(),
        w9,
        insurance,
        coi_current,
        problems,
    })
}

/// `GET /entities/<id>/compliance` — the vendor's W-9 and insurance.
#[rocket_okapi::openapi(tag = "Vendors")]
#[get("/entities/<id>/compliance")]
pub async fn get_compliance(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<ComplianceDto>> {
    user.require(Permission::EntityRead)?;
    let c = vendor(&db, scope.tenant_id, pid(id)?).await?;
    Ok(Json(compliance(&db, scope.tenant_id, &c).await?))
}

/// `PUT /entities/<id>/w9` — record the vendor's W-9. The taxpayer id is
/// encrypted and never returned.
#[rocket_okapi::openapi(tag = "Vendors")]
#[put("/entities/<id>/w9", data = "<body>")]
pub async fn save_w9(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<W9Req>,
) -> ApiResult<Json<ComplianceDto>> {
    user.require(Permission::EntityManage)?;
    let c = vendor(&db, scope.tenant_id, pid(id)?).await?;
    let b = body.into_inner();
    let legal = b.legal_name.trim().to_string();
    if legal.is_empty() {
        return Err(ApiError::BadRequest("legal_name is required".into()));
    }
    if !vc::CLASSIFICATIONS.contains(&b.classification.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "classification must be one of {}",
            vc::CLASSIFICATIONS.join(", ")
        )));
    }
    let digits = vc::validate_tin(&b.tin_type, &b.tin).map_err(ApiError::BadRequest)?;
    if let Some(d) = b.signed_on.as_deref().filter(|d| !d.trim().is_empty()) {
        NaiveDate::parse_from_str(d, "%Y-%m-%d")
            .map_err(|_| ApiError::BadRequest("signed_on must be YYYY-MM-DD".into()))?;
    }
    let sealed = crate::pii::encrypt(&crate::config::Config::global().pii_key, &digits)
        .map_err(ApiError::Internal)?;
    let last4 = digits[5..].to_string();
    let now = Utc::now();
    let existing = VendorTaxProfile::find()
        .filter(entity::vendor_tax_profile::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::vendor_tax_profile::Column::CounterpartyId.eq(c.id))
        .one(&db)
        .await?;
    match existing {
        Some(p) => {
            let mut am: entity::vendor_tax_profile::ActiveModel = p.into();
            am.legal_name = Set(legal);
            am.business_name = Set(clean(b.business_name));
            am.classification = Set(b.classification);
            am.tin_type = Set(b.tin_type);
            am.tin_ciphertext = Set(sealed.ciphertext);
            am.tin_nonce = Set(sealed.nonce);
            am.tin_last4 = Set(last4.clone());
            am.signed_on = Set(clean(b.signed_on));
            am.document_id = Set(b.document_id);
            am.updated_by = Set(Some(user.user_id));
            am.updated_at = Set(now.into());
            am.update(&db).await?;
        }
        None => {
            entity::vendor_tax_profile::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(scope.tenant_id),
                counterparty_id: Set(c.id),
                legal_name: Set(legal),
                business_name: Set(clean(b.business_name)),
                classification: Set(b.classification),
                tin_type: Set(b.tin_type),
                tin_ciphertext: Set(sealed.ciphertext),
                tin_nonce: Set(sealed.nonce),
                tin_last4: Set(last4.clone()),
                signed_on: Set(clean(b.signed_on)),
                document_id: Set(b.document_id),
                updated_by: Set(Some(user.user_id)),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }
            .insert(&db)
            .await?;
        }
    }
    // The trail says a W-9 was saved and which id it ends in, never the id.
    change::noted(
        &db,
        Ctx::new(&user, &scope),
        act::VENDOR_W9_SAVE,
        "counterparty",
        c.id,
        None,
        &format!("{} W-9", c.name),
        &format!("W-9 saved, taxpayer id ending {last4}"),
    )
    .await;
    Ok(Json(compliance(&db, scope.tenant_id, &c).await?))
}

/// `POST /entities/<id>/insurance` — add an insurance certificate.
#[rocket_okapi::openapi(tag = "Vendors")]
#[post("/entities/<id>/insurance", data = "<body>")]
pub async fn add_insurance(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<InsuranceReq>,
) -> ApiResult<Json<ComplianceDto>> {
    user.require(Permission::EntityManage)?;
    let c = vendor(&db, scope.tenant_id, pid(id)?).await?;
    let b = body.into_inner();
    if !vc::INSURANCE_KINDS.contains(&b.kind.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "kind must be one of {}",
            vc::INSURANCE_KINDS.join(", ")
        )));
    }
    let carrier = b.carrier.trim().to_string();
    if carrier.is_empty() {
        return Err(ApiError::BadRequest("carrier is required".into()));
    }
    NaiveDate::parse_from_str(b.expires_on.trim(), "%Y-%m-%d")
        .map_err(|_| ApiError::BadRequest("expires_on must be YYYY-MM-DD".into()))?;
    if b.limit_cents.is_some_and(|l| l < 0) {
        return Err(ApiError::BadRequest(
            "limit_cents cannot be negative".into(),
        ));
    }
    let now = Utc::now();
    let saved = entity::vendor_insurance::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        counterparty_id: Set(c.id),
        kind: Set(b.kind),
        carrier: Set(carrier),
        policy_number: Set(clean(b.policy_number)),
        limit_cents: Set(b.limit_cents),
        expires_on: Set(b.expires_on.trim().to_string()),
        document_id: Set(b.document_id),
        created_by: Set(Some(user.user_id)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;
    change::noted(
        &db,
        Ctx::new(&user, &scope),
        act::VENDOR_INSURANCE_ADD,
        "counterparty",
        c.id,
        None,
        &format!("{} insurance", c.name),
        &format!(
            "{} with {} added, ends {}",
            saved.kind.replace('_', " "),
            saved.carrier,
            saved.expires_on
        ),
    )
    .await;
    Ok(Json(compliance(&db, scope.tenant_id, &c).await?))
}

/// `DELETE /vendor-insurance/<id>` — remove a certificate entered by mistake.
#[rocket_okapi::openapi(tag = "Vendors")]
#[delete("/vendor-insurance/<id>")]
pub async fn remove_insurance(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<ComplianceDto>> {
    user.require(Permission::EntityManage)?;
    let i = VendorInsurance::find_by_id(pid(id)?)
        .filter(entity::vendor_insurance::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("certificate not found".into()))?;
    let c = vendor(&db, scope.tenant_id, i.counterparty_id).await?;
    VendorInsurance::delete_by_id(i.id).exec(&db).await?;
    change::removed(
        &db,
        Ctx::new(&user, &scope),
        act::VENDOR_INSURANCE_REMOVE,
        "counterparty",
        c.id,
        None,
        &format!("{} insurance with {}", c.name, i.carrier),
        None,
    )
    .await;
    Ok(Json(compliance(&db, scope.tenant_id, &c).await?))
}

/// `GET /compliance/vendors` — vendors that need something: contractors, and
/// anyone paid this year, without a W-9 or current insurance.
#[rocket_okapi::openapi(tag = "Vendors")]
#[get("/compliance/vendors")]
pub async fn attention(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<VendorAttention>>> {
    user.require(Permission::EntityRead)?;
    let year = Utc::now().year();
    let mut paid: HashMap<Uuid, i64> = HashMap::new();
    for b in VendorBill::find()
        .filter(entity::vendor_bill::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::vendor_bill::Column::Status.eq("paid"))
        .all(&db)
        .await?
    {
        if b.paid_at.is_some_and(|t| t.year() == year) {
            *paid.entry(b.counterparty_id).or_insert(0) += b.amount_cents;
        }
    }
    let vendors = Counterparty::find()
        .filter(entity::counterparty::Column::TenantId.eq(scope.tenant_id))
        .all(&db)
        .await?;
    let mut out = vec![];
    for c in vendors {
        let cents = paid.get(&c.id).copied().unwrap_or(0);
        if c.kind != "contractor" && cents == 0 {
            continue;
        }
        let comp = compliance(&db, scope.tenant_id, &c).await?;
        if comp.problems.is_empty() {
            continue;
        }
        out.push(VendorAttention {
            counterparty_id: c.id,
            name: c.name,
            paid_this_year_cents: cents,
            paid_this_year_label: usd(cents),
            problems: comp.problems,
        });
    }
    out.sort_by_key(|v| std::cmp::Reverse(v.paid_this_year_cents));
    Ok(Json(out))
}
