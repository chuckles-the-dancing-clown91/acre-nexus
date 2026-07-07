//! `/my/messages` — the **renter portal's** messaging surface. No staff
//! permission required: threads are scoped to the signed-in resident's own
//! lease (matched by account email, like `/my/lease`).

use super::dto::{
    thread_dto, CreateThreadReq, MessageDto, SendMessageReq, ThreadDetailDto, ThreadDto,
};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{Message, MessageThread, Property};
use rocket::serde::json::Json;
use rocket::{get, post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use uuid::Uuid;

/// The signed-in resident's lease, or 404.
async fn my_lease(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    user_id: Uuid,
) -> ApiResult<entity::lease::Model> {
    crate::payments::lease_for_user(db, tenant_id, user_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("no lease found for your account".into()))
}

/// One of the resident's own threads, or 404.
async fn my_thread(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    lease_id: Uuid,
    id: &str,
) -> ApiResult<entity::message_thread::Model> {
    let tid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    MessageThread::find_by_id(tid)
        .filter(entity::message_thread::Column::TenantId.eq(tenant_id))
        .filter(entity::message_thread::Column::LeaseId.eq(lease_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("conversation not found".into()))
}

/// Alert the messaging staff about a resident message.
async fn notify_manager(
    db: &crate::db::RequestDb,
    scope: &TenantScope,
    lease: &entity::lease::Model,
    thread: &entity::message_thread::Model,
    message: &entity::message::Model,
    actor: Uuid,
) {
    let property = Property::find_by_id(lease.property_id)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(db)
        .await
        .ok()
        .flatten();
    crate::notify::notify_staff(
        db,
        scope.tenant_id,
        "message:read",
        "resident_message",
        serde_json::json!({
            "resident": lease.tenant_name,
            "subject": thread.subject,
            "preview": super::preview(&message.body),
            "property": property.map(|p| p.address).unwrap_or_default(),
        }),
        Some(("message_thread", thread.id)),
        &format!("message:{}", message.id),
        Some(actor),
    )
    .await;
}

/// `GET /my/messages` — the resident's conversations, most recent first.
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[get("/my/messages")]
pub async fn my_threads(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<ThreadDto>>> {
    let lease = my_lease(&db, scope.tenant_id, user.user_id).await?;
    let threads = MessageThread::find()
        .filter(entity::message_thread::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::message_thread::Column::LeaseId.eq(lease.id))
        .order_by_desc(entity::message_thread::Column::LastMessageAt)
        .all(&db)
        .await?;

    let mut out = Vec::with_capacity(threads.len());
    for t in threads {
        let messages = Message::find()
            .filter(entity::message::Column::TenantId.eq(scope.tenant_id))
            .filter(entity::message::Column::ThreadId.eq(t.id))
            .order_by_asc(entity::message::Column::CreatedAt)
            .all(&db)
            .await?;
        out.push(thread_dto(
            t,
            None,
            None,
            messages.len() as i64,
            messages.last(),
        ));
    }
    Ok(Json(out))
}

/// `POST /my/messages` — start a conversation with the manager (subject +
/// first message).
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[post("/my/messages", data = "<body>")]
pub async fn create_my_thread(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<CreateThreadReq>,
) -> ApiResult<Json<ThreadDetailDto>> {
    let lease = my_lease(&db, scope.tenant_id, user.user_id).await?;
    let b = body.into_inner();
    let subject = super::clean_subject(&b.subject).map_err(ApiError::BadRequest)?;
    let text = super::clean_body(&b.body).map_err(ApiError::BadRequest)?;

    let now = Utc::now();
    let thread = entity::message_thread::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        lease_id: Set(lease.id),
        property_id: Set(lease.property_id),
        subject: Set(subject),
        status: Set("open".into()),
        created_by: Set(user.user_id),
        last_message_at: Set(now.into()),
        created_at: Set(now.into()),
    }
    .insert(&db)
    .await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::MESSAGE_THREAD_CREATE,
        Some("message_thread"),
        Some(thread.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "lease_id": lease.id, "subject": thread.subject })),
    )
    .await;

    let message = super::append_message(
        &db,
        scope.tenant_id,
        &thread,
        user.user_id,
        "resident",
        &lease.tenant_name,
        text,
    )
    .await?;
    notify_manager(&db, &scope, &lease, &thread, &message, user.user_id).await;

    let dto = thread_dto(thread, None, None, 1, Some(&message));
    Ok(Json(ThreadDetailDto {
        thread: dto,
        messages: vec![MessageDto::from(message)],
    }))
}

/// `GET /my/messages/<id>` — one conversation with its full timeline
/// (oldest-first).
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[get("/my/messages/<id>")]
pub async fn my_thread_detail(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<ThreadDetailDto>> {
    let lease = my_lease(&db, scope.tenant_id, user.user_id).await?;
    let thread = my_thread(&db, scope.tenant_id, lease.id, id).await?;
    let messages = Message::find()
        .filter(entity::message::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::message::Column::ThreadId.eq(thread.id))
        .order_by_asc(entity::message::Column::CreatedAt)
        .all(&db)
        .await?;
    let dto = thread_dto(thread, None, None, messages.len() as i64, messages.last());
    Ok(Json(ThreadDetailDto {
        thread: dto,
        messages: messages.into_iter().map(MessageDto::from).collect(),
    }))
}

/// `POST /my/messages/<id>` — reply in a conversation (reopens a closed one).
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[post("/my/messages/<id>", data = "<body>")]
pub async fn reply_my_thread(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<SendMessageReq>,
) -> ApiResult<Json<MessageDto>> {
    let lease = my_lease(&db, scope.tenant_id, user.user_id).await?;
    let thread = my_thread(&db, scope.tenant_id, lease.id, id).await?;
    let text = super::clean_body(&body.into_inner().body).map_err(ApiError::BadRequest)?;

    let message = super::append_message(
        &db,
        scope.tenant_id,
        &thread,
        user.user_id,
        "resident",
        &lease.tenant_name,
        text,
    )
    .await?;
    notify_manager(&db, &scope, &lease, &thread, &message, user.user_id).await;

    Ok(Json(MessageDto::from(message)))
}
