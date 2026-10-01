//! `/api/v1/tickets` — work orders for a vendor's own system. Scope
//! `maintenance:read` to see them, `maintenance:manage` to post progress.

use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::routes::maintenance::dto::{TicketCommentDto, TicketDto};
use crate::routes::team::parse_id;
use crate::tokens::ApiPrincipal;
use chrono::Utc;
use entity::prelude::{MaintenanceTicket, Property, TicketComment};
use rocket::serde::json::Json;
use rocket::{get, patch};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, schemars::JsonSchema)]
pub struct VendorTicket {
    #[serde(flatten)]
    pub ticket: TicketDto,
    pub property_name: String,
    pub property_address: String,
    pub comments: Vec<TicketCommentDto>,
}

async fn with_property(
    db: &crate::db::RequestDb,
    t: entity::maintenance_ticket::Model,
    comments: bool,
) -> ApiResult<VendorTicket> {
    let p = Property::find_by_id(t.property_id).one(db).await?;
    let comments = if comments {
        TicketComment::find()
            .filter(entity::ticket_comment::Column::TicketId.eq(t.id))
            .filter(entity::ticket_comment::Column::Visibility.eq("public"))
            .order_by_asc(entity::ticket_comment::Column::CreatedAt)
            .all(db)
            .await?
            .into_iter()
            .map(TicketCommentDto::from)
            .collect()
    } else {
        Vec::new()
    };
    Ok(VendorTicket {
        property_name: p.as_ref().map(|p| p.name.clone()).unwrap_or_default(),
        property_address: p.as_ref().map(crate::geo::full_address).unwrap_or_default(),
        ticket: TicketDto::from(t),
        comments,
    })
}

/// `GET /api/v1/tickets?status=` — open work orders (all statuses with `status=all`).
#[rocket_okapi::openapi(tag = "Vendor API")]
#[get("/api/v1/tickets?<status>")]
pub async fn tickets(
    db: crate::db::RequestDb,
    principal: ApiPrincipal,
    status: Option<String>,
) -> ApiResult<Json<Vec<VendorTicket>>> {
    principal.require(Permission::MaintenanceRead)?;
    let mut q = MaintenanceTicket::find()
        .filter(entity::maintenance_ticket::Column::TenantId.eq(principal.tenant_id));
    match status.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => {
            q = q.filter(
                entity::maintenance_ticket::Column::Status
                    .is_in(crate::routes::maintenance::OPEN_STATUSES.to_vec()),
            )
        }
        Some("all") => {}
        Some(s) => q = q.filter(entity::maintenance_ticket::Column::Status.eq(s)),
    }
    let rows = q
        .order_by_desc(entity::maintenance_ticket::Column::CreatedAt)
        .limit(500)
        .all(&db)
        .await?;
    let mut out = Vec::with_capacity(rows.len());
    for t in rows {
        out.push(with_property(&db, t, false).await?);
    }
    Ok(Json(out))
}

/// `GET /api/v1/tickets/<id>`.
#[rocket_okapi::openapi(tag = "Vendor API")]
#[get("/api/v1/tickets/<id>")]
pub async fn ticket(
    db: crate::db::RequestDb,
    principal: ApiPrincipal,
    id: &str,
) -> ApiResult<Json<VendorTicket>> {
    principal.require(Permission::MaintenanceRead)?;
    let t = MaintenanceTicket::find_by_id(parse_id(id, "ticket")?)
        .filter(entity::maintenance_ticket::Column::TenantId.eq(principal.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("ticket not found".into()))?;
    Ok(Json(with_property(&db, t, true).await?))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct VendorTicketPatch {
    /// `scheduled` | `in_progress` | `on_hold` | `resolved`
    pub status: Option<String>,
    /// A note for the timeline (residents see it).
    pub note: Option<String>,
}

/// `PATCH /api/v1/tickets/<id>` — a vendor posts progress. Scope `maintenance:manage`.
#[rocket_okapi::openapi(tag = "Vendor API")]
#[patch("/api/v1/tickets/<id>", data = "<body>")]
pub async fn update_ticket(
    db: crate::db::RequestDb,
    principal: ApiPrincipal,
    id: &str,
    body: Json<VendorTicketPatch>,
) -> ApiResult<Json<VendorTicket>> {
    principal.require(Permission::MaintenanceManage)?;
    let t = MaintenanceTicket::find_by_id(parse_id(id, "ticket")?)
        .filter(entity::maintenance_ticket::Column::TenantId.eq(principal.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("ticket not found".into()))?;
    let now = Utc::now();
    let mut am: entity::maintenance_ticket::ActiveModel = t.clone().into();
    let mut changed = None;
    if let Some(s) = body
        .status
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if !["scheduled", "in_progress", "on_hold", "resolved"].contains(&s) {
            return Err(ApiError::BadRequest(
                "status must be scheduled, in_progress, on_hold or resolved".into(),
            ));
        }
        if s != t.status {
            am.status = Set(s.into());
            if s == "resolved" {
                am.resolved_at = Set(Some(now.into()));
            }
            changed = Some(s.to_string());
        }
    }
    am.updated_at = Set(now.into());
    let saved = am.update(&db).await?;
    let note = body
        .note
        .clone()
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty());
    if changed.is_some() || note.is_some() {
        let body_text = match (&changed, &note) {
            (Some(s), Some(n)) => format!("Status -> {s}\n{n}"),
            (Some(s), None) => format!("Status -> {s}"),
            (None, Some(n)) => n.clone(),
            (None, None) => String::new(),
        };
        entity::ticket_comment::ActiveModel {
            id: Set(uuid::Uuid::new_v4()),
            tenant_id: Set(principal.tenant_id),
            ticket_id: Set(saved.id),
            author_user_id: Set(None),
            kind: Set(if changed.is_some() {
                "status"
            } else {
                "comment"
            }
            .into()),
            visibility: Set("public".into()),
            author_name: Set(Some("Vendor".into())),
            body: Set(body_text),
            created_at: Set(now.into()),
        }
        .insert(&db)
        .await?;
    }
    if let Some(s) = &changed {
        crate::webhooks_out::emit(
            &db,
            principal.tenant_id,
            if s == "resolved" {
                "maintenance_ticket.resolved"
            } else {
                "maintenance_ticket.updated"
            },
            serde_json::json!({ "ticket_id": saved.id, "status": s, "source": "vendor_api" }),
        )
        .await;
    }
    Ok(Json(with_property(&db, saved, true).await?))
}
