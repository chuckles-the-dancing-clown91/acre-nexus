//! **The vendor portal** (fix plan F13): a vendor signs in and sees every
//! job sent to them, across work orders, and answers each the same way the
//! emailed link does: accept (and say when), decline, upload photos and the
//! invoice file, send the invoice, mark it done.
//!
//! A login is a vendor of a workspace through `vendor_portal_user`; staff give
//! one with `POST /entities/<id>/portal-invite`. A job is the batch of tasks
//! one dispatch sent, named by its stored link hash; every action checks the
//! batch is this vendor's. Alpha vendors keep the partner link.

use super::vendor_link::{self, BatchKey, VendorJob};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{
    Counterparty, MaintenanceTicket, Property, TicketTask, User, VendorPortalUser,
};
use rocket::serde::json::Json;
use rocket::{get, post};
use schemars::JsonSchema;
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use serde::Serialize;
use std::collections::BTreeMap;
use uuid::Uuid;

/// The vendor this login is, in this workspace.
async fn vendor_of(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    user_id: Uuid,
) -> ApiResult<entity::counterparty::Model> {
    let link = VendorPortalUser::find()
        .filter(entity::vendor_portal_user::Column::TenantId.eq(tenant_id))
        .filter(entity::vendor_portal_user::Column::UserId.eq(user_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::Forbidden("this login isn't a vendor here".into()))?;
    Counterparty::find_by_id(link.counterparty_id)
        .one(db)
        .await?
        .filter(|c| c.tenant_id == tenant_id)
        .ok_or_else(|| ApiError::Forbidden("this login isn't a vendor here".into()))
}

/// The batch, if it's this vendor's.
async fn mine<'a>(
    db: &impl ConnectionTrait,
    vendor_id: Uuid,
    batch: &'a str,
) -> ApiResult<BatchKey<'a>> {
    if batch.len() < 16 {
        return Err(ApiError::NotFound("job not found".into()));
    }
    let key = BatchKey::Hash(batch);
    match vendor_link::batch_vendor(db, &key).await {
        Ok(v) if v == vendor_id => Ok(key),
        _ => Err(ApiError::NotFound("job not found".into())),
    }
}

#[derive(Serialize, JsonSchema)]
pub struct VendorMe {
    pub vendor_id: Uuid,
    pub vendor: String,
    pub company: String,
    pub open_jobs: usize,
}

#[derive(Serialize, JsonSchema, Debug, Clone)]
pub struct JobRow {
    /// Pass back to the job routes.
    pub batch: String,
    pub ticket_id: Uuid,
    pub title: String,
    pub property: String,
    pub priority: String,
    pub due_date: Option<String>,
    pub tasks: usize,
    pub tasks_done: usize,
    /// `accepted` | `declined` | `done`, or null before they answer.
    pub response: Option<String>,
    pub sent_at: Option<String>,
    /// Still needs something from the vendor.
    pub open: bool,
}

#[derive(Serialize, JsonSchema)]
pub struct JobsResp {
    pub open: Vec<JobRow>,
    pub closed: Vec<JobRow>,
}

async fn jobs_for(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    vendor_id: Uuid,
) -> ApiResult<Vec<JobRow>> {
    // Declined batches no longer name the vendor as assignee, so they're out:
    // nothing more is wanted from them.
    let tasks = TicketTask::find()
        .filter(entity::ticket_task::Column::TenantId.eq(tenant_id))
        .filter(entity::ticket_task::Column::AssigneeEntityId.eq(vendor_id))
        .filter(entity::ticket_task::Column::VendorTokenHash.is_not_null())
        .all(db)
        .await?;
    let mut by: BTreeMap<String, Vec<entity::ticket_task::Model>> = BTreeMap::new();
    for t in tasks {
        if let Some(h) = t.vendor_token_hash.clone() {
            by.entry(h).or_default().push(t);
        }
    }
    let mut out = Vec::new();
    for (hash, ts) in by {
        let first = &ts[0];
        let Some(ticket) = MaintenanceTicket::find_by_id(first.ticket_id)
            .one(db)
            .await?
        else {
            continue;
        };
        let property = Property::find_by_id(ticket.property_id)
            .one(db)
            .await?
            .map(|p| format!("{}, {}", p.address, p.city))
            .unwrap_or_default();
        let done = ts
            .iter()
            .filter(|t| matches!(t.status.as_str(), "done" | "skipped"))
            .count();
        let response = ts.iter().find_map(|t| t.vendor_response.clone());
        let open = done < ts.len()
            && response.as_deref() != Some("done")
            && crate::routes::maintenance::is_open(&ticket.status);
        out.push(JobRow {
            batch: hash,
            ticket_id: ticket.id,
            title: ticket.title.clone(),
            property,
            priority: ticket.priority.clone(),
            due_date: ticket.due_date.clone(),
            tasks: ts.len(),
            tasks_done: done,
            response,
            sent_at: ts
                .iter()
                .filter_map(|t| t.dispatched_at)
                .max()
                .map(|d| d.to_rfc3339()),
            open,
        });
    }
    out.sort_by(|a, b| b.sent_at.cmp(&a.sent_at));
    Ok(out)
}

/// `GET /vendor-portal/me` — who this vendor is here.
#[rocket_okapi::openapi(tag = "Vendor Portal")]
#[get("/vendor-portal/me")]
pub async fn me(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<VendorMe>> {
    let v = vendor_of(&db, scope.tenant_id, user.user_id).await?;
    let jobs = jobs_for(&db, scope.tenant_id, v.id).await?;
    let company = entity::prelude::Tenant::find_by_id(scope.tenant_id)
        .one(&db)
        .await?
        .map(|t| t.name)
        .unwrap_or_default();
    Ok(Json(VendorMe {
        vendor_id: v.id,
        vendor: v.name,
        company,
        open_jobs: jobs.iter().filter(|j| j.open).count(),
    }))
}

/// `GET /vendor-portal/jobs` — every job sent to this vendor, open first.
#[rocket_okapi::openapi(tag = "Vendor Portal")]
#[get("/vendor-portal/jobs")]
pub async fn jobs(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<JobsResp>> {
    let v = vendor_of(&db, scope.tenant_id, user.user_id).await?;
    let (open, closed) = jobs_for(&db, scope.tenant_id, v.id)
        .await?
        .into_iter()
        .partition(|j| j.open);
    Ok(Json(JobsResp { open, closed }))
}

/// `GET /vendor-portal/jobs/<batch>` — one job, as the link shows it.
#[rocket_okapi::openapi(tag = "Vendor Portal")]
#[get("/vendor-portal/jobs/<batch>")]
pub async fn job(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    batch: &str,
) -> ApiResult<Json<VendorJob>> {
    let v = vendor_of(&db, scope.tenant_id, user.user_id).await?;
    let key = mine(&db, v.id, batch).await?;
    vendor_link::view_for(&db, &key).await
}

/// `POST /vendor-portal/jobs/<batch>/accept`
#[rocket_okapi::openapi(tag = "Vendor Portal")]
#[post("/vendor-portal/jobs/<batch>/accept", data = "<body>")]
pub async fn accept(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    batch: &str,
    body: Json<vendor_link::AcceptReq>,
) -> ApiResult<Json<VendorJob>> {
    let v = vendor_of(&db, scope.tenant_id, user.user_id).await?;
    let key = mine(&db, v.id, batch).await?;
    vendor_link::accept_for(&db, &key, body.into_inner()).await
}

/// `POST /vendor-portal/jobs/<batch>/decline`
#[rocket_okapi::openapi(tag = "Vendor Portal")]
#[post("/vendor-portal/jobs/<batch>/decline", data = "<body>")]
pub async fn decline(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    batch: &str,
    body: Json<vendor_link::DeclineReq>,
) -> ApiResult<Json<VendorJob>> {
    let v = vendor_of(&db, scope.tenant_id, user.user_id).await?;
    let key = mine(&db, v.id, batch).await?;
    vendor_link::decline_for(&db, &key, body.into_inner()).await
}

/// `POST /vendor-portal/jobs/<batch>/done`
#[rocket_okapi::openapi(tag = "Vendor Portal")]
#[post("/vendor-portal/jobs/<batch>/done", data = "<body>")]
pub async fn done(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    batch: &str,
    body: Json<vendor_link::DoneReq>,
) -> ApiResult<Json<VendorJob>> {
    let v = vendor_of(&db, scope.tenant_id, user.user_id).await?;
    let key = mine(&db, v.id, batch).await?;
    vendor_link::done_for(&db, &key, body.into_inner()).await
}

/// `POST /vendor-portal/jobs/<batch>/uploads`
#[rocket_okapi::openapi(tag = "Vendor Portal")]
#[post("/vendor-portal/jobs/<batch>/uploads", data = "<body>")]
pub async fn upload(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    batch: &str,
    body: Json<vendor_link::VendorUploadReq>,
) -> ApiResult<Json<vendor_link::VendorUploadResp>> {
    let v = vendor_of(&db, scope.tenant_id, user.user_id).await?;
    let key = mine(&db, v.id, batch).await?;
    vendor_link::upload_for(&db, &key, body.into_inner()).await
}

/// `POST /vendor-portal/jobs/<batch>/invoice`
#[rocket_okapi::openapi(tag = "Vendor Portal")]
#[post("/vendor-portal/jobs/<batch>/invoice", data = "<body>")]
pub async fn invoice(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    batch: &str,
    body: Json<vendor_link::InvoiceReq>,
) -> ApiResult<Json<VendorJob>> {
    let v = vendor_of(&db, scope.tenant_id, user.user_id).await?;
    let key = mine(&db, v.id, batch).await?;
    vendor_link::invoice_for(&db, &key, body.into_inner()).await
}

#[derive(Serialize, JsonSchema)]
pub struct PortalInviteResp {
    pub entity_id: Uuid,
    pub user_id: Uuid,
    /// `invited` (a link to set a password went out) or `linked`.
    pub outcome: String,
}

/// `POST /entities/<id>/portal-invite` — give a vendor a login to the vendor
/// portal: the vendor persona (no console access) and a link to set a
/// password; an existing account is linked instead.
#[rocket_okapi::openapi(tag = "Vendor Portal")]
#[post("/entities/<id>/portal-invite")]
pub async fn invite(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<PortalInviteResp>> {
    user.require(Permission::EntityManage)?;
    user.require(Permission::MemberManage)?;
    let cid = Uuid::parse_str(id).map_err(|_| ApiError::NotFound("vendor not found".into()))?;
    let v = Counterparty::find_by_id(cid)
        .one(&db)
        .await?
        .filter(|c| c.tenant_id == scope.tenant_id)
        .ok_or_else(|| ApiError::NotFound("vendor not found".into()))?;
    let email = v
        .email
        .as_deref()
        .map(|e| e.trim().to_lowercase())
        .filter(|e| e.contains('@'))
        .ok_or_else(|| ApiError::BadRequest("the vendor needs an email first".into()))?;
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
                name: Set(v.contact_name.clone().unwrap_or_else(|| v.name.clone())),
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
                profile_type: "vendor".into(),
                title: Some(v.name.clone()),
            },
            false,
        )
        .await?;
    }
    let linked = VendorPortalUser::find()
        .filter(entity::vendor_portal_user::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::vendor_portal_user::Column::UserId.eq(uid))
        .one(&db)
        .await?;
    match linked {
        Some(l) if l.counterparty_id != v.id => {
            return Err(ApiError::Conflict(
                "that login is already another vendor's".into(),
            ))
        }
        Some(_) => {}
        None => {
            entity::vendor_portal_user::ActiveModel {
                tenant_id: Set(scope.tenant_id),
                user_id: Set(uid),
                counterparty_id: Set(v.id),
                created_at: Set(Utc::now().into()),
            }
            .insert(&db)
            .await?;
        }
    }
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
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::VENDOR_PORTAL_INVITE,
        Some("counterparty"),
        Some(v.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "user_id": uid, "outcome": outcome })),
    )
    .await;
    Ok(Json(PortalInviteResp {
        entity_id: v.id,
        user_id: uid,
        outcome: outcome.into(),
    }))
}
