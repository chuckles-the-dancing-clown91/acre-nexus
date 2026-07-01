//! `GET /entities/<entity_id>/cap-table` — the legal entity's ownership table.

use super::dto::{CapTableResp, CapTableRow};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{EntityOwnership, Llc, Owner};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use std::collections::HashMap;
use uuid::Uuid;

/// Format basis points as a percentage label (e.g. 4000 -> "40.0%").
pub(crate) fn bps_label(bps: i32) -> String {
    format!("{:.1}%", bps as f64 / 100.0)
}

/// `GET /entities/<entity_id>/cap-table` — owners + stakes for one LLC.
#[rocket_okapi::openapi(tag = "Legal Entities")]
#[get("/entities/<entity_id>/cap-table")]
pub async fn list(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    entity_id: &str,
) -> ApiResult<Json<CapTableResp>> {
    user.require(Permission::EntityRead)?;
    let eid =
        Uuid::parse_str(entity_id).map_err(|_| ApiError::BadRequest("invalid entity id".into()))?;

    // The legal entity must belong to the active tenant.
    let llc = Llc::find_by_id(eid)
        .one(&db)
        .await?
        .filter(|l| l.tenant_id == scope.tenant_id)
        .ok_or_else(|| ApiError::NotFound("legal entity not found".into()))?;

    let rows = EntityOwnership::find()
        .filter(entity::entity_ownership::Column::EntityId.eq(llc.id))
        .all(&db)
        .await?;

    let mut owners: HashMap<Uuid, (String, String)> = HashMap::new();
    for o in Owner::find()
        .filter(entity::owner::Column::TenantId.eq(scope.tenant_id))
        .all(&db)
        .await?
    {
        owners.insert(o.id, (o.name, o.kind));
    }

    let mut total = 0i32;
    let cap_rows = rows
        .into_iter()
        .map(|r| {
            total += r.ownership_bps;
            let (name, kind) = owners
                .get(&r.owner_id)
                .cloned()
                .unwrap_or_else(|| ("(unknown)".into(), "individual".into()));
            CapTableRow {
                ownership_id: r.id,
                owner_id: r.owner_id,
                owner_name: name,
                owner_kind: kind,
                ownership_bps: r.ownership_bps,
                ownership_label: bps_label(r.ownership_bps),
                role: r.role,
            }
        })
        .collect();

    Ok(Json(CapTableResp {
        entity_id: llc.id,
        rows: cap_rows,
        total_bps: total,
        total_label: bps_label(total),
    }))
}
