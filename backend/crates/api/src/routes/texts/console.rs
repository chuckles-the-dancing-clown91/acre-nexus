//! Console text inbox: list threads, read one (marks it read), reply, text a
//! new number, mark done / reopen, and — while texts are simulated — pretend a
//! resident texted in.

use super::{
    SendTextReq, SimulateInboundReq, StartTextReq, TextMessageDto, TextThreadDetailDto,
    TextThreadDto, TextsStatusDto, UpdateTextThreadReq,
};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use crate::texts::{self, Outbound, MAX_TEXT_CHARS};
use chrono::Utc;
use entity::prelude::{SmsMessage, SmsThread, Tenant, User};
use rocket::serde::json::Json;
use rocket::{get, patch, post, State};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use std::collections::HashMap;
use uuid::Uuid;

/// Most threads returned by one list call.
const MAX_THREADS: u64 = 200;

async fn find_thread(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    id: &str,
) -> ApiResult<entity::sms_thread::Model> {
    let tid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    SmsThread::find_by_id(tid)
        .filter(entity::sms_thread::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("conversation not found".into()))
}

fn clean_body(body: &str) -> ApiResult<String> {
    let body = body.trim();
    if body.is_empty() {
        return Err(ApiError::BadRequest("the text is empty".into()));
    }
    if body.chars().count() > MAX_TEXT_CHARS {
        return Err(ApiError::BadRequest(format!(
            "texts are limited to {MAX_TEXT_CHARS} characters"
        )));
    }
    Ok(body.to_string())
}

/// File a console-typed text as `queued` and hand it to the notification
/// queue; the job marks it sent / failed / blocked.
async fn send_text(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    user: &AuthUser,
    phone: &str,
    body: &str,
) -> ApiResult<entity::sms_message::Model> {
    if texts::is_opted_out(db, tenant_id, phone).await {
        return Err(ApiError::Conflict(
            "this number texted STOP — it can't be texted until they reply START".into(),
        ));
    }
    let message = texts::record_outbound(
        db,
        tenant_id,
        Outbound {
            to: phone,
            body,
            status: "queued",
            provider_message_id: None,
            template_key: None,
            sent_by_user_id: Some(user.user_id),
            error: None,
        },
    )
    .await?
    .ok_or_else(|| ApiError::BadRequest("that doesn't look like a phone number".into()))?;
    let phone = texts::normalize_phone(phone).unwrap_or_else(|| phone.to_string());
    crate::scheduler::enqueue(
        db,
        tenant_id,
        "auto_sms",
        serde_json::json!({
            "template": "direct_text",
            "to": phone,
            "vars": { "text": body },
            "owner_type": "sms_message",
            "owner_id": message.id.to_string(),
            "trigger": "console",
            "sms_message_id": message.id.to_string(),
        }),
        0,
    )
    .await?;
    crate::audit::record(
        db,
        Some(user.user_id),
        crate::audit::actions::SMS_SEND,
        Some("sms_thread"),
        Some(message.thread_id.to_string()),
        Some(tenant_id),
        Some(serde_json::json!({ "message_id": message.id })),
    )
    .await;
    Ok(message)
}

async fn detail(
    db: &crate::db::RequestDb,
    thread: entity::sms_thread::Model,
) -> ApiResult<TextThreadDetailDto> {
    let messages = SmsMessage::find()
        .filter(entity::sms_message::Column::TenantId.eq(thread.tenant_id))
        .filter(entity::sms_message::Column::ThreadId.eq(thread.id))
        .order_by_asc(entity::sms_message::Column::CreatedAt)
        .all(db)
        .await?;
    let senders: Vec<Uuid> = messages.iter().filter_map(|m| m.sent_by_user_id).collect();
    let names: HashMap<Uuid, String> = if senders.is_empty() {
        HashMap::new()
    } else {
        User::find()
            .filter(entity::user::Column::Id.is_in(senders))
            .all(db)
            .await?
            .into_iter()
            .map(|u| (u.id, u.name))
            .collect()
    };
    Ok(TextThreadDetailDto {
        thread: thread.into(),
        messages: messages
            .into_iter()
            .map(|m| TextMessageDto {
                id: m.id,
                direction: m.direction,
                body: m.body,
                status: m.status,
                template_key: m.template_key,
                sent_by: m.sent_by_user_id.and_then(|u| names.get(&u).cloned()),
                media_count: m.media_count,
                media: serde_json::from_value(m.media).unwrap_or_default(),
                error: m.error,
                created_at: m.created_at.to_rfc3339(),
            })
            .collect(),
    })
}

/// `GET /texts/status` — test mode or live, and the webhook URLs to give Twilio.
#[rocket_okapi::openapi(tag = "Texts")]
#[get("/texts/status")]
pub async fn status(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<TextsStatusDto>> {
    user.require(Permission::MessageRead)?;
    let slug = Tenant::find_by_id(scope.tenant_id)
        .one(&db)
        .await?
        .map(|t| t.slug)
        .unwrap_or_default();
    let base = crate::oauth::public_api_url();
    let base = base.trim_end_matches('/');
    let unread_threads = SmsThread::find()
        .filter(entity::sms_thread::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::sms_thread::Column::UnreadCount.gt(0))
        .count(&db)
        .await? as i64;
    Ok(Json(TextsStatusDto {
        live: crate::providers::is_live("sms"),
        provider_configured: crate::notify::default_provider(&db, scope.tenant_id, "sms")
            .await
            .is_some(),
        inbound_webhook_url: format!("{base}/webhooks/twilio/sms?tenant={slug}"),
        status_webhook_url: format!("{base}/webhooks/twilio/status?tenant={slug}"),
        voice_webhook_url: format!("{base}/webhooks/twilio/voice?tenant={slug}"),
        unread_threads,
    }))
}

/// `GET /texts?status=&mine=` — conversations, most recent first; `mine`
/// keeps the ones assigned to the caller.
#[rocket_okapi::openapi(tag = "Texts")]
#[get("/texts?<status>&<mine>")]
pub async fn list(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    status: Option<String>,
    mine: Option<bool>,
) -> ApiResult<Json<Vec<TextThreadDto>>> {
    user.require(Permission::MessageRead)?;
    let mut q = SmsThread::find().filter(entity::sms_thread::Column::TenantId.eq(scope.tenant_id));
    if let Some(s) = status.filter(|s| !s.trim().is_empty()) {
        q = q.filter(entity::sms_thread::Column::Status.eq(s.trim().to_lowercase()));
    }
    if mine == Some(true) {
        q = q.filter(entity::sms_thread::Column::AssignedUserId.eq(user.user_id));
    }
    let threads = q
        .order_by_desc(entity::sms_thread::Column::LastMessageAt)
        .limit(MAX_THREADS)
        .all(&db)
        .await?;
    Ok(Json(threads.into_iter().map(Into::into).collect()))
}

/// `GET /texts/<id>` — one conversation, oldest text first. Marks it read.
#[rocket_okapi::openapi(tag = "Texts")]
#[get("/texts/<id>", rank = 2)]
pub async fn get(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<TextThreadDetailDto>> {
    user.require(Permission::MessageRead)?;
    let mut thread = find_thread(&db, scope.tenant_id, id).await?;
    if thread.unread_count > 0 {
        let mut am: entity::sms_thread::ActiveModel = thread.into();
        am.unread_count = Set(0);
        thread = am.update(&db).await?;
    }
    Ok(Json(detail(&db, thread).await?))
}

/// `POST /texts/<id>/reply` — text the person back.
#[rocket_okapi::openapi(tag = "Texts")]
#[post("/texts/<id>/reply", data = "<body>")]
pub async fn reply(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<SendTextReq>,
) -> ApiResult<Json<TextThreadDetailDto>> {
    user.require(Permission::MessageManage)?;
    let thread = find_thread(&db, scope.tenant_id, id).await?;
    let text = clean_body(&body.body)?;
    send_text(&db, scope.tenant_id, &user, &thread.phone, &text).await?;
    let thread = find_thread(&db, scope.tenant_id, id).await?;
    Ok(Json(detail(&db, thread).await?))
}

/// `POST /texts` — text a number (starting a conversation if there isn't one).
#[rocket_okapi::openapi(tag = "Texts")]
#[post("/texts", data = "<body>")]
pub async fn start(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<StartTextReq>,
) -> ApiResult<Json<TextThreadDetailDto>> {
    user.require(Permission::MessageManage)?;
    let phone = texts::normalize_phone(&body.phone)
        .ok_or_else(|| ApiError::BadRequest("that doesn't look like a phone number".into()))?;
    let text = clean_body(&body.body)?;
    let message = send_text(&db, scope.tenant_id, &user, &phone, &text).await?;
    let thread = find_thread(&db, scope.tenant_id, &message.thread_id.to_string()).await?;
    Ok(Json(detail(&db, thread).await?))
}

/// `PATCH /texts/<id>` — mark a conversation done or reopen it, give it to a
/// staff member, or record marketing consent.
#[rocket_okapi::openapi(tag = "Texts")]
#[patch("/texts/<id>", data = "<body>")]
pub async fn update(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateTextThreadReq>,
) -> ApiResult<Json<TextThreadDto>> {
    user.require(Permission::MessageManage)?;
    let b = body.into_inner();
    let thread = find_thread(&db, scope.tenant_id, id).await?;
    let who = thread
        .display_name
        .clone()
        .unwrap_or_else(|| thread.phone.clone());
    let mut am: entity::sms_thread::ActiveModel = thread.clone().into();
    let mut detail = serde_json::Map::new();

    if let Some(status) = b.status.map(|s| s.trim().to_lowercase()) {
        if status != "open" && status != "done" {
            return Err(ApiError::BadRequest("status must be open or done".into()));
        }
        am.status = Set(status.clone());
        if status == "done" {
            am.unread_count = Set(0);
        }
        detail.insert("status".into(), serde_json::json!(status));
    }

    let mut new_owner = None;
    if let Some(raw) = b.assignee.map(|s| s.trim().to_string()) {
        let assignee = if raw.is_empty() {
            None
        } else {
            let uid = Uuid::parse_str(&raw)
                .map_err(|_| ApiError::BadRequest("assignee must be a user id".into()))?;
            let member = entity::prelude::Membership::find()
                .filter(entity::membership::Column::UserId.eq(uid))
                .filter(entity::membership::Column::TenantId.eq(scope.tenant_id))
                .one(&db)
                .await?;
            if member.is_none() {
                return Err(ApiError::BadRequest(
                    "that person isn't on this workspace's team".into(),
                ));
            }
            Some(uid)
        };
        if assignee != thread.assigned_user_id {
            new_owner = assignee;
        }
        am.assigned_user_id = Set(assignee);
        detail.insert("assignee".into(), serde_json::json!(assignee));
    }

    if let Some(consent) = b.marketing_consent {
        if consent && thread.opted_out_at.is_some() {
            return Err(ApiError::Conflict(
                "this number texted STOP; they need to text START first".into(),
            ));
        }
        let at = if consent {
            thread.marketing_opt_in_at.or(Some(Utc::now().into()))
        } else {
            None
        };
        am.marketing_opt_in_at = Set(at);
        detail.insert("marketing_consent".into(), serde_json::json!(consent));
    }

    if let Some(link) = b.link {
        let t = scope.tenant_id;
        let (lease, lead, vendor, name) = match (link.kind.as_str(), link.id) {
            ("none", _) => (None, None, None, None),
            ("resident", Some(id)) => {
                let l = entity::prelude::Lease::find_by_id(id)
                    .one(&db)
                    .await?
                    .filter(|l| l.tenant_id == t)
                    .ok_or_else(|| ApiError::NotFound("lease not found".into()))?;
                (Some(l.id), None, None, Some(l.tenant_name))
            }
            ("lead", Some(id)) => {
                let l = entity::prelude::Lead::find_by_id(id)
                    .one(&db)
                    .await?
                    .filter(|l| l.tenant_id == t)
                    .ok_or_else(|| ApiError::NotFound("prospect not found".into()))?;
                (None, Some(l.id), None, Some(l.name))
            }
            ("vendor", Some(id)) => {
                let c = entity::prelude::Counterparty::find_by_id(id)
                    .one(&db)
                    .await?
                    .filter(|c| c.tenant_id == t)
                    .ok_or_else(|| ApiError::NotFound("vendor not found".into()))?;
                (None, None, Some(c.id), Some(c.name))
            }
            ("resident" | "lead" | "vendor", None) => {
                return Err(ApiError::BadRequest("say which one to link".into()))
            }
            _ => {
                return Err(ApiError::BadRequest(
                    "link kind must be resident, lead, vendor or none".into(),
                ))
            }
        };
        am.lease_id = Set(lease);
        am.lead_id = Set(lead);
        am.counterparty_id = Set(vendor);
        if name.is_some() && b.display_name.is_none() {
            am.display_name = Set(name);
        }
        detail.insert(
            "link".into(),
            serde_json::json!({ "kind": link.kind, "id": link.id }),
        );
    }

    if let Some(name) = b.display_name.map(|s| s.trim().to_string()) {
        if name.chars().count() > 120 {
            return Err(ApiError::BadRequest("that name is too long".into()));
        }
        am.display_name = Set((!name.is_empty()).then_some(name.clone()));
        detail.insert("display_name".into(), serde_json::json!(name));
    }

    if detail.is_empty() {
        return Err(ApiError::BadRequest("nothing to change".into()));
    }
    am.updated_at = Set(Utc::now().into());
    let thread = am.update(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::SMS_THREAD_UPDATE,
        Some("sms_thread"),
        Some(thread.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::Value::Object(detail)),
    )
    .await;

    // The new owner hears about it, unless they gave it to themselves.
    if let Some(uid) = new_owner.filter(|u| *u != user.user_id) {
        if let Some(member) = User::find_by_id(uid).one(&db).await? {
            crate::notify::in_app(
                &db,
                scope.tenant_id,
                &member,
                "text_assigned",
                &serde_json::json!({ "sender": who }),
                Some(("sms_thread", thread.id)),
                &format!("assigned:{uid}:{}", Utc::now().timestamp()),
            )
            .await;
        }
    }
    Ok(Json(thread.into()))
}

/// `POST /texts/simulate-call` — test mode only: act as if `phone` called
/// and nobody answered, so the missed-call text-back can be tried.
#[rocket_okapi::openapi(tag = "Texts")]
#[post("/texts/simulate-call", data = "<body>")]
pub async fn simulate_call(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<SimulateCallReq>,
) -> ApiResult<Json<SimulatedCall>> {
    user.require(Permission::MessageManage)?;
    if crate::providers::is_live("sms") {
        return Err(ApiError::Conflict(
            "texts are live — calls arrive through Twilio".into(),
        ));
    }
    let (thread, outcome) = texts::record_missed_call(&db, scope.tenant_id, &body.phone, None)
        .await?
        .ok_or_else(|| ApiError::BadRequest("that doesn't look like a phone number".into()))?;
    Ok(Json(SimulatedCall {
        texted_back: outcome == texts::MissedCall::TextedBack,
        thread: detail(&db, thread).await?,
    }))
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct SimulateCallReq {
    pub phone: String,
}

#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct SimulatedCall {
    pub texted_back: bool,
    pub thread: TextThreadDetailDto,
}

/// `POST /texts/simulate` — test mode only: act as if `phone` texted `body`
/// (STOP/START included), so the inbox can be tried without Twilio.
#[rocket_okapi::openapi(tag = "Texts")]
#[post("/texts/simulate", data = "<body>")]
pub async fn simulate(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<SimulateInboundReq>,
) -> ApiResult<Json<TextThreadDetailDto>> {
    user.require(Permission::MessageManage)?;
    if crate::providers::is_live("sms") {
        return Err(ApiError::Conflict(
            "texts are live — real replies arrive through Twilio".into(),
        ));
    }
    let text = clean_body(&body.body)?;
    let thread = texts::record_inbound(&db, scope.tenant_id, &body.phone, &text, None, &[])
        .await?
        .ok_or_else(|| ApiError::BadRequest("that doesn't look like a phone number".into()))?;
    Ok(Json(detail(&db, thread).await?))
}
