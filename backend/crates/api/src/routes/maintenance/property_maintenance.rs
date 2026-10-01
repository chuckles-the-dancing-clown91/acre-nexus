//! `GET /properties/<id>/maintenance` — the Maintenance tab: open work orders,
//! resolved history, and roll-up counts.

use super::dto::{AssetDto, CategorySpend, MaintenancePlanDto, PropertyMaintenanceResp, TicketDto};
use super::is_open;
use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{Asset, Expense, MaintenancePlan, MaintenanceTicket, Property};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

/// `GET /properties/<id>/maintenance` — split a property's tickets into open
/// work vs resolved history, with counts and the cost of open work.
#[rocket_okapi::openapi(tag = "Maintenance")]
#[get("/properties/<id>/maintenance")]
pub async fn property_maintenance(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<PropertyMaintenanceResp>> {
    user.require(Permission::MaintenanceRead)?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;

    let rows = MaintenanceTicket::find()
        .filter(entity::maintenance_ticket::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::maintenance_ticket::Column::PropertyId.eq(pid))
        .order_by_desc(entity::maintenance_ticket::Column::CreatedAt)
        .all(&db)
        .await?;

    let total_count = rows.len() as i64;
    let mut open: Vec<TicketDto> = Vec::new();
    let mut history: Vec<TicketDto> = Vec::new();
    let mut open_cost_cents: i64 = 0;
    let mut history_cost_cents: i64 = 0;
    let mut last_12mo_cents: i64 = 0;
    let year_ago = chrono::Utc::now() - chrono::Duration::days(365);
    let mut cats: std::collections::BTreeMap<String, (i64, i64)> =
        std::collections::BTreeMap::new();
    for t in rows {
        let cost = t.cost_cents.unwrap_or(0);
        let c = cats.entry(t.category.clone()).or_default();
        c.0 += 1;
        c.1 += cost;
        if is_open(&t.status) {
            open_cost_cents += cost;
            open.push(TicketDto::from(t));
        } else {
            history_cost_cents += cost;
            if t.resolved_at.map(|r| r >= year_ago).unwrap_or(false) {
                last_12mo_cents += cost;
            }
            history.push(TicketDto::from(t));
        }
    }
    let expenses_cents: i64 = Expense::find()
        .filter(entity::expense::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::expense::Column::PropertyId.eq(pid))
        .all(&db)
        .await?
        .iter()
        .map(|x| x.amount_cents)
        .sum();
    let assets = Asset::find()
        .filter(entity::asset::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::asset::Column::PropertyId.eq(pid))
        .order_by_asc(entity::asset::Column::Name)
        .all(&db)
        .await?
        .into_iter()
        .map(AssetDto::from)
        .collect();
    let plans = MaintenancePlan::find()
        .filter(entity::maintenance_plan::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::maintenance_plan::Column::PropertyId.eq(pid))
        .order_by_asc(entity::maintenance_plan::Column::NextDueDate)
        .all(&db)
        .await?
        .into_iter()
        .map(MaintenancePlanDto::from)
        .collect();

    Ok(Json(PropertyMaintenanceResp {
        property_id: pid,
        total_count,
        open_count: open.len() as i64,
        open_cost_cents,
        open_cost_label: usd(open_cost_cents),
        open,
        history,
        history_cost_label: usd(history_cost_cents),
        history_cost_cents,
        last_12mo_cents,
        by_category: cats
            .into_iter()
            .map(|(category, (tickets, cents))| CategorySpend {
                category,
                tickets,
                cents,
                label: usd(cents),
            })
            .collect(),
        expenses_cents,
        assets,
        plans,
    }))
}
