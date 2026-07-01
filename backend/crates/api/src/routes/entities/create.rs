use super::dto::{CounterpartyDto, CreateCounterpartyReq};
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, Set};
use uuid::Uuid;

/// `POST /entities` — register a new counterparty in the active tenant.
#[rocket_okapi::openapi(tag = "Entities")]
#[post("/entities", data = "<body>")]
pub async fn create(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<CreateCounterpartyReq>,
) -> ApiResult<Json<CounterpartyDto>> {
    user.require(Permission::EntityManage)?;
    let b = body.into_inner();
    let now = Utc::now();
    let kind = if b.kind.is_empty() {
        "other".to_string()
    } else {
        b.kind
    };
    let model = entity::counterparty::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        kind: Set(kind),
        name: Set(b.name),
        contact_name: Set(b.contact_name),
        email: Set(b.email),
        phone: Set(b.phone),
        website: Set(b.website),
        address: Set(b.address),
        notes: Set(b.notes),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    };
    let saved = model.insert(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::ENTITY_CREATE,
        Some("counterparty"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "kind": saved.kind, "name": saved.name })),
    )
    .await;
    Ok(Json(CounterpartyDto::from(saved)))
}
