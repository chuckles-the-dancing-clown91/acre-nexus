//! The **owner's side**: answering approvals from a link, the owner portal
//! (`/my/owner/*`: holdings, open work, approvals, statements), the staff view
//! of a work order's approvals, and inviting an owner to sign in.

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::owner_approvals::{self as oa, money, Statement};
use crate::rbac::Permission;
use crate::routes::maintenance::desk::{cost_summary, FileDto};
use crate::storage::{ObjectStore, SIGNED_URL_TTL_SECS};
use crate::tenancy::TenantScope;
use chrono::{Datelike, Utc};
use entity::prelude::{Document, MaintenanceTicket, Owner, OwnerApproval, Property, Tenant, User};
use rocket::serde::json::Json;
use rocket::{get, patch, post};
use schemars::JsonSchema;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, JsonSchema)]
pub struct ApprovalDto {
    pub id: Uuid,
    pub ticket_id: Uuid,
    pub ticket_title: String,
    pub property_id: Uuid,
    pub property: String,
    pub owner_id: Uuid,
    pub owner_name: String,
    /// `approval` | `signoff`.
    pub kind: String,
    pub amount_cents: i64,
    pub amount_label: String,
    /// `pending` | `approved` | `declined` | `disputed` | `overridden`.
    pub status: String,
    pub note: Option<String>,
    pub requested_at: String,
    pub decided_at: Option<String>,
    pub decided_by: Option<String>,
    pub decision_note: Option<String>,
    pub override_reason: Option<String>,
    pub nudges: i32,
}

async fn dto(
    db: &impl ConnectionTrait,
    a: entity::owner_approval::Model,
) -> ApiResult<ApprovalDto> {
    let t = MaintenanceTicket::find_by_id(a.ticket_id).one(db).await?;
    let (title, property_id) = t
        .as_ref()
        .map(|t| (t.title.clone(), t.property_id))
        .unwrap_or_default();
    let property = Property::find_by_id(property_id)
        .one(db)
        .await?
        .map(|p| format!("{}, {}", p.address, p.city))
        .unwrap_or_default();
    let owner_name = Owner::find_by_id(a.owner_id)
        .one(db)
        .await?
        .map(|o| o.name)
        .unwrap_or_default();
    Ok(ApprovalDto {
        id: a.id,
        ticket_id: a.ticket_id,
        ticket_title: title,
        property_id,
        property,
        owner_id: a.owner_id,
        owner_name,
        kind: a.kind,
        amount_label: money(a.amount_cents),
        amount_cents: a.amount_cents,
        status: a.status,
        note: a.note,
        requested_at: a.requested_at.to_rfc3339(),
        decided_at: a.decided_at.map(|d| d.to_rfc3339()),
        decided_by: a.decided_by,
        decision_note: a.decision_note,
        override_reason: a.override_reason,
        nudges: a.nudges,
    })
}

async fn dtos(
    db: &impl ConnectionTrait,
    rows: Vec<entity::owner_approval::Model>,
) -> ApiResult<Vec<ApprovalDto>> {
    let mut out = Vec::with_capacity(rows.len());
    for a in rows {
        out.push(dto(db, a).await?);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// The link
// ---------------------------------------------------------------------------

#[derive(Serialize, JsonSchema)]
pub struct PublicApproval {
    pub id: Uuid,
    pub company: String,
    pub owner_name: String,
    pub kind: String,
    pub status: String,
    pub amount_label: String,
    pub limit_label: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub property: String,
    pub note: Option<String>,
    pub requested_at: String,
    pub decided_at: Option<String>,
    pub decision_note: Option<String>,
    /// What's planned, or what was done.
    pub tasks: Vec<String>,
    pub photos: Vec<FileDto>,
    /// For a sign-off: the lines on the work order the resident could see.
    pub updates: Vec<String>,
}

async fn public_dto(
    db: &impl ConnectionTrait,
    a: entity::owner_approval::Model,
) -> ApiResult<PublicApproval> {
    let company = Tenant::find_by_id(a.tenant_id)
        .one(db)
        .await?
        .map(|t| t.name)
        .unwrap_or_default();
    let owner = Owner::find_by_id(a.owner_id).one(db).await?;
    let t = MaintenanceTicket::find_by_id(a.ticket_id)
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("that link isn't valid".into()))?;
    let property = Property::find_by_id(t.property_id)
        .one(db)
        .await?
        .map(|p| format!("{}, {}", p.address, p.city))
        .unwrap_or_default();
    let tasks = entity::prelude::TicketTask::find()
        .filter(entity::ticket_task::Column::TicketId.eq(t.id))
        .filter(entity::ticket_task::Column::Status.ne("skipped"))
        .order_by_asc(entity::ticket_task::Column::Position)
        .all(db)
        .await?
        .into_iter()
        .map(|x| {
            if x.status == "done" {
                format!("{} (done)", x.title)
            } else {
                x.title
            }
        })
        .collect();
    let store = ObjectStore::from_env().ok();
    let photos = Document::find()
        .filter(entity::document::Column::OwnerType.eq("maintenance_ticket"))
        .filter(entity::document::Column::OwnerId.eq(t.id))
        .filter(entity::document::Column::Category.is_in(["photo", "video"]))
        .order_by_desc(entity::document::Column::CreatedAt)
        .all(db)
        .await?
        .into_iter()
        .filter(|d| !matches!(store, Some(ObjectStore::Local(_))) || d.status == "stored")
        .take(12)
        .map(|d| FileDto {
            url: store
                .as_ref()
                .and_then(|s| s.signed_get_url(&d.storage_key, SIGNED_URL_TTL_SECS).ok())
                .map(|s| s.url),
            kind: d.category.clone().unwrap_or_else(|| "photo".into()),
            id: d.id,
            filename: d.filename,
            mime_type: d.mime_type,
            size_bytes: d.size_bytes,
            created_at: d.created_at.to_rfc3339(),
        })
        .collect();
    let updates = if a.kind == "signoff" {
        entity::prelude::TicketComment::find()
            .filter(entity::ticket_comment::Column::TicketId.eq(t.id))
            .filter(entity::ticket_comment::Column::Visibility.eq("public"))
            .order_by_asc(entity::ticket_comment::Column::CreatedAt)
            .all(db)
            .await?
            .into_iter()
            .filter(|c| c.kind == "action")
            .map(|c| c.body)
            .collect()
    } else {
        vec![]
    };
    let limit_label = match &owner {
        Some(o) if a.kind == "approval" => Some(money(oa::limit_for(db, a.tenant_id, o).await)),
        _ => None,
    };
    Ok(PublicApproval {
        id: a.id,
        company,
        owner_name: owner.map(|o| o.name).unwrap_or_default(),
        kind: a.kind,
        status: a.status,
        amount_label: money(a.amount_cents),
        limit_label,
        title: t.title,
        description: t.description,
        property,
        note: a.note,
        requested_at: a.requested_at.to_rfc3339(),
        decided_at: a.decided_at.map(|d| d.to_rfc3339()),
        decision_note: a.decision_note,
        tasks,
        photos,
        updates,
    })
}

/// `GET /public/approve/<token>` — what the link opens.
#[rocket_okapi::openapi(tag = "Owners (Public)")]
#[get("/public/approve/<token>")]
pub async fn public_view(db: crate::db::RequestDb, token: &str) -> ApiResult<Json<PublicApproval>> {
    let a = oa::by_token(&db, token).await?;
    Ok(Json(public_dto(&db, a).await?))
}

#[derive(Deserialize, JsonSchema)]
pub struct DecideReq {
    pub approve: bool,
    pub note: Option<String>,
}

/// `POST /public/approve/<token>` — approve or decline (dispute, for a sign-off).
#[rocket_okapi::openapi(tag = "Owners (Public)")]
#[post("/public/approve/<token>", data = "<body>")]
pub async fn public_decide(
    db: crate::db::RequestDb,
    token: &str,
    body: Json<DecideReq>,
) -> ApiResult<Json<PublicApproval>> {
    let a = oa::by_token(&db, token).await?;
    let b = body.into_inner();
    let saved = oa::decide(&db, a.tenant_id, a, b.approve, b.note, "owner").await?;
    Ok(Json(public_dto(&db, saved).await?))
}

// ---------------------------------------------------------------------------
// The owner portal
// ---------------------------------------------------------------------------

async fn me(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    user: &AuthUser,
) -> ApiResult<entity::owner::Model> {
    let account = User::find_by_id(user.user_id)
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("user not found".into()))?;
    oa::owner_for_user(db, tenant_id, user.user_id, &account.email)
        .await?
        .ok_or_else(|| ApiError::NotFound("you're not set up as an owner here".into()))
}

#[derive(Serialize, JsonSchema)]
pub struct OwnerHome {
    pub owner_id: Uuid,
    pub name: String,
    pub company: String,
    pub approval_limit_cents: i64,
    pub approval_limit_label: String,
    pub entities: Vec<String>,
    pub properties: Vec<OwnerProperty>,
    pub pending: Vec<ApprovalDto>,
    pub open_work: Vec<OwnerWork>,
    /// `YYYY-MM` of the latest full month.
    pub last_month: String,
    pub month_to_date: MoneyBrief,
}

#[derive(Serialize, JsonSchema)]
pub struct MoneyBrief {
    pub month: String,
    pub rent_collected_label: String,
    pub expenses_label: String,
    pub net_label: String,
}

#[derive(Serialize, JsonSchema)]
pub struct OwnerProperty {
    pub id: Uuid,
    pub name: String,
    pub address: String,
    pub units: i32,
    pub occupied_units: i32,
    pub monthly_rent_cents: i64,
    pub monthly_rent_label: String,
    pub open_work: usize,
    pub image_url: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct OwnerWork {
    pub id: Uuid,
    pub title: String,
    pub property_id: Uuid,
    pub property: String,
    pub status: String,
    pub waiting_on: Option<String>,
    pub priority: String,
    pub est_label: String,
    pub actual_label: String,
    pub created_at: String,
    pub resolved_at: Option<String>,
    pub approval: Option<ApprovalDto>,
}

async fn work_for(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    props: &[entity::property::Model],
    open_only: bool,
) -> ApiResult<Vec<OwnerWork>> {
    let ids: Vec<Uuid> = props.iter().map(|p| p.id).collect();
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let mut q = MaintenanceTicket::find()
        .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
        .filter(entity::maintenance_ticket::Column::PropertyId.is_in(ids));
    if open_only {
        q = q.filter(
            entity::maintenance_ticket::Column::Status
                .is_in(crate::routes::maintenance::OPEN_STATUSES.to_vec()),
        );
    }
    let tickets = q
        .order_by_desc(entity::maintenance_ticket::Column::CreatedAt)
        .all(db)
        .await?;
    let mut out = vec![];
    for t in tickets.into_iter().take(60) {
        let c = cost_summary(db, tenant_id, &t).await?;
        let approval = match OwnerApproval::find()
            .filter(entity::owner_approval::Column::TicketId.eq(t.id))
            .order_by_desc(entity::owner_approval::Column::RequestedAt)
            .one(db)
            .await?
        {
            Some(a) => Some(dto(db, a).await?),
            None => None,
        };
        out.push(OwnerWork {
            id: t.id,
            title: t.title.clone(),
            property_id: t.property_id,
            property: props
                .iter()
                .find(|p| p.id == t.property_id)
                .map(|p| p.name.clone())
                .unwrap_or_default(),
            status: t.status.clone(),
            waiting_on: t.waiting_on.clone(),
            priority: t.priority.clone(),
            est_label: c.est_total_label,
            actual_label: c.actual_total_label,
            created_at: t.created_at.to_rfc3339(),
            resolved_at: t.resolved_at.map(|d| d.to_rfc3339()),
            approval,
        });
    }
    Ok(out)
}

/// `GET /my/owner` — the owner's home: holdings, what's waiting on them,
/// open work, and the month so far.
#[rocket_okapi::openapi(tag = "Owner Portal")]
#[get("/my/owner")]
pub async fn home(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<OwnerHome>> {
    let o = me(&db, scope.tenant_id, &user).await?;
    let (llcs, props) = oa::holdings(&db, scope.tenant_id, o.id).await?;
    let open = work_for(&db, scope.tenant_id, &props, true).await?;
    let pending = dtos(
        &db,
        OwnerApproval::find()
            .filter(entity::owner_approval::Column::TenantId.eq(scope.tenant_id))
            .filter(entity::owner_approval::Column::OwnerId.eq(o.id))
            .filter(entity::owner_approval::Column::Status.eq("pending"))
            .order_by_asc(entity::owner_approval::Column::RequestedAt)
            .all(&db)
            .await?,
    )
    .await?;
    let today = Utc::now().date_naive();
    let this_month = today.format("%Y-%m").to_string();
    let first = chrono::NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap();
    let last_month = first.pred_opt().unwrap().format("%Y-%m").to_string();
    let mtd = oa::statement(&db, scope.tenant_id, &o, &this_month).await?;
    let company = Tenant::find_by_id(scope.tenant_id)
        .one(&db)
        .await?
        .map(|t| t.name)
        .unwrap_or_default();
    let limit = oa::limit_for(&db, scope.tenant_id, &o).await;
    Ok(Json(OwnerHome {
        owner_id: o.id,
        name: o.name.clone(),
        company,
        approval_limit_cents: limit,
        approval_limit_label: money(limit),
        entities: llcs.iter().map(|l| l.name.clone()).collect(),
        properties: props
            .iter()
            .map(|p| OwnerProperty {
                id: p.id,
                name: p.name.clone(),
                address: format!("{}, {}", p.address, p.city),
                units: p.units,
                occupied_units: p.occupied_units,
                monthly_rent_cents: p.monthly_rent_cents,
                monthly_rent_label: money(p.monthly_rent_cents),
                open_work: open.iter().filter(|w| w.property_id == p.id).count(),
                image_url: p.image_url.clone(),
            })
            .collect(),
        pending,
        open_work: open,
        last_month,
        month_to_date: MoneyBrief {
            month: this_month,
            rent_collected_label: mtd.rent_collected_label,
            expenses_label: mtd.expenses_label,
            net_label: mtd.net_label,
        },
    }))
}

/// `GET /my/owner/work?<all>` — work on the owner's properties, open by
/// default, everything with `all=true`.
#[rocket_okapi::openapi(tag = "Owner Portal")]
#[get("/my/owner/work?<all>")]
pub async fn work(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    all: Option<bool>,
) -> ApiResult<Json<Vec<OwnerWork>>> {
    let o = me(&db, scope.tenant_id, &user).await?;
    let (_, props) = oa::holdings(&db, scope.tenant_id, o.id).await?;
    Ok(Json(
        work_for(&db, scope.tenant_id, &props, !all.unwrap_or(false)).await?,
    ))
}

/// `GET /my/owner/approvals` — everything the owner was asked, newest first.
#[rocket_okapi::openapi(tag = "Owner Portal")]
#[get("/my/owner/approvals")]
pub async fn approvals(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<ApprovalDto>>> {
    let o = me(&db, scope.tenant_id, &user).await?;
    Ok(Json(
        dtos(
            &db,
            OwnerApproval::find()
                .filter(entity::owner_approval::Column::TenantId.eq(scope.tenant_id))
                .filter(entity::owner_approval::Column::OwnerId.eq(o.id))
                .order_by_desc(entity::owner_approval::Column::RequestedAt)
                .all(&db)
                .await?,
        )
        .await?,
    ))
}

/// `POST /my/owner/approvals/<id>` — answer from the portal.
#[rocket_okapi::openapi(tag = "Owner Portal")]
#[post("/my/owner/approvals/<id>", data = "<body>")]
pub async fn decide(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<DecideReq>,
) -> ApiResult<Json<ApprovalDto>> {
    let o = me(&db, scope.tenant_id, &user).await?;
    let aid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let a = OwnerApproval::find_by_id(aid)
        .filter(entity::owner_approval::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::owner_approval::Column::OwnerId.eq(o.id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("approval not found".into()))?;
    let b = body.into_inner();
    let saved = oa::decide(&db, scope.tenant_id, a, b.approve, b.note, "owner").await?;
    Ok(Json(dto(&db, saved).await?))
}

/// `GET /my/owner/statement?<month>` — a month's statement (default: last month).
#[rocket_okapi::openapi(tag = "Owner Portal")]
#[get("/my/owner/statement?<month>")]
pub async fn statement(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    month: Option<String>,
) -> ApiResult<Json<Statement>> {
    let o = me(&db, scope.tenant_id, &user).await?;
    let month = month.unwrap_or_else(default_month);
    Ok(Json(oa::statement(&db, scope.tenant_id, &o, &month).await?))
}

fn default_month() -> String {
    let today = Utc::now().date_naive();
    let first = chrono::NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap();
    first.pred_opt().unwrap().format("%Y-%m").to_string()
}

/// `GET /my/owner/statement.pdf?<month>`.
#[rocket_okapi::openapi(skip)]
#[get("/my/owner/statement.pdf?<month>")]
pub async fn statement_pdf(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    month: Option<String>,
) -> ApiResult<crate::routes::reports::ReportFile> {
    let o = me(&db, scope.tenant_id, &user).await?;
    let month = month.unwrap_or_else(default_month);
    let s = oa::statement(&db, scope.tenant_id, &o, &month).await?;
    let company = Tenant::find_by_id(scope.tenant_id)
        .one(&db)
        .await?
        .map(|t| t.name)
        .unwrap_or_default();
    Ok(crate::routes::reports::ReportFile::new(
        oa::statement_pdf(&s, &company),
        "application/pdf",
        format!("owner-statement-{month}.pdf"),
    ))
}

// ---------------------------------------------------------------------------
// Staff
// ---------------------------------------------------------------------------

/// `GET /tickets/<id>/approvals` — the owner's asks on a work order, with
/// where the owner stands (who they are, their limit, the estimate).
#[derive(Serialize, JsonSchema)]
pub struct TicketApprovals {
    pub owner_id: Option<Uuid>,
    pub owner_name: Option<String>,
    pub limit_cents: i64,
    pub limit_label: String,
    pub est_total_cents: i64,
    pub est_total_label: String,
    /// The estimate is at or over the limit and nothing approved covers it.
    pub needs_approval: bool,
    pub approvals: Vec<ApprovalDto>,
}

#[rocket_okapi::openapi(tag = "Service Desk")]
#[get("/tickets/<id>/approvals")]
pub async fn ticket_approvals(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<TicketApprovals>> {
    user.require(Permission::MaintenanceRead)?;
    let tid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let t = MaintenanceTicket::find_by_id(tid)
        .filter(entity::maintenance_ticket::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("work order not found".into()))?;
    let owner = oa::owner_for_property(&db, scope.tenant_id, t.property_id).await?;
    let limit = match &owner {
        Some(o) => oa::limit_for(&db, scope.tenant_id, o).await,
        None => 0,
    };
    let est = cost_summary(&db, scope.tenant_id, &t)
        .await?
        .est_total_cents;
    let rows = OwnerApproval::find()
        .filter(entity::owner_approval::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::owner_approval::Column::TicketId.eq(t.id))
        .order_by_desc(entity::owner_approval::Column::RequestedAt)
        .all(&db)
        .await?;
    let covered = rows
        .iter()
        .any(|a| a.kind == "approval" && matches!(a.status.as_str(), "approved" | "overridden"));
    Ok(Json(TicketApprovals {
        owner_id: owner.as_ref().map(|o| o.id),
        owner_name: owner.as_ref().map(|o| o.name.clone()),
        limit_cents: limit,
        limit_label: money(limit),
        est_total_cents: est,
        est_total_label: money(est),
        needs_approval: owner.is_some() && limit > 0 && est >= limit && !covered,
        approvals: dtos(&db, rows).await?,
    }))
}

#[derive(Deserialize, JsonSchema)]
pub struct RequestReq {
    /// What to ask about; defaults to the estimate.
    pub amount_cents: Option<i64>,
    pub note: Option<String>,
}

/// `POST /tickets/<id>/approvals` — ask the owner now, whatever the limit.
#[rocket_okapi::openapi(tag = "Service Desk")]
#[post("/tickets/<id>/approvals", data = "<body>")]
pub async fn request_approval(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<RequestReq>,
) -> ApiResult<Json<ApprovalDto>> {
    user.require(Permission::MaintenanceManage)?;
    let tid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let t = MaintenanceTicket::find_by_id(tid)
        .filter(entity::maintenance_ticket::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("work order not found".into()))?;
    let owner = oa::owner_for_property(&db, scope.tenant_id, t.property_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("this property has no owner on file".into()))?;
    if owner.email.as_deref().unwrap_or("").trim().is_empty()
        && owner.phone.as_deref().unwrap_or("").trim().is_empty()
    {
        return Err(ApiError::BadRequest(format!(
            "{} has no email or phone to send to",
            owner.name
        )));
    }
    if let Some(a) = oa::latest(&db, scope.tenant_id, t.id, "approval").await? {
        if a.status == "pending" {
            return Err(ApiError::Conflict(format!(
                "{} was already asked, on {}",
                owner.name,
                a.requested_at.format("%b %-d")
            )));
        }
    }
    let b = body.into_inner();
    let amount = match b.amount_cents {
        Some(a) if a > 0 => a,
        _ => {
            cost_summary(&db, scope.tenant_id, &t)
                .await?
                .est_total_cents
        }
    };
    let saved = oa::request(
        &db,
        scope.tenant_id,
        &t,
        &owner,
        amount,
        b.note
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty()),
        Some(user.user_id),
    )
    .await?;
    Ok(Json(dto(&db, saved).await?))
}

/// `POST /approvals/<id>/decide` — staff record the owner's answer given by
/// phone, or cancel an ask by declining it.
#[rocket_okapi::openapi(tag = "Service Desk")]
#[post("/approvals/<id>/decide", data = "<body>")]
pub async fn staff_decide(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<DecideReq>,
) -> ApiResult<Json<ApprovalDto>> {
    user.require(Permission::MaintenanceManage)?;
    let aid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let a = OwnerApproval::find_by_id(aid)
        .filter(entity::owner_approval::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("approval not found".into()))?;
    let b = body.into_inner();
    let saved = oa::decide(&db, scope.tenant_id, a, b.approve, b.note, "staff").await?;
    Ok(Json(dto(&db, saved).await?))
}

#[derive(Deserialize, JsonSchema)]
pub struct OwnerSettingsReq {
    /// Their own approval limit in cents; null goes back to the workspace's.
    pub approval_limit_cents: Option<i64>,
    pub clear_limit: Option<bool>,
}

/// `PATCH /crm/owners/<id>/approvals` — an owner's own spend limit.
#[rocket_okapi::openapi(tag = "CRM")]
#[patch("/crm/owners/<id>/approvals", data = "<body>")]
pub async fn owner_limit(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<OwnerSettingsReq>,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::EntityManage)?;
    let oid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let o = Owner::find_by_id(oid)
        .filter(entity::owner::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("owner not found".into()))?;
    let b = body.into_inner();
    let mut am: entity::owner::ActiveModel = o.into();
    if b.clear_limit.unwrap_or(false) {
        am.approval_limit_cents = Set(None);
    } else if let Some(l) = b.approval_limit_cents {
        if l < 0 {
            return Err(ApiError::BadRequest("the limit can't be negative".into()));
        }
        am.approval_limit_cents = Set(Some(l));
    }
    let saved = am.update(&db).await?;
    Ok(Json(serde_json::json!({
        "id": saved.id,
        "approval_limit_cents": saved.approval_limit_cents,
        "user_id": saved.user_id,
    })))
}

#[derive(Serialize, JsonSchema)]
pub struct InviteResp {
    pub owner_id: Uuid,
    pub user_id: Uuid,
    /// `invited` (a link to set a password went out) or `linked` (an account
    /// already existed and now opens the owner portal).
    pub outcome: String,
}

/// `POST /crm/owners/<id>/invite` — give an owner a login to the owner
/// portal. They get the landlord persona, assigned to their LLCs, and a link
/// to set a password.
#[rocket_okapi::openapi(tag = "CRM")]
#[post("/crm/owners/<id>/invite")]
pub async fn invite_owner(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<InviteResp>> {
    user.require(Permission::EntityManage)?;
    user.require(Permission::MemberManage)?;
    let oid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let o = Owner::find_by_id(oid)
        .filter(entity::owner::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("owner not found".into()))?;
    let email = o
        .email
        .as_deref()
        .map(|e| e.trim().to_lowercase())
        .filter(|e| e.contains('@'))
        .ok_or_else(|| ApiError::BadRequest("the owner needs an email first".into()))?;
    let existing = User::find()
        .filter(entity::user::Column::Email.eq(email.clone()))
        .one(&db)
        .await?;
    let still_invited = existing.as_ref().is_some_and(|u| u.status == "invited");
    let (uid, created) = match existing {
        Some(u) => (u.id, false),
        None => {
            let uid = Uuid::new_v4();
            let pw = crate::auth::hash_password(&crate::auth::random_secret(24))
                .map_err(ApiError::Internal)?;
            entity::user::ActiveModel {
                id: Set(uid),
                tenant_id: Set(Some(scope.tenant_id)),
                email: Set(email),
                username: Set(None),
                password_hash: Set(pw),
                name: Set(o.name.clone()),
                is_platform_staff: Set(false),
                status: Set("invited".into()),
                last_login_at: Set(None),
                created_at: Set(Utc::now().into()),
            }
            .insert(&db)
            .await?;
            (uid, true)
        }
    };
    // A landlord membership (reach limited to their LLCs) if they have none here.
    let has = entity::prelude::Membership::find()
        .filter(entity::membership::Column::UserId.eq(uid))
        .filter(entity::membership::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?;
    if has.is_none() {
        crate::routes::iam::helpers::add_membership_inner(
            &db,
            uid,
            &crate::routes::iam::dto::NewMembership {
                scope: crate::rbac::SCOPE_TENANT.to_string(),
                tenant_id: Some(scope.tenant_id),
                profile_type: "landlord".into(),
                title: Some("Owner".into()),
            },
            false,
        )
        .await?;
        let (llcs, _) = oa::holdings(&db, scope.tenant_id, o.id).await?;
        for l in llcs {
            let dup = entity::prelude::Assignment::find()
                .filter(entity::assignment::Column::TenantId.eq(scope.tenant_id))
                .filter(entity::assignment::Column::UserId.eq(uid))
                .filter(entity::assignment::Column::SubjectType.eq("entity"))
                .filter(entity::assignment::Column::SubjectId.eq(l.id))
                .one(&db)
                .await?;
            if dup.is_none() {
                entity::assignment::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    tenant_id: Set(scope.tenant_id),
                    user_id: Set(uid),
                    subject_type: Set("entity".into()),
                    subject_id: Set(l.id),
                    relationship: Set("landlord".into()),
                    role_id: Set(None),
                    is_primary: Set(false),
                    title: Set(Some("Owner".into())),
                    notes: Set(None),
                    assigned_by: Set(Some(user.user_id)),
                    created_at: Set(Utc::now().into()),
                    updated_at: Set(Utc::now().into()),
                }
                .insert(&db)
                .await?;
            }
        }
    }
    let mut am: entity::owner::ActiveModel = o.clone().into();
    am.user_id = Set(Some(uid));
    am.update(&db).await?;
    let outcome = if created || still_invited {
        if let Some(account) = User::find_by_id(uid).one(&db).await? {
            let (token, row) =
                crate::password_links::issue(&db, uid, crate::password_links::PURPOSE_INVITE)
                    .await?;
            crate::password_links::deliver(&db, Some(scope.tenant_id), &account, &token, &row)
                .await;
        }
        "invited"
    } else {
        "linked"
    };
    Ok(Json(InviteResp {
        owner_id: o.id,
        user_id: uid,
        outcome: outcome.into(),
    }))
}
