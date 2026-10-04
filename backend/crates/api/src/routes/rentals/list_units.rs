use super::dto::UnitDto;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::{Access, TenantScope};
use entity::prelude::{Asset, Lease, MaintenanceTicket, Meter, Property, Unit};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use std::collections::HashMap;
use uuid::Uuid;

/// `GET /properties/<id>/units` — a property's rentable units, each with who
/// lives there (for people who may see leases) and how much equipment, how
/// many meters and open work orders it has. Anyone who can see the property
/// sees its units; a maintenance crew needs them for the equipment.
#[rocket_okapi::openapi(tag = "Rentals")]
#[get("/properties/<id>/units")]
pub async fn list_units(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    id: &str,
) -> ApiResult<Json<Vec<UnitDto>>> {
    user.require(Permission::PropertyRead)?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    if !access.sees(pid) {
        return Err(ApiError::NotFound("property not found".into()));
    }
    let rows = Unit::find()
        .filter(entity::unit::Column::PropertyId.eq(pid))
        .order_by_asc(entity::unit::Column::UnitNumber)
        .all(&db)
        .await?;

    let mut appliances: HashMap<Uuid, i64> = HashMap::new();
    for a in Asset::find()
        .filter(entity::asset::Column::PropertyId.eq(pid))
        .filter(entity::asset::Column::Status.eq("active"))
        .all(&db)
        .await?
    {
        if let Some(u) = a.unit_id {
            *appliances.entry(u).or_default() += 1;
        }
    }
    let mut meters: HashMap<Uuid, i64> = HashMap::new();
    for m in Meter::find()
        .filter(entity::meter::Column::PropertyId.eq(pid))
        .filter(entity::meter::Column::Status.eq("active"))
        .all(&db)
        .await?
    {
        if let Some(u) = m.unit_id {
            *meters.entry(u).or_default() += 1;
        }
    }
    let mut tickets: HashMap<Uuid, i64> = HashMap::new();
    for t in MaintenanceTicket::find()
        .filter(entity::maintenance_ticket::Column::PropertyId.eq(pid))
        .filter(entity::maintenance_ticket::Column::Status.is_not_in([
            "resolved",
            "closed",
            "cancelled",
        ]))
        .all(&db)
        .await?
    {
        if let Some(u) = t.unit_id {
            *tickets.entry(u).or_default() += 1;
        }
    }
    let mut tenants: HashMap<Uuid, (Uuid, String)> = HashMap::new();
    if user.grants.has_key("lease:read") {
        for l in Lease::find()
            .filter(entity::lease::Column::PropertyId.eq(pid))
            .filter(entity::lease::Column::Status.is_in(["active", "upcoming"]))
            .order_by_asc(entity::lease::Column::StartDate)
            .all(&db)
            .await?
        {
            if let Some(u) = l.unit_id {
                tenants.insert(u, (l.id, l.tenant_name));
            }
        }
    }
    Ok(Json(
        rows.into_iter()
            .map(|u| {
                let uid = u.id;
                let mut d = UnitDto::from(u);
                d.appliances = Some(appliances.get(&uid).copied().unwrap_or(0));
                d.meters = Some(meters.get(&uid).copied().unwrap_or(0));
                d.open_tickets = Some(tickets.get(&uid).copied().unwrap_or(0));
                if let Some((lid, name)) = tenants.get(&uid) {
                    d.lease_id = Some(*lid);
                    d.tenant_name = Some(name.clone());
                }
                d
            })
            .collect(),
    ))
}
