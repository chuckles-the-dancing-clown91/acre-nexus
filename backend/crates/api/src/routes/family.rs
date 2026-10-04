//! **Family-plan features** (roadmap area 17), staff side: related-party
//! reviews, linking a counterparty to the family, foundation mode on an LLC,
//! housing vouchers and income certifications on a lease, the compliance
//! list, and recording the housing authority's payment. See
//! [`crate::family`] for the rules.

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::family as fam;
use crate::rbac::Permission;
use crate::tenancy::{Access, TenantScope};
use chrono::{Months, NaiveDate, Utc};
use entity::prelude::{
    Counterparty, HousingVoucher, IncomeCertification, Lease, LeasePayment, Llc, Owner, Property,
    RelatedPartyReview,
};
use rocket::serde::json::Json;
use rocket::{delete, get, patch, post, put};
use schemars::JsonSchema;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

fn uuid(s: &str, what: &str) -> ApiResult<Uuid> {
    Uuid::parse_str(s).map_err(|_| ApiError::NotFound(format!("{what} not found")))
}

fn date(s: &str, what: &str) -> ApiResult<NaiveDate> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d")
        .map_err(|_| ApiError::BadRequest(format!("{what} must be YYYY-MM-DD")))
}

fn text(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

// ---- related parties ---------------------------------------------------------

#[derive(Serialize, JsonSchema)]
pub struct PartyDto {
    pub id: Uuid,
    pub name: String,
}

#[derive(Serialize, JsonSchema)]
pub struct ReviewDto {
    pub id: Uuid,
    pub subject_type: String,
    pub subject_id: Option<Uuid>,
    pub entity_id: Option<Uuid>,
    pub entity_name: Option<String>,
    pub counterparty_id: Option<Uuid>,
    pub counterparty_name: Option<String>,
    pub summary: String,
    pub reason: String,
    pub amount_cents: Option<i64>,
    pub market_cents: Option<i64>,
    pub market_note: Option<String>,
    pub parties: Vec<PartyDto>,
    pub status: String,
    pub decided_by: Option<Uuid>,
    pub decided_at: Option<String>,
    pub decision_note: Option<String>,
    pub created_at: String,
    /// Why the signed-in user can't decide it, if they can't.
    pub cannot_decide: Option<String>,
}

struct Names {
    llcs: HashMap<Uuid, String>,
    vendors: HashMap<Uuid, String>,
    owners: HashMap<Uuid, String>,
    mine: Vec<Uuid>,
}

async fn names(db: &crate::db::RequestDb, tenant_id: Uuid, user_id: Uuid) -> ApiResult<Names> {
    Ok(Names {
        llcs: crate::payouts::entity_names(db, tenant_id).await?,
        vendors: crate::payables::vendor_names(db, tenant_id).await?,
        owners: Owner::find()
            .filter(entity::owner::Column::TenantId.eq(tenant_id))
            .all(db)
            .await?
            .into_iter()
            .map(|o| (o.id, o.name))
            .collect(),
        mine: fam::owner_ids_of(db, tenant_id, user_id).await?,
    })
}

fn review_dto(r: entity::related_party_review::Model, n: &Names, user_id: Uuid) -> ReviewDto {
    let parties = fam::parties_of(&r);
    let cannot_decide = if r.status != "open" {
        Some("Already decided.".to_string())
    } else {
        fam::bar_to_deciding(&n.mine, &parties, user_id, r.created_by).map(str::to_string)
    };
    ReviewDto {
        id: r.id,
        subject_type: r.subject_type,
        subject_id: r.subject_id,
        entity_name: r.entity_id.and_then(|i| n.llcs.get(&i).cloned()),
        entity_id: r.entity_id,
        counterparty_name: r.counterparty_id.and_then(|i| n.vendors.get(&i).cloned()),
        counterparty_id: r.counterparty_id,
        summary: r.summary,
        reason: r.reason,
        amount_cents: r.amount_cents,
        market_cents: r.market_cents,
        market_note: r.market_note,
        parties: parties
            .into_iter()
            .map(|id| PartyDto {
                id,
                name: n.owners.get(&id).cloned().unwrap_or_else(|| "Owner".into()),
            })
            .collect(),
        status: r.status,
        decided_by: r.decided_by,
        decided_at: r.decided_at.map(|d| d.to_rfc3339()),
        decision_note: r.decision_note,
        created_at: r.created_at.to_rfc3339(),
        cannot_decide,
    }
}

async fn find_review(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    id: &str,
) -> ApiResult<entity::related_party_review::Model> {
    RelatedPartyReview::find_by_id(uuid(id, "review")?)
        .filter(entity::related_party_review::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("review not found".into()))
}

/// `GET /related-party?status` — reviews, open first.
#[rocket_okapi::openapi(tag = "Related parties")]
#[get("/related-party?<status>")]
pub async fn list_reviews(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    status: Option<&str>,
) -> ApiResult<Json<Vec<ReviewDto>>> {
    user.require(Permission::PayableRead)?;
    let mut q = RelatedPartyReview::find()
        .filter(entity::related_party_review::Column::TenantId.eq(scope.tenant_id));
    if let Some(s) = status.filter(|s| !s.is_empty() && *s != "all") {
        q = q.filter(entity::related_party_review::Column::Status.eq(s));
    }
    let rows = q
        .order_by_desc(entity::related_party_review::Column::CreatedAt)
        .all(&db)
        .await?;
    let n = names(&db, scope.tenant_id, user.user_id).await?;
    let mut out: Vec<ReviewDto> = rows
        .into_iter()
        .map(|r| review_dto(r, &n, user.user_id))
        .collect();
    out.sort_by_key(|r| r.status != "open");
    Ok(Json(out))
}

#[derive(Deserialize, JsonSchema)]
pub struct FlagReq {
    /// `lease` | `deal` | `other` (bills are flagged on their own).
    pub subject_type: String,
    pub subject_id: Option<Uuid>,
    pub entity_id: Option<Uuid>,
    pub counterparty_id: Option<Uuid>,
    pub summary: String,
    pub reason: Option<String>,
    pub amount_cents: Option<i64>,
    /// Owners who are parties, beyond those the counterparty brings.
    #[serde(default)]
    pub party_owner_ids: Vec<Uuid>,
}

/// `POST /related-party` — flag something by hand (a lease to a relative, a
/// sale between the family's entities).
#[rocket_okapi::openapi(tag = "Related parties")]
#[post("/related-party", data = "<body>")]
pub async fn flag(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<FlagReq>,
) -> ApiResult<Json<ReviewDto>> {
    user.require(Permission::PayableManage)?;
    let b = body.into_inner();
    if !["lease", "deal", "other"].contains(&b.subject_type.as_str()) {
        return Err(ApiError::BadRequest(
            "subject_type must be lease, deal or other".into(),
        ));
    }
    let summary = b.summary.trim().to_string();
    if summary.is_empty() {
        return Err(ApiError::BadRequest("say what it is".into()));
    }
    let n = names(&db, scope.tenant_id, user.user_id).await?;
    if let Some(e) = b.entity_id {
        if !n.llcs.contains_key(&e) {
            return Err(ApiError::BadRequest("unknown entity".into()));
        }
    }
    let mut parties: Vec<Uuid> = vec![];
    if let Some(cid) = b.counterparty_id {
        let c = Counterparty::find_by_id(cid)
            .filter(entity::counterparty::Column::TenantId.eq(scope.tenant_id))
            .one(&db)
            .await?
            .ok_or_else(|| ApiError::BadRequest("unknown counterparty".into()))?;
        parties = fam::parties_for(&db, scope.tenant_id, &c).await?;
    }
    for o in b.party_owner_ids {
        if !n.owners.contains_key(&o) {
            return Err(ApiError::BadRequest("unknown owner".into()));
        }
        if !parties.contains(&o) {
            parties.push(o);
        }
    }
    if let Some(sid) = b.subject_id {
        let dup = RelatedPartyReview::find()
            .filter(entity::related_party_review::Column::TenantId.eq(scope.tenant_id))
            .filter(entity::related_party_review::Column::SubjectType.eq(b.subject_type.clone()))
            .filter(entity::related_party_review::Column::SubjectId.eq(sid))
            .one(&db)
            .await?;
        if dup.is_some() {
            return Err(ApiError::Conflict("that's already flagged".into()));
        }
    }
    let now = Utc::now();
    let r = entity::related_party_review::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        subject_type: Set(b.subject_type),
        subject_id: Set(b.subject_id),
        entity_id: Set(b.entity_id),
        counterparty_id: Set(b.counterparty_id),
        summary: Set(summary),
        reason: Set(text(b.reason).unwrap_or_else(|| "Flagged by staff.".into())),
        amount_cents: Set(b.amount_cents),
        market_cents: Set(None),
        market_note: Set(None),
        parties: Set(serde_json::json!(parties)),
        status: Set("open".into()),
        decided_by: Set(None),
        decided_at: Set(None),
        decision_note: Set(None),
        created_by: Set(Some(user.user_id)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::RELATED_PARTY_FLAG,
        Some("related_party_review"),
        Some(r.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "subject": r.subject_type, "auto": false })),
    )
    .await;
    Ok(Json(review_dto(r, &n, user.user_id)))
}

#[derive(Deserialize, JsonSchema)]
pub struct NoteReq {
    pub market_cents: Option<i64>,
    pub market_note: Option<String>,
}

/// `PATCH /related-party/<id>` — the market-rate note (comparable quotes,
/// rent comps, an appraisal).
#[rocket_okapi::openapi(tag = "Related parties")]
#[patch("/related-party/<id>", data = "<body>")]
pub async fn note(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<NoteReq>,
) -> ApiResult<Json<ReviewDto>> {
    user.require(Permission::PayableManage)?;
    let r = find_review(&db, scope.tenant_id, id).await?;
    if r.status != "open" {
        return Err(ApiError::BadRequest("it's already decided".into()));
    }
    let b = body.into_inner();
    if b.market_cents.is_some_and(|c| c < 0) {
        return Err(ApiError::BadRequest(
            "market amount can't be negative".into(),
        ));
    }
    let before =
        serde_json::json!({ "market_cents": r.market_cents, "market_note": r.market_note });
    let mut am: entity::related_party_review::ActiveModel = r.into();
    am.market_cents = Set(b.market_cents);
    am.market_note = Set(text(b.market_note));
    am.updated_at = Set(Utc::now().into());
    let r = am.update(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::RELATED_PARTY_NOTE,
        Some("related_party_review"),
        Some(r.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "before": before, "after": { "market_cents": r.market_cents, "market_note": r.market_note } })),
    )
    .await;
    let n = names(&db, scope.tenant_id, user.user_id).await?;
    Ok(Json(review_dto(r, &n, user.user_id)))
}

#[derive(Deserialize, JsonSchema)]
pub struct DecideReq {
    pub approve: bool,
    pub note: Option<String>,
}

/// `POST /related-party/<id>/decide` — approve or reject, by someone who
/// isn't a party and didn't raise it. Approving needs the market-rate note.
#[rocket_okapi::openapi(tag = "Related parties")]
#[post("/related-party/<id>/decide", data = "<body>")]
pub async fn decide(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<DecideReq>,
) -> ApiResult<Json<ReviewDto>> {
    user.require(Permission::PayableApprove)?;
    let r = find_review(&db, scope.tenant_id, id).await?;
    if r.status != "open" {
        return Err(ApiError::BadRequest("it's already decided".into()));
    }
    let n = names(&db, scope.tenant_id, user.user_id).await?;
    if let Some(why) =
        fam::bar_to_deciding(&n.mine, &fam::parties_of(&r), user.user_id, r.created_by)
    {
        return Err(ApiError::Forbidden(why.into()));
    }
    let b = body.into_inner();
    if b.approve && r.market_note.is_none() {
        return Err(ApiError::BadRequest(
            "Add a market-rate note before approving.".into(),
        ));
    }
    let now = Utc::now();
    let mut am: entity::related_party_review::ActiveModel = r.into();
    am.status = Set(if b.approve { "approved" } else { "rejected" }.into());
    am.decided_by = Set(Some(user.user_id));
    am.decided_at = Set(Some(now.into()));
    am.decision_note = Set(text(b.note));
    am.updated_at = Set(now.into());
    let r = am.update(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::RELATED_PARTY_DECIDE,
        Some("related_party_review"),
        Some(r.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "status": r.status, "note": r.decision_note })),
    )
    .await;
    Ok(Json(review_dto(r, &n, user.user_id)))
}

#[derive(Deserialize, JsonSchema)]
pub struct RelatedReq {
    pub related_llc_id: Option<Uuid>,
    pub related_owner_id: Option<Uuid>,
}

/// `PUT /entities/<id>/related` — mark a counterparty as one of the family's
/// own entities or a family member (both empty clears it).
#[rocket_okapi::openapi(tag = "Related parties")]
#[put("/entities/<id>/related", data = "<body>")]
pub async fn set_related(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<RelatedReq>,
) -> ApiResult<Json<crate::routes::entities::dto::CounterpartyDto>> {
    user.require(Permission::EntityManage)?;
    let c = Counterparty::find_by_id(uuid(id, "counterparty")?)
        .filter(entity::counterparty::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("counterparty not found".into()))?;
    let b = body.into_inner();
    if let Some(l) = b.related_llc_id {
        Llc::find_by_id(l)
            .filter(entity::llc::Column::TenantId.eq(scope.tenant_id))
            .one(&db)
            .await?
            .ok_or_else(|| ApiError::BadRequest("unknown entity".into()))?;
    }
    if let Some(o) = b.related_owner_id {
        Owner::find_by_id(o)
            .filter(entity::owner::Column::TenantId.eq(scope.tenant_id))
            .one(&db)
            .await?
            .ok_or_else(|| ApiError::BadRequest("unknown owner".into()))?;
    }
    let before = serde_json::json!({ "related_llc_id": c.related_llc_id, "related_owner_id": c.related_owner_id });
    let mut am: entity::counterparty::ActiveModel = c.into();
    am.related_llc_id = Set(b.related_llc_id);
    am.related_owner_id = Set(b.related_owner_id);
    am.updated_at = Set(Utc::now().into());
    let c = am.update(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::COUNTERPARTY_RELATED,
        Some("counterparty"),
        Some(c.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "before": before, "after": { "related_llc_id": c.related_llc_id, "related_owner_id": c.related_owner_id } })),
    )
    .await;
    Ok(Json(c.into()))
}

// ---- foundation mode --------------------------------------------------------

#[derive(Deserialize, JsonSchema)]
pub struct FoundationReq {
    pub foundation: bool,
    /// `percent` | `at_cost`.
    pub fee_basis: String,
}

/// `PUT /llcs/<id>/foundation` — foundation mode and how its management fee
/// is worked out.
#[rocket_okapi::openapi(tag = "Foundation")]
#[put("/llcs/<id>/foundation", data = "<body>")]
pub async fn set_foundation(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<FoundationReq>,
) -> ApiResult<Json<crate::routes::llcs::dto::LlcResp>> {
    user.require(Permission::TenantManage)?;
    let b = body.into_inner();
    if !["percent", "at_cost"].contains(&b.fee_basis.as_str()) {
        return Err(ApiError::BadRequest(
            "fee_basis must be percent or at_cost".into(),
        ));
    }
    let l = Llc::find_by_id(uuid(id, "entity")?)
        .filter(entity::llc::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("entity not found".into()))?;
    let before = serde_json::json!({ "foundation": l.foundation, "fee_basis": l.fee_basis });
    let mut am: entity::llc::ActiveModel = l.into();
    am.foundation = Set(b.foundation);
    am.fee_basis = Set(b.fee_basis);
    let l = am.update(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::LLC_FOUNDATION,
        Some("llc"),
        Some(l.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "before": before, "after": { "foundation": l.foundation, "fee_basis": l.fee_basis } })),
    )
    .await;
    Ok(Json(l.into()))
}

#[derive(Serialize, JsonSchema)]
pub struct VoucherDto {
    pub authority: String,
    pub contract_number: Option<String>,
    pub hap_cents: i64,
    pub starts_on: String,
    pub ends_on: Option<String>,
}

impl From<entity::housing_voucher::Model> for VoucherDto {
    fn from(v: entity::housing_voucher::Model) -> Self {
        VoucherDto {
            authority: v.authority,
            contract_number: v.contract_number,
            hap_cents: v.hap_cents,
            starts_on: v.starts_on.to_string(),
            ends_on: v.ends_on.map(|d| d.to_string()),
        }
    }
}

#[derive(Serialize, JsonSchema)]
pub struct CertDto {
    pub id: Uuid,
    pub effective_on: String,
    pub expires_on: String,
    pub household_size: i32,
    pub annual_income_cents: i64,
    pub ami_cents: i64,
    pub limit_pct: i32,
    /// The income limit: `limit_pct` of AMI.
    pub limit_cents: i64,
    pub qualified: bool,
    pub notes: Option<String>,
    pub created_at: String,
}

impl From<entity::income_certification::Model> for CertDto {
    fn from(c: entity::income_certification::Model) -> Self {
        CertDto {
            id: c.id,
            effective_on: c.effective_on.to_string(),
            expires_on: c.expires_on.to_string(),
            household_size: c.household_size,
            annual_income_cents: c.annual_income_cents,
            ami_cents: c.ami_cents,
            limit_pct: c.limit_pct,
            limit_cents: c.ami_cents * c.limit_pct as i64 / 100,
            qualified: c.qualified,
            notes: c.notes,
            created_at: c.created_at.to_rfc3339(),
        }
    }
}

#[derive(Serialize, JsonSchema)]
pub struct HapDue {
    pub payment_id: Uuid,
    pub due_date: String,
    pub amount_cents: i64,
}

#[derive(Serialize, JsonSchema)]
pub struct AssistanceDto {
    /// The lease's property is held by an LLC in foundation mode.
    pub foundation: bool,
    pub rent_cents: i64,
    pub voucher: Option<VoucherDto>,
    /// What the resident pays each month after the voucher.
    pub resident_share_cents: i64,
    pub certifications: Vec<CertDto>,
    /// `missing` | `ok` | `expiring` | `expired` | `over_limit`.
    pub certification: String,
    /// HAP payments not yet received.
    pub hap_due: Vec<HapDue>,
}

async fn lease_in_reach(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    access: &Access,
    id: &str,
) -> ApiResult<entity::lease::Model> {
    let l = Lease::find_by_id(uuid(id, "lease")?)
        .filter(entity::lease::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("lease not found".into()))?;
    if !access.sees(l.property_id) {
        return Err(ApiError::NotFound("lease not found".into()));
    }
    Ok(l)
}

async fn is_foundation(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    property_id: Uuid,
) -> ApiResult<bool> {
    let Some(llc) = Property::find_by_id(property_id)
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .and_then(|p| p.llc_id)
    else {
        return Ok(false);
    };
    Ok(Llc::find_by_id(llc)
        .one(db)
        .await?
        .is_some_and(|l| l.foundation))
}

async fn assistance_for(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    l: &entity::lease::Model,
) -> ApiResult<AssistanceDto> {
    let today = Utc::now().date_naive();
    let voucher = HousingVoucher::find()
        .filter(entity::housing_voucher::Column::TenantId.eq(tenant_id))
        .filter(entity::housing_voucher::Column::LeaseId.eq(l.id))
        .one(db)
        .await?;
    let certs = IncomeCertification::find()
        .filter(entity::income_certification::Column::TenantId.eq(tenant_id))
        .filter(entity::income_certification::Column::LeaseId.eq(l.id))
        .order_by_desc(entity::income_certification::Column::EffectiveOn)
        .order_by_desc(entity::income_certification::Column::CreatedAt)
        .all(db)
        .await?;
    let hap_now = voucher
        .as_ref()
        .filter(|v| fam::voucher_covers(v, today))
        .map(|v| v.hap_cents)
        .unwrap_or(0);
    let hap_due = LeasePayment::find()
        .filter(entity::lease_payment::Column::TenantId.eq(tenant_id))
        .filter(entity::lease_payment::Column::LeaseId.eq(l.id))
        .filter(entity::lease_payment::Column::Kind.eq(crate::payments::KIND_HAP))
        .filter(entity::lease_payment::Column::Status.is_in(["due", "late"]))
        .order_by_asc(entity::lease_payment::Column::DueDate)
        .all(db)
        .await?
        .into_iter()
        .map(|p| HapDue {
            payment_id: p.id,
            due_date: p.due_date,
            amount_cents: p.amount_cents,
        })
        .collect();
    Ok(AssistanceDto {
        foundation: is_foundation(db, tenant_id, l.property_id).await?,
        rent_cents: l.rent_cents,
        resident_share_cents: fam::split_hap(l.rent_cents, hap_now).0,
        voucher: voucher.map(Into::into),
        certification: fam::cert_state(certs.first(), today).to_string(),
        certifications: certs.into_iter().map(Into::into).collect(),
        hap_due,
    })
}

/// `GET /leases/<id>/assistance` — voucher, income certifications and HAP due.
#[rocket_okapi::openapi(tag = "Foundation")]
#[get("/leases/<id>/assistance")]
pub async fn assistance(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    id: &str,
) -> ApiResult<Json<AssistanceDto>> {
    user.require(Permission::LeaseRead)?;
    let l = lease_in_reach(&db, scope.tenant_id, &access, id).await?;
    Ok(Json(assistance_for(&db, scope.tenant_id, &l).await?))
}

#[derive(Deserialize, JsonSchema)]
pub struct VoucherReq {
    pub authority: String,
    pub contract_number: Option<String>,
    pub hap_cents: i64,
    pub starts_on: String,
    pub ends_on: Option<String>,
}

/// `PUT /leases/<id>/voucher` — set the lease's housing voucher. Rent raised
/// from the next cycle splits into the resident's share and the HAP.
#[rocket_okapi::openapi(tag = "Foundation")]
#[put("/leases/<id>/voucher", data = "<body>")]
pub async fn set_voucher(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    id: &str,
    body: Json<VoucherReq>,
) -> ApiResult<Json<AssistanceDto>> {
    user.require(Permission::LeaseManage)?;
    let l = lease_in_reach(&db, scope.tenant_id, &access, id).await?;
    let b = body.into_inner();
    let authority = b.authority.trim().to_string();
    if authority.is_empty() {
        return Err(ApiError::BadRequest("name the housing authority".into()));
    }
    if b.hap_cents < 0 || b.hap_cents > l.rent_cents {
        return Err(ApiError::BadRequest(
            "the voucher can't pay more than the rent".into(),
        ));
    }
    let starts = date(&b.starts_on, "starts_on")?;
    let ends = match text(b.ends_on) {
        Some(s) => Some(date(&s, "ends_on")?),
        None => None,
    };
    if ends.is_some_and(|e| e < starts) {
        return Err(ApiError::BadRequest(
            "the voucher ends before it starts".into(),
        ));
    }
    let now = Utc::now();
    let existing = HousingVoucher::find()
        .filter(entity::housing_voucher::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::housing_voucher::Column::LeaseId.eq(l.id))
        .one(&db)
        .await?;
    let before = existing.as_ref().map(|v| serde_json::json!({ "authority": v.authority, "hap_cents": v.hap_cents, "starts_on": v.starts_on, "ends_on": v.ends_on }));
    match existing {
        Some(v) => {
            let mut am: entity::housing_voucher::ActiveModel = v.into();
            am.authority = Set(authority);
            am.contract_number = Set(text(b.contract_number));
            am.hap_cents = Set(b.hap_cents);
            am.starts_on = Set(starts);
            am.ends_on = Set(ends);
            am.updated_at = Set(now.into());
            am.update(&db).await?;
        }
        None => {
            entity::housing_voucher::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(scope.tenant_id),
                lease_id: Set(l.id),
                authority: Set(authority),
                contract_number: Set(text(b.contract_number)),
                hap_cents: Set(b.hap_cents),
                starts_on: Set(starts),
                ends_on: Set(ends),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }
            .insert(&db)
            .await?;
        }
    }
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::VOUCHER_SET,
        Some("lease"),
        Some(l.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "before": before, "after": { "hap_cents": b.hap_cents, "starts_on": starts, "ends_on": ends } })),
    )
    .await;
    Ok(Json(assistance_for(&db, scope.tenant_id, &l).await?))
}

/// `DELETE /leases/<id>/voucher` — the voucher ended; the resident owes all of
/// the rent from the next cycle.
#[rocket_okapi::openapi(tag = "Foundation")]
#[delete("/leases/<id>/voucher")]
pub async fn delete_voucher(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    id: &str,
) -> ApiResult<Json<AssistanceDto>> {
    user.require(Permission::LeaseManage)?;
    let l = lease_in_reach(&db, scope.tenant_id, &access, id).await?;
    let removed = HousingVoucher::delete_many()
        .filter(entity::housing_voucher::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::housing_voucher::Column::LeaseId.eq(l.id))
        .exec(&db)
        .await?
        .rows_affected;
    if removed > 0 {
        crate::audit::record(
            &db,
            Some(user.user_id),
            crate::audit::actions::VOUCHER_SET,
            Some("lease"),
            Some(l.id.to_string()),
            Some(scope.tenant_id),
            Some(serde_json::json!({ "removed": true })),
        )
        .await;
    }
    Ok(Json(assistance_for(&db, scope.tenant_id, &l).await?))
}

#[derive(Deserialize, JsonSchema)]
pub struct CertReq {
    pub effective_on: String,
    /// Defaults to a year after `effective_on`.
    pub expires_on: Option<String>,
    pub household_size: i32,
    pub annual_income_cents: i64,
    /// Area median income for this household size.
    pub ami_cents: i64,
    pub limit_pct: i32,
    pub notes: Option<String>,
}

/// `POST /leases/<id>/income-certifications` — certify the household's income.
#[rocket_okapi::openapi(tag = "Foundation")]
#[post("/leases/<id>/income-certifications", data = "<body>")]
pub async fn certify(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    id: &str,
    body: Json<CertReq>,
) -> ApiResult<Json<AssistanceDto>> {
    user.require(Permission::LeaseManage)?;
    let l = lease_in_reach(&db, scope.tenant_id, &access, id).await?;
    let b = body.into_inner();
    if !(1..=12).contains(&b.household_size) {
        return Err(ApiError::BadRequest("household size is 1 to 12".into()));
    }
    if b.annual_income_cents < 0 || b.ami_cents <= 0 {
        return Err(ApiError::BadRequest("income and AMI must be set".into()));
    }
    if !(1..=150).contains(&b.limit_pct) {
        return Err(ApiError::BadRequest(
            "the limit is a percent of AMI, 1 to 150".into(),
        ));
    }
    let effective = date(&b.effective_on, "effective_on")?;
    let expires = match text(b.expires_on) {
        Some(s) => date(&s, "expires_on")?,
        None => effective
            .checked_add_months(Months::new(12))
            .unwrap_or(effective),
    };
    if expires <= effective {
        return Err(ApiError::BadRequest(
            "it must expire after it takes effect".into(),
        ));
    }
    let qualified = fam::qualifies(b.annual_income_cents, b.ami_cents, b.limit_pct);
    let c = entity::income_certification::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        lease_id: Set(l.id),
        effective_on: Set(effective),
        expires_on: Set(expires),
        household_size: Set(b.household_size),
        annual_income_cents: Set(b.annual_income_cents),
        ami_cents: Set(b.ami_cents),
        limit_pct: Set(b.limit_pct),
        qualified: Set(qualified),
        notes: Set(text(b.notes)),
        certified_by: Set(Some(user.user_id)),
        created_at: Set(Utc::now().into()),
    }
    .insert(&db)
    .await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::INCOME_CERTIFY,
        Some("lease"),
        Some(l.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "certification_id": c.id, "household_size": c.household_size, "limit_pct": c.limit_pct, "qualified": qualified, "expires_on": expires })),
    )
    .await;
    Ok(Json(assistance_for(&db, scope.tenant_id, &l).await?))
}

#[derive(Serialize, JsonSchema)]
pub struct ComplianceRow {
    pub lease_id: Uuid,
    pub tenant_name: String,
    pub property_id: Uuid,
    pub property_name: String,
    pub entity_name: String,
    /// `missing` | `ok` | `expiring` | `expired` | `over_limit`.
    pub certification: String,
    pub expires_on: Option<String>,
    pub hap_cents: Option<i64>,
    pub authority: Option<String>,
    pub hap_owed_cents: i64,
}

/// `GET /foundation/compliance` — every active lease in a foundation entity:
/// where its income certification stands and what the voucher owes.
#[rocket_okapi::openapi(tag = "Foundation")]
#[get("/foundation/compliance")]
pub async fn compliance(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
) -> ApiResult<Json<Vec<ComplianceRow>>> {
    user.require(Permission::LeaseRead)?;
    let llcs: HashMap<Uuid, String> = Llc::find()
        .filter(entity::llc::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::llc::Column::Foundation.eq(true))
        .all(&db)
        .await?
        .into_iter()
        .map(|l| (l.id, l.name))
        .collect();
    if llcs.is_empty() {
        return Ok(Json(vec![]));
    }
    let props: HashMap<Uuid, (String, Uuid)> = Property::find()
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::property::Column::LlcId.is_in(llcs.keys().copied().collect::<Vec<_>>()))
        .all(&db)
        .await?
        .into_iter()
        .filter(|p| access.sees(p.id))
        .filter_map(|p| p.llc_id.map(|l| (p.id, (p.name, l))))
        .collect();
    if props.is_empty() {
        return Ok(Json(vec![]));
    }
    let leases = Lease::find()
        .filter(entity::lease::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::lease::Column::Status.eq("active"))
        .filter(entity::lease::Column::PropertyId.is_in(props.keys().copied().collect::<Vec<_>>()))
        .all(&db)
        .await?;
    let ids: Vec<Uuid> = leases.iter().map(|l| l.id).collect();
    let mut latest: HashMap<Uuid, entity::income_certification::Model> = HashMap::new();
    for c in IncomeCertification::find()
        .filter(entity::income_certification::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::income_certification::Column::LeaseId.is_in(ids.clone()))
        .order_by_asc(entity::income_certification::Column::EffectiveOn)
        .order_by_asc(entity::income_certification::Column::CreatedAt)
        .all(&db)
        .await?
    {
        latest.insert(c.lease_id, c);
    }
    let vouchers: HashMap<Uuid, entity::housing_voucher::Model> = HousingVoucher::find()
        .filter(entity::housing_voucher::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::housing_voucher::Column::LeaseId.is_in(ids.clone()))
        .all(&db)
        .await?
        .into_iter()
        .map(|v| (v.lease_id, v))
        .collect();
    let mut owed: HashMap<Uuid, i64> = HashMap::new();
    for p in LeasePayment::find()
        .filter(entity::lease_payment::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::lease_payment::Column::LeaseId.is_in(ids))
        .filter(entity::lease_payment::Column::Kind.eq(crate::payments::KIND_HAP))
        .filter(entity::lease_payment::Column::Status.is_in(["due", "late"]))
        .all(&db)
        .await?
    {
        *owed.entry(p.lease_id).or_default() += p.amount_cents;
    }
    let today = Utc::now().date_naive();
    let mut rows: Vec<ComplianceRow> = leases
        .into_iter()
        .map(|l| {
            let (pname, llc) = props.get(&l.property_id).cloned().unwrap_or_default();
            let c = latest.get(&l.id);
            let v = vouchers.get(&l.id);
            ComplianceRow {
                lease_id: l.id,
                tenant_name: l.tenant_name,
                property_id: l.property_id,
                property_name: pname,
                entity_name: llcs.get(&llc).cloned().unwrap_or_default(),
                certification: fam::cert_state(c, today).to_string(),
                expires_on: c.map(|c| c.expires_on.to_string()),
                hap_cents: v.map(|v| v.hap_cents),
                authority: v.map(|v| v.authority.clone()),
                hap_owed_cents: owed.get(&l.id).copied().unwrap_or(0),
            }
        })
        .collect();
    let rank = |s: &str| match s {
        "expired" => 0,
        "missing" => 1,
        "over_limit" => 2,
        "expiring" => 3,
        _ => 4,
    };
    rows.sort_by(|a, b| {
        rank(&a.certification)
            .cmp(&rank(&b.certification))
            .then_with(|| a.tenant_name.cmp(&b.tenant_name))
    });
    Ok(Json(rows))
}

#[derive(Deserialize, JsonSchema)]
pub struct HapReceivedReq {
    pub paid_date: Option<String>,
}

/// `POST /lease-payments/<id>/hap-received` — the housing authority paid its
/// part for a month.
#[rocket_okapi::openapi(tag = "Foundation")]
#[post("/lease-payments/<id>/hap-received", data = "<body>")]
pub async fn hap_received(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    id: &str,
    body: Json<HapReceivedReq>,
) -> ApiResult<Json<AssistanceDto>> {
    user.require(Permission::LeaseManage)?;
    let p = LeasePayment::find_by_id(uuid(id, "payment")?)
        .filter(entity::lease_payment::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("payment not found".into()))?;
    if p.kind != crate::payments::KIND_HAP {
        return Err(ApiError::BadRequest(
            "that isn't a housing assistance payment".into(),
        ));
    }
    if !["due", "late"].contains(&p.status.as_str()) {
        return Err(ApiError::BadRequest("it's already settled".into()));
    }
    let l = lease_in_reach(&db, scope.tenant_id, &access, &p.lease_id.to_string()).await?;
    let paid = match text(body.into_inner().paid_date) {
        Some(s) => date(&s, "paid_date")?,
        None => Utc::now().date_naive(),
    }
    .to_string();
    let amount = p.amount_cents;
    let pid = p.id;
    let mut txn_id = None;
    if let Some(entity_id) =
        crate::payments::entity_for_property(&db, scope.tenant_id, l.property_id).await
    {
        let t = crate::accounting::post_payment_settled(
            &db,
            scope.tenant_id,
            entity_id,
            Some(l.property_id),
            l.id,
            &paid,
            amount,
            crate::payments::KIND_HAP,
            pid,
        )
        .await?;
        txn_id = Some(t.id);
    }
    let mut am: entity::lease_payment::ActiveModel = p.into();
    am.status = Set("paid".into());
    am.paid_date = Set(Some(paid.clone()));
    am.method = Set(Some("hap".into()));
    am.ledger_txn_id = Set(txn_id);
    am.update(&db).await?;
    let new_balance = (l.balance_cents - amount).max(0);
    let mut lam: entity::lease::ActiveModel = l.clone().into();
    lam.balance_cents = Set(new_balance);
    if new_balance == 0 {
        lam.payment_status = Set("current".into());
    }
    lam.updated_at = Set(Utc::now().into());
    let l = lam.update(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::HAP_RECEIVED,
        Some("lease_payment"),
        Some(pid.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "lease_id": l.id, "amount_cents": amount, "paid_date": paid })),
    )
    .await;
    Ok(Json(assistance_for(&db, scope.tenant_id, &l).await?))
}
