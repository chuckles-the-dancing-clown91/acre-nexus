//! Action items: to-dos on a property or anything on it.

use super::{date, one_of, parse_id, property_in, text};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{ActionItem, User};
use rocket::serde::json::Json;
use rocket::{delete, get, patch, post, State};
use schemars::JsonSchema;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

pub const SUBJECTS: &[&str] = &[
    "property",
    "parcel",
    "permit",
    "insurance",
    "school",
    "asset",
    "utility",
    "tax",
    "document",
    "unit",
    "plan",
];
pub const PRIORITIES: &[&str] = &["low", "normal", "high"];
pub const STATUSES: &[&str] = &["open", "done", "dismissed"];

#[derive(Deserialize, JsonSchema)]
pub struct CreateActionItemReq {
    pub title: String,
    pub subject_type: Option<String>,
    pub subject_id: Option<Uuid>,
    pub notes: Option<String>,
    pub due_on: Option<String>,
    pub priority: Option<String>,
    pub assignee_user_id: Option<Uuid>,
    /// From a "needs attention" suggestion, so it isn't suggested again.
    pub suggestion_key: Option<String>,
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct UpdateActionItemReq {
    pub title: Option<String>,
    pub notes: Option<String>,
    /// "" clears it.
    pub due_on: Option<String>,
    pub priority: Option<String>,
    pub status: Option<String>,
    /// Use `clear_assignee` to unassign.
    pub assignee_user_id: Option<Uuid>,
    #[serde(default)]
    pub clear_assignee: bool,
}

#[derive(Serialize, JsonSchema)]
pub struct ActionItemDto {
    pub id: Uuid,
    pub property_id: Uuid,
    pub subject_type: String,
    pub subject_id: Option<Uuid>,
    pub title: String,
    pub notes: Option<String>,
    pub due_on: Option<String>,
    pub priority: String,
    pub status: String,
    /// Open and past its due date.
    pub overdue: bool,
    pub assignee_user_id: Option<Uuid>,
    pub assignee_name: Option<String>,
    pub suggestion_key: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
}

fn dto(a: entity::action_item::Model, names: &HashMap<Uuid, String>) -> ActionItemDto {
    let today = Utc::now().date_naive().to_string();
    ActionItemDto {
        overdue: a.status == "open" && a.due_on.as_deref().is_some_and(|d| d < today.as_str()),
        assignee_name: a.assignee_user_id.and_then(|u| names.get(&u).cloned()),
        id: a.id,
        property_id: a.property_id,
        subject_type: a.subject_type,
        subject_id: a.subject_id,
        title: a.title,
        notes: a.notes,
        due_on: a.due_on,
        priority: a.priority,
        status: a.status,
        assignee_user_id: a.assignee_user_id,
        suggestion_key: a.suggestion_key,
        completed_at: a.completed_at.map(|t| t.to_rfc3339()),
        created_at: a.created_at.to_rfc3339(),
    }
}

async fn names_for(
    db: &crate::db::RequestDb,
    rows: &[entity::action_item::Model],
) -> ApiResult<HashMap<Uuid, String>> {
    let ids: Vec<Uuid> = rows.iter().filter_map(|r| r.assignee_user_id).collect();
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    Ok(User::find()
        .filter(entity::user::Column::Id.is_in(ids))
        .all(db)
        .await?
        .into_iter()
        .map(|u| (u.id, u.name))
        .collect())
}

/// Open first (high priority, then soonest due), then finished, newest first.
pub fn order(rows: &mut [entity::action_item::Model]) {
    fn pri(p: &str) -> u8 {
        match p {
            "high" => 0,
            "normal" => 1,
            _ => 2,
        }
    }
    rows.sort_by(|a, b| {
        let open = |x: &entity::action_item::Model| x.status != "open";
        open(a).cmp(&open(b)).then_with(|| {
            if a.status == "open" {
                pri(&a.priority)
                    .cmp(&pri(&b.priority))
                    .then_with(|| match (&a.due_on, &b.due_on) {
                        (Some(x), Some(y)) => x.cmp(y),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => a.created_at.cmp(&b.created_at),
                    })
            } else {
                b.updated_at.cmp(&a.updated_at)
            }
        })
    });
}

/// `GET /properties/<id>/action-items?<status>` — `open` (default), `done`,
/// or `all`.
#[rocket_okapi::openapi(tag = "Property Profile")]
#[get("/properties/<id>/action-items?<status>")]
pub async fn list_action_items(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    status: Option<String>,
) -> ApiResult<Json<Vec<ActionItemDto>>> {
    user.require(Permission::PropertyRead)?;
    let property = property_in(&db, scope.tenant_id, id).await?;
    let mut q = ActionItem::find()
        .filter(entity::action_item::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::action_item::Column::PropertyId.eq(property.id));
    match status.as_deref().unwrap_or("open") {
        "all" => {}
        "done" => q = q.filter(entity::action_item::Column::Status.ne("open")),
        _ => q = q.filter(entity::action_item::Column::Status.eq("open")),
    }
    let mut rows = q.all(&db).await?;
    order(&mut rows);
    let names = names_for(&db, &rows).await?;
    Ok(Json(rows.into_iter().map(|r| dto(r, &names)).collect()))
}

/// `POST /properties/<id>/action-items` — add a to-do.
#[rocket_okapi::openapi(tag = "Property Profile")]
#[post("/properties/<id>/action-items", data = "<body>")]
pub async fn create_action_item(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<CreateActionItemReq>,
) -> ApiResult<Json<ActionItemDto>> {
    user.require(Permission::PropertyWrite)?;
    let property = property_in(&db, scope.tenant_id, id).await?;
    let b = body.into_inner();
    let title =
        text(Some(b.title)).ok_or_else(|| ApiError::BadRequest("say what needs doing".into()))?;
    let key = text(b.suggestion_key);
    if let Some(k) = &key {
        if ActionItem::find()
            .filter(entity::action_item::Column::PropertyId.eq(property.id))
            .filter(entity::action_item::Column::SuggestionKey.eq(k.as_str()))
            .one(&db)
            .await?
            .is_some()
        {
            return Err(ApiError::Conflict("that's already on the list".into()));
        }
    }
    let now = Utc::now();
    let saved = entity::action_item::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        property_id: Set(property.id),
        subject_type: Set(one_of("subject", b.subject_type, SUBJECTS, "property")?),
        subject_id: Set(b.subject_id),
        title: Set(title),
        notes: Set(text(b.notes)),
        due_on: Set(date("due date", b.due_on)?),
        priority: Set(one_of("priority", b.priority, PRIORITIES, "normal")?),
        status: Set("open".into()),
        assignee_user_id: Set(b.assignee_user_id),
        suggestion_key: Set(key),
        created_by: Set(Some(user.user_id)),
        completed_at: Set(None),
        completed_by: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;
    let names = names_for(&db, std::slice::from_ref(&saved)).await?;
    Ok(Json(dto(saved, &names)))
}

async fn item_of(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    property_id: Uuid,
    item_id: &str,
) -> ApiResult<entity::action_item::Model> {
    ActionItem::find_by_id(parse_id(item_id)?)
        .filter(entity::action_item::Column::TenantId.eq(tenant_id))
        .filter(entity::action_item::Column::PropertyId.eq(property_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("action item not found".into()))
}

/// `PATCH /properties/<id>/action-items/<item_id>` — tick it off, reopen it,
/// or change it.
#[rocket_okapi::openapi(tag = "Property Profile")]
#[patch("/properties/<id>/action-items/<item_id>", data = "<body>")]
pub async fn update_action_item(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    item_id: &str,
    body: Json<UpdateActionItemReq>,
) -> ApiResult<Json<ActionItemDto>> {
    user.require(Permission::PropertyWrite)?;
    let property = property_in(&db, scope.tenant_id, id).await?;
    let row = item_of(&db, scope.tenant_id, property.id, item_id).await?;
    let b = body.into_inner();
    let now = Utc::now();
    let mut am: entity::action_item::ActiveModel = row.clone().into();
    if let Some(t) = b.title {
        am.title =
            Set(text(Some(t)).ok_or_else(|| ApiError::BadRequest("say what needs doing".into()))?);
    }
    if let Some(n) = b.notes {
        am.notes = Set(text(Some(n)));
    }
    if let Some(d) = b.due_on {
        am.due_on = Set(date("due date", Some(d))?);
    }
    if b.priority.is_some() {
        am.priority = Set(one_of("priority", b.priority, PRIORITIES, "normal")?);
    }
    if b.clear_assignee {
        am.assignee_user_id = Set(None);
    } else if b.assignee_user_id.is_some() {
        am.assignee_user_id = Set(b.assignee_user_id);
    }
    if b.status.is_some() {
        let s = one_of("status", b.status, STATUSES, "open")?;
        if s != row.status {
            if s == "open" {
                am.completed_at = Set(None);
                am.completed_by = Set(None);
            } else {
                am.completed_at = Set(Some(now.into()));
                am.completed_by = Set(Some(user.user_id));
            }
        }
        am.status = Set(s);
    }
    am.updated_at = Set(now.into());
    let saved = am.update(&db).await?;
    let names = names_for(&db, std::slice::from_ref(&saved)).await?;
    Ok(Json(dto(saved, &names)))
}

/// `DELETE /properties/<id>/action-items/<item_id>`
#[rocket_okapi::openapi(tag = "Property Profile")]
#[delete("/properties/<id>/action-items/<item_id>")]
pub async fn delete_action_item(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    item_id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::PropertyWrite)?;
    let property = property_in(&db, scope.tenant_id, id).await?;
    let row = item_of(&db, scope.tenant_id, property.id, item_id).await?;
    ActionItem::delete_by_id(row.id).exec(&db).await?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}
