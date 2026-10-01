use super::dto::{Kpi, PortfolioSummary};
use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::routes::maintenance::OPEN_STATUSES;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{Application, Lease, MaintenanceTicket, Property, Reminder};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

/// `GET /portfolio/summary` — top-line KPIs for the active tenant.
#[rocket_okapi::openapi(tag = "Portfolio")]
#[get("/portfolio/summary")]
pub async fn summary(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: crate::tenancy::Access,
) -> ApiResult<Json<PortfolioSummary>> {
    user.require(Permission::PropertyRead)?;
    let props: Vec<_> = Property::find()
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .all(&db)
        .await?
        .into_iter()
        .filter(|p| access.sees(p.id))
        .collect();

    let count = props.len() as i64;
    let units: i64 = props.iter().map(|p| p.units as i64).sum();
    let occ: i64 = props.iter().map(|p| p.occupied_units as i64).sum();
    let revenue: i64 = props.iter().map(|p| p.monthly_rent_cents).sum();
    let occ_pct = if units > 0 { occ * 100 / units } else { 0 };

    let kpis = vec![
        Kpi {
            label: "Monthly revenue".into(),
            value: usd(revenue),
        },
        Kpi {
            label: "Properties".into(),
            value: count.to_string(),
        },
        Kpi {
            label: "Units".into(),
            value: units.to_string(),
        },
        Kpi {
            label: "Occupancy".into(),
            value: format!("{occ_pct}%"),
        },
    ];

    // Each stat below is gated by its own domain permission (not `property:read`,
    // which only unlocks the summary endpoint itself) and omitted for viewers
    // who can't see that domain, mirroring how the console hides the nav item.
    let (open_tickets, urgent_tickets) =
        if user.grants.has_key(Permission::MaintenanceRead.as_str()) {
            let tickets = MaintenanceTicket::find()
                .filter(entity::maintenance_ticket::Column::TenantId.eq(scope.tenant_id))
                .filter(entity::maintenance_ticket::Column::Status.is_in(OPEN_STATUSES.to_vec()))
                .all(&db)
                .await?
                .into_iter()
                .filter(|t| access.sees(t.property_id))
                .collect::<Vec<_>>();
            let urgent = tickets.iter().filter(|t| t.priority == "urgent").count() as i64;
            (Some(tickets.len() as i64), Some(urgent))
        } else {
            (None, None)
        };

    let (delinquent_tenants, delinquent_balance_cents, delinquent_balance_label) =
        if user.grants.has_key(Permission::LedgerRead.as_str()) {
            let leases = Lease::find()
                .filter(entity::lease::Column::TenantId.eq(scope.tenant_id))
                .filter(entity::lease::Column::BalanceCents.gt(0))
                .all(&db)
                .await?
                .into_iter()
                .filter(|l| access.sees(l.property_id))
                .collect::<Vec<_>>();
            let total: i64 = leases.iter().map(|l| l.balance_cents).sum();
            (Some(leases.len() as i64), Some(total), Some(usd(total)))
        } else {
            (None, None, None)
        };

    let pending_applications = if user.grants.has_key(Permission::ApplicationRead.as_str()) {
        // Applications reach a property through their listing.
        let listing_property: std::collections::HashMap<uuid::Uuid, Option<uuid::Uuid>> =
            if access.is_scoped() {
                entity::prelude::Listing::find()
                    .filter(entity::listing::Column::TenantId.eq(scope.tenant_id))
                    .all(&db)
                    .await?
                    .into_iter()
                    .map(|l| (l.id, l.property_id))
                    .collect()
            } else {
                Default::default()
            };
        let n = Application::find()
            .filter(entity::application::Column::TenantId.eq(scope.tenant_id))
            .filter(entity::application::Column::Status.eq("Screening"))
            .all(&db)
            .await?
            .into_iter()
            .filter(|a| {
                !access.is_scoped()
                    || a.listing_id
                        .and_then(|l| listing_property.get(&l).copied().flatten())
                        .is_some_and(|p| access.sees(p))
            })
            .count() as i64;
        Some(n)
    } else {
        None
    };

    let (upcoming_reminders, overdue_reminders) =
        if user.grants.has_key(Permission::CalendarRead.as_str()) {
            let today = Utc::now().date_naive();
            let reminders = Reminder::find()
                .filter(entity::reminder::Column::TenantId.eq(scope.tenant_id))
                .filter(entity::reminder::Column::Status.eq("active"))
                .all(&db)
                .await?;
            let mut upcoming = 0i64;
            let mut overdue = 0i64;
            // Scoped people see the reminders on their own properties.
            let reminders: Vec<_> = reminders
                .into_iter()
                .filter(|r| {
                    !access.is_scoped()
                        || r.subject_type == "property"
                            && r.subject_id.is_some_and(|p| access.sees(p))
                })
                .collect();
            for r in &reminders {
                match crate::reminders::days_until(&r.due_date, today) {
                    Some(d) if d < 0 => overdue += 1,
                    Some(d) if d <= 14 => upcoming += 1,
                    _ => {}
                }
            }
            (Some(upcoming), Some(overdue))
        } else {
            (None, None)
        };

    Ok(Json(PortfolioSummary {
        properties: count,
        units,
        occupied_units: occ,
        occupancy_pct: occ_pct,
        monthly_revenue_cents: revenue,
        kpis,
        open_tickets,
        urgent_tickets,
        delinquent_tenants,
        delinquent_balance_cents,
        delinquent_balance_label,
        pending_applications,
        upcoming_reminders,
        overdue_reminders,
    }))
}
