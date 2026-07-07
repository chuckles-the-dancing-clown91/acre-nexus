//! Staff-side messaging routes — read the resident threads with
//! `message:read`, reply/close/reopen with `message:manage`. A staff reply
//! notifies the resident (in-app + email through the notification substrate).

use super::dto::{
    thread_dto, MessageDto, SendMessageReq, ThreadDetailDto, ThreadDto, UpdateThreadReq,
};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{Lease, Message, MessageThread, Property, User};
use rocket::serde::json::Json;
use rocket::{get, patch, post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use std::collections::HashMap;
use uuid::Uuid;

/// A tenant-scoped thread, or 404.
async fn find_thread(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    id: &str,
) -> ApiResult<entity::message_thread::Model> {
    let tid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    MessageThread::find_by_id(tid)
        .filter(entity::message_thread::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("thread not found".into()))
}

/// Resident + property display context for one thread.
async fn thread_context(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    thread: &entity::message_thread::Model,
) -> (Option<entity::lease::Model>, Option<String>, Option<String>) {
    let lease = Lease::find_by_id(thread.lease_id)
        .filter(entity::lease::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .ok()
        .flatten();
    let property = Property::find_by_id(thread.property_id)
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .ok()
        .flatten();
    let resident = lease.as_ref().map(|l| l.tenant_name.clone());
    let address = property.map(|p| p.address);
    (lease, resident, address)
}

/// `GET /messages?status=` — the workspace's resident threads, most recent
/// activity first.
#[rocket_okapi::openapi(tag = "Messaging")]
#[get("/messages?<status>")]
pub async fn list_threads(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    status: Option<String>,
) -> ApiResult<Json<Vec<ThreadDto>>> {
    user.require(Permission::MessageRead)?;
    let mut q =
        MessageThread::find().filter(entity::message_thread::Column::TenantId.eq(scope.tenant_id));
    if let Some(s) = status.filter(|s| !s.trim().is_empty()) {
        q = q.filter(entity::message_thread::Column::Status.eq(s.trim().to_lowercase()));
    }
    let threads = q
        .order_by_desc(entity::message_thread::Column::LastMessageAt)
        .all(&db)
        .await?;

    // Display context in two lookups instead of per-thread queries.
    let lease_ids: Vec<Uuid> = threads.iter().map(|t| t.lease_id).collect();
    let property_ids: Vec<Uuid> = threads.iter().map(|t| t.property_id).collect();
    let leases: HashMap<Uuid, String> = Lease::find()
        .filter(entity::lease::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::lease::Column::Id.is_in(lease_ids))
        .all(&db)
        .await?
        .into_iter()
        .map(|l| (l.id, l.tenant_name))
        .collect();
    let properties: HashMap<Uuid, String> = Property::find()
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::property::Column::Id.is_in(property_ids))
        .all(&db)
        .await?
        .into_iter()
        .map(|p| (p.id, p.address))
        .collect();

    let mut out = Vec::with_capacity(threads.len());
    for t in threads {
        let messages = Message::find()
            .filter(entity::message::Column::TenantId.eq(scope.tenant_id))
            .filter(entity::message::Column::ThreadId.eq(t.id))
            .order_by_asc(entity::message::Column::CreatedAt)
            .all(&db)
            .await?;
        let resident = leases.get(&t.lease_id).cloned();
        let address = properties.get(&t.property_id).cloned();
        out.push(thread_dto(
            t,
            resident,
            address,
            messages.len() as i64,
            messages.last(),
        ));
    }
    Ok(Json(out))
}

/// `GET /messages/<id>` — one thread with its full timeline (oldest-first).
#[rocket_okapi::openapi(tag = "Messaging")]
#[get("/messages/<id>")]
pub async fn get_thread(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<ThreadDetailDto>> {
    user.require(Permission::MessageRead)?;
    let thread = find_thread(&db, scope.tenant_id, id).await?;
    let (_, resident, address) = thread_context(&db, scope.tenant_id, &thread).await;
    let messages = Message::find()
        .filter(entity::message::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::message::Column::ThreadId.eq(thread.id))
        .order_by_asc(entity::message::Column::CreatedAt)
        .all(&db)
        .await?;
    let dto = thread_dto(
        thread,
        resident,
        address,
        messages.len() as i64,
        messages.last(),
    );
    Ok(Json(ThreadDetailDto {
        thread: dto,
        messages: messages.into_iter().map(MessageDto::from).collect(),
    }))
}

/// `POST /messages/<id>/reply` — staff reply; the resident is notified in-app
/// and by email.
#[rocket_okapi::openapi(tag = "Messaging")]
#[post("/messages/<id>/reply", data = "<body>")]
pub async fn reply_thread(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<SendMessageReq>,
) -> ApiResult<Json<MessageDto>> {
    user.require(Permission::MessageManage)?;
    let thread = find_thread(&db, scope.tenant_id, id).await?;
    let text = super::clean_body(&body.into_inner().body).map_err(ApiError::BadRequest)?;

    let me = User::find_by_id(user.user_id)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("user not found".into()))?;
    let sender_name = if me.name.trim().is_empty() {
        me.email.clone()
    } else {
        me.name.clone()
    };

    let message = super::append_message(
        &db,
        scope.tenant_id,
        &thread,
        user.user_id,
        "staff",
        &sender_name,
        text,
    )
    .await?;

    // Notify the resident: in-app when they have an account, email always
    // (when the lease carries one).
    let (lease, _, _) = thread_context(&db, scope.tenant_id, &thread).await;
    if let Some(lease) = lease {
        if let Some(email) = lease
            .tenant_email
            .as_deref()
            .filter(|e| !e.trim().is_empty())
        {
            let vars = serde_json::json!({
                "subject": thread.subject,
                "preview": super::preview(&message.body),
            });
            if let Ok(Some(resident_user)) = User::find()
                .filter(entity::user::Column::Email.eq(email.to_lowercase()))
                .one(&db)
                .await
            {
                crate::notify::in_app(
                    &db,
                    scope.tenant_id,
                    &resident_user,
                    "manager_message",
                    &vars,
                    Some(("message_thread", thread.id)),
                    &format!("message:{}", message.id),
                )
                .await;
            }
            let payload = serde_json::json!({
                "template": "manager_message",
                "to": email,
                "owner_type": "message_thread",
                "owner_id": thread.id,
                "trigger": format!("message:{}", message.id),
                "vars": vars,
            });
            if let Err(e) =
                crate::scheduler::enqueue(&db, scope.tenant_id, "auto_email", payload, 0).await
            {
                tracing::error!("failed to enqueue message email: {e}");
            }
        }
    }

    Ok(Json(MessageDto::from(message)))
}

/// `PATCH /messages/<id>` — close or reopen a thread.
#[rocket_okapi::openapi(tag = "Messaging")]
#[patch("/messages/<id>", data = "<body>")]
pub async fn update_thread(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateThreadReq>,
) -> ApiResult<Json<ThreadDto>> {
    user.require(Permission::MessageManage)?;
    let thread = find_thread(&db, scope.tenant_id, id).await?;
    let status = body.into_inner().status.trim().to_lowercase();
    if !matches!(status.as_str(), "open" | "closed") {
        return Err(ApiError::BadRequest("status must be open|closed".into()));
    }

    let mut am: entity::message_thread::ActiveModel = thread.into();
    am.status = Set(status.clone());
    let saved = am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::MESSAGE_THREAD_UPDATE,
        Some("message_thread"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "status": status })),
    )
    .await;

    let (_, resident, address) = thread_context(&db, scope.tenant_id, &saved).await;
    let messages = Message::find()
        .filter(entity::message::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::message::Column::ThreadId.eq(saved.id))
        .order_by_asc(entity::message::Column::CreatedAt)
        .all(&db)
        .await?;
    Ok(Json(thread_dto(
        saved,
        resident,
        address,
        messages.len() as i64,
        messages.last(),
    )))
}
