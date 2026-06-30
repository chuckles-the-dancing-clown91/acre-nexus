//! `POST /entities/<entity_id>/cap-table` — add an owner's stake to a legal
//! entity, creating the owner inline when only a name is given.

use super::dto::AddOwnershipReq;
use super::list::bps_label;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{EntityOwnership, Llc, Owner};
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `POST /entities/<entity_id>/cap-table` — add a cap-table row.
#[rocket_okapi::openapi(tag = "Legal Entities")]
#[post("/entities/<entity_id>/cap-table", data = "<body>")]
pub async fn add(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    entity_id: &str,
    body: Json<AddOwnershipReq>,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::EntityManage)?;
    let eid =
        Uuid::parse_str(entity_id).map_err(|_| ApiError::BadRequest("invalid entity id".into()))?;
    let b = body.into_inner();

    if b.ownership_bps < 0 || b.ownership_bps > 10000 {
        return Err(ApiError::BadRequest(
            "ownership_bps must be between 0 and 10000".into(),
        ));
    }

    // The legal entity must belong to the active tenant.
    let llc = Llc::find_by_id(eid)
        .one(&state.db)
        .await?
        .filter(|l| l.tenant_id == scope.tenant_id)
        .ok_or_else(|| ApiError::NotFound("legal entity not found".into()))?;

    // A cap table may never allocate more than 100% (10000 bps).
    let allocated: i32 = EntityOwnership::find()
        .filter(entity::entity_ownership::Column::EntityId.eq(llc.id))
        .all(&state.db)
        .await?
        .iter()
        .map(|r| r.ownership_bps)
        .sum();
    if allocated + b.ownership_bps > 10000 {
        return Err(ApiError::BadRequest(format!(
            "cap table would exceed 100%: {:.1}% already allocated, {:.1}% requested",
            allocated as f64 / 100.0,
            b.ownership_bps as f64 / 100.0
        )));
    }

    // Resolve the owner: existing reference (scoped to tenant) or create inline.
    let owner_id = match b.owner_id {
        Some(id) => {
            Owner::find_by_id(id)
                .one(&state.db)
                .await?
                .filter(|o| o.tenant_id == scope.tenant_id)
                .ok_or_else(|| ApiError::NotFound("owner not found".into()))?;
            id
        }
        None => {
            let name = b
                .owner_name
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| ApiError::BadRequest("owner_id or owner_name is required".into()))?;
            let oid = Uuid::new_v4();
            entity::owner::ActiveModel {
                id: Set(oid),
                tenant_id: Set(scope.tenant_id),
                kind: Set(b.owner_kind.clone().unwrap_or_else(|| "individual".into())),
                name: Set(name.to_string()),
                email: Set(None),
                phone: Set(None),
                notes: Set(None),
                created_at: Set(Utc::now().into()),
            }
            .insert(&state.db)
            .await?;
            crate::audit::record(
                &state.db,
                Some(user.user_id),
                crate::audit::actions::OWNER_CREATE,
                Some("owner"),
                Some(oid.to_string()),
                Some(scope.tenant_id),
                Some(serde_json::json!({ "name": name })),
            )
            .await;
            oid
        }
    };

    let ownership_id = Uuid::new_v4();
    entity::entity_ownership::ActiveModel {
        id: Set(ownership_id),
        tenant_id: Set(scope.tenant_id),
        entity_id: Set(llc.id),
        owner_id: Set(owner_id),
        ownership_bps: Set(b.ownership_bps),
        role: Set(b.role.unwrap_or_else(|| "investor".into())),
        created_at: Set(Utc::now().into()),
    }
    .insert(&state.db)
    .await?;

    crate::audit::record(
        &state.db,
        Some(user.user_id),
        crate::audit::actions::ENTITY_OWNERSHIP_ADD,
        Some("entity_ownership"),
        Some(ownership_id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({
            "entity_id": llc.id,
            "owner_id": owner_id,
            "ownership_bps": b.ownership_bps,
        })),
    )
    .await;

    Ok(Json(serde_json::json!({
        "ownership_id": ownership_id,
        "owner_id": owner_id,
        "ownership_label": bps_label(b.ownership_bps),
    })))
}
