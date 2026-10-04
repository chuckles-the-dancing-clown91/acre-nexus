//! **Operations analytics and the portfolio map** (roadmap area 15).
//!
//! - `GET /analytics/operations?months=&property_id=`: turns (count, days
//!   vacant, cost) and work orders (opened, resolved, past their resolve
//!   target, rating) by month and by property; the issues that keep coming
//!   back at a property; and appliances whose repair spend has passed a share
//!   of their price (`analytics.replace_share_pct`), flagged to replace.
//! - `GET /analytics/leasing?months=`: tour requests, applications and leases
//!   in the window, and days on market for each listing.
//! - `GET /portfolio/map`: every property in reach with its coordinates,
//!   occupancy, open work and turns, and whether it has a site map.
//!
//! All of it is narrowed to the properties the caller can see.

use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::tenancy::{Access, TenantScope};
use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use entity::prelude::{
    Application, Asset, Lease, Listing, MaintenanceTicket, Process, ProcessStep, Property,
    PropertyDetail, SiteMap, TourRequest,
};
use rocket::get;
use rocket::serde::json::Json;
use schemars::JsonSchema;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

/// A month as `YYYY-MM`.
fn month_of(d: NaiveDate) -> String {
    format!("{:04}-{:02}", d.year(), d.month())
}

/// The last `n` months, oldest first, ending with this one.
pub fn months_back(today: NaiveDate, n: u32) -> Vec<String> {
    let mut out = Vec::new();
    let (mut y, mut m) = (today.year(), today.month() as i32);
    for _ in 0..n.max(1) {
        out.push(format!("{y:04}-{m:02}"));
        m -= 1;
        if m == 0 {
            m = 12;
            y -= 1;
        }
    }
    out.reverse();
    out
}

fn first_of(month: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(&format!("{month}-01"), "%Y-%m-%d").ok()
}

fn date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s.get(..10)?, "%Y-%m-%d").ok()
}

fn avg(sum: f64, n: usize) -> Option<f64> {
    (n > 0).then(|| (sum / n as f64 * 10.0).round() / 10.0)
}

// ---- turns ---------------------------------------------------------------

/// One finished turn, as the analytics see it.
#[derive(Clone, Debug)]
pub struct TurnFact {
    pub property_id: Uuid,
    pub started: NaiveDate,
    pub finished: NaiveDate,
    pub cost_cents: i64,
}

#[derive(Serialize, JsonSchema, Debug, Default, Clone, PartialEq)]
pub struct TurnStats {
    pub count: usize,
    pub avg_days: Option<f64>,
    pub avg_cost_cents: Option<i64>,
    pub total_cost_cents: i64,
}

pub fn turn_stats<'a>(turns: impl Iterator<Item = &'a TurnFact>) -> TurnStats {
    let (mut n, mut days, mut cost) = (0usize, 0f64, 0i64);
    for t in turns {
        n += 1;
        days += (t.finished - t.started).num_days().max(0) as f64;
        cost += t.cost_cents;
    }
    TurnStats {
        count: n,
        avg_days: avg(days, n),
        avg_cost_cents: (n > 0).then(|| cost / n as i64),
        total_cost_cents: cost,
    }
}

// ---- tickets -------------------------------------------------------------

/// One work order, as the analytics see it.
#[derive(Clone, Debug)]
pub struct TicketFact {
    pub property_id: Uuid,
    pub category: String,
    pub asset_id: Option<Uuid>,
    pub opened: DateTime<Utc>,
    pub resolved: Option<DateTime<Utc>>,
    pub resolve_due: Option<DateTime<Utc>>,
    pub rating: Option<i32>,
    pub cost_cents: i64,
}

impl TicketFact {
    /// Finished after its target, or still open past it.
    pub fn past_sla(&self, now: DateTime<Utc>) -> bool {
        match (self.resolve_due, self.resolved) {
            (Some(due), Some(done)) => done > due,
            (Some(due), None) => now > due,
            _ => false,
        }
    }
}

#[derive(Serialize, JsonSchema, Debug, Default, Clone, PartialEq)]
pub struct TicketStats {
    pub opened: usize,
    pub resolved: usize,
    pub past_sla: usize,
    pub avg_rating: Option<f64>,
    pub avg_hours_to_resolve: Option<f64>,
    pub spend_cents: i64,
}

pub fn ticket_stats<'a>(
    tickets: impl Iterator<Item = &'a TicketFact>,
    now: DateTime<Utc>,
) -> TicketStats {
    let mut s = TicketStats::default();
    let (mut rated, mut rating_sum) = (0usize, 0f64);
    let (mut hours_n, mut hours) = (0usize, 0f64);
    for t in tickets {
        s.opened += 1;
        if let Some(r) = t.resolved {
            s.resolved += 1;
            hours_n += 1;
            hours += (r - t.opened).num_minutes().max(0) as f64 / 60.0;
        }
        if t.past_sla(now) {
            s.past_sla += 1;
        }
        if let Some(r) = t.rating {
            rated += 1;
            rating_sum += r as f64;
        }
        s.spend_cents += t.cost_cents;
    }
    s.avg_rating = avg(rating_sum, rated);
    s.avg_hours_to_resolve = avg(hours, hours_n);
    s
}

/// The same kind of problem at the same property, again and again.
#[derive(Serialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct RepeatIssue {
    pub property_id: Uuid,
    pub property_name: String,
    pub category: String,
    pub count: usize,
    pub spend_cents: i64,
    pub last_opened: String,
}

pub fn repeat_issues(
    tickets: &[TicketFact],
    names: &HashMap<Uuid, String>,
    min_count: usize,
) -> Vec<RepeatIssue> {
    let mut groups: HashMap<(Uuid, String), (usize, i64, DateTime<Utc>)> = HashMap::new();
    for t in tickets {
        let e = groups
            .entry((t.property_id, t.category.clone()))
            .or_insert((0, 0, t.opened));
        e.0 += 1;
        e.1 += t.cost_cents;
        if t.opened > e.2 {
            e.2 = t.opened;
        }
    }
    let mut out: Vec<RepeatIssue> = groups
        .into_iter()
        .filter(|(_, (n, _, _))| *n >= min_count)
        .map(|((pid, cat), (n, spend, last))| RepeatIssue {
            property_id: pid,
            property_name: names.get(&pid).cloned().unwrap_or_default(),
            category: cat,
            count: n,
            spend_cents: spend,
            last_opened: last.date_naive().to_string(),
        })
        .collect();
    out.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then(b.spend_cents.cmp(&a.spend_cents))
    });
    out
}

/// An appliance whose repairs are adding up.
#[derive(Serialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct ApplianceSpend {
    pub asset_id: Uuid,
    pub name: String,
    pub kind: String,
    pub property_id: Uuid,
    pub property_name: String,
    pub repairs: usize,
    pub spend_cents: i64,
    pub price_cents: Option<i64>,
    /// Repair spend as a share of the price, in percent.
    pub share_pct: Option<i64>,
    /// Spend has passed the replace threshold.
    pub replace: bool,
}

pub fn appliance_spend(
    tickets: &[TicketFact],
    assets: &[entity::asset::Model],
    names: &HashMap<Uuid, String>,
    replace_share_pct: i64,
) -> Vec<ApplianceSpend> {
    let mut by_asset: HashMap<Uuid, (usize, i64)> = HashMap::new();
    for t in tickets {
        if let Some(a) = t.asset_id {
            let e = by_asset.entry(a).or_default();
            e.0 += 1;
            e.1 += t.cost_cents;
        }
    }
    let mut out: Vec<ApplianceSpend> = assets
        .iter()
        .filter_map(|a| {
            let (repairs, spend) = *by_asset.get(&a.id)?;
            let price = a.purchase_price_cents.filter(|p| *p > 0);
            let share = price.map(|p| spend * 100 / p);
            Some(ApplianceSpend {
                asset_id: a.id,
                name: a.name.clone(),
                kind: a.kind.clone(),
                property_id: a.property_id,
                property_name: names.get(&a.property_id).cloned().unwrap_or_default(),
                repairs,
                spend_cents: spend,
                price_cents: price,
                share_pct: share,
                replace: share.is_some_and(|s| s >= replace_share_pct),
            })
        })
        .collect();
    out.sort_by(|a, b| {
        b.replace
            .cmp(&a.replace)
            .then(b.share_pct.unwrap_or(0).cmp(&a.share_pct.unwrap_or(0)))
            .then(b.spend_cents.cmp(&a.spend_cents))
    });
    out
}

// ---- responses -----------------------------------------------------------

#[derive(Serialize, JsonSchema)]
pub struct MonthRow {
    pub month: String,
    pub turns: TurnStats,
    pub tickets: TicketStats,
}

#[derive(Serialize, JsonSchema)]
pub struct PropertyRow {
    pub property_id: Uuid,
    pub name: String,
    pub units: i32,
    pub occupied: i32,
    pub turns: TurnStats,
    pub tickets: TicketStats,
}

#[derive(Serialize, JsonSchema)]
pub struct CategoryRow {
    pub category: String,
    pub count: usize,
    pub spend_cents: i64,
}

#[derive(Serialize, JsonSchema)]
pub struct OperationsResp {
    pub from: String,
    pub to: String,
    pub totals: MonthRow,
    pub months: Vec<MonthRow>,
    pub properties: Vec<PropertyRow>,
    pub categories: Vec<CategoryRow>,
    pub repeats: Vec<RepeatIssue>,
    pub appliances: Vec<ApplianceSpend>,
    pub replace_share_pct: i64,
    /// Turns still open now.
    pub turns_open: usize,
    /// Work orders still open now.
    pub tickets_open: usize,
}

async fn properties_in(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    access: &Access,
    only: Option<Uuid>,
) -> ApiResult<Vec<entity::property::Model>> {
    let mut q = Property::find().filter(entity::property::Column::TenantId.eq(tenant_id));
    if let Some(ids) = access.property_ids() {
        q = q.filter(entity::property::Column::Id.is_in(ids));
    }
    if let Some(p) = only {
        q = q.filter(entity::property::Column::Id.eq(p));
    }
    Ok(q.order_by_asc(entity::property::Column::Name)
        .all(db)
        .await?)
}

/// Finished turns and the number still open, for these properties.
async fn turn_facts(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    ids: &[Uuid],
) -> ApiResult<(Vec<TurnFact>, usize)> {
    let runs = Process::find()
        .filter(entity::process::Column::TenantId.eq(tenant_id))
        .filter(entity::process::Column::Kind.eq(crate::process::KIND_TURNOVER))
        .filter(entity::process::Column::PropertyId.is_in(ids.to_vec()))
        .all(db)
        .await?;
    let run_ids: Vec<Uuid> = runs.iter().map(|r| r.id).collect();
    let mut cost: HashMap<Uuid, i64> = HashMap::new();
    if !run_ids.is_empty() {
        for s in ProcessStep::find()
            .filter(entity::process_step::Column::TenantId.eq(tenant_id))
            .filter(entity::process_step::Column::ProcessId.is_in(run_ids))
            .all(db)
            .await?
        {
            *cost.entry(s.process_id).or_default() += s.cost_cents.unwrap_or(0);
        }
    }
    let mut open = 0;
    let mut facts = Vec::new();
    for r in runs {
        match (date(&r.started_on), r.finished_on.as_deref().and_then(date)) {
            (Some(started), Some(finished)) => facts.push(TurnFact {
                property_id: r.property_id,
                started,
                finished,
                cost_cents: cost.get(&r.id).copied().unwrap_or(0),
            }),
            _ if r.status != "cancelled" => open += 1,
            _ => {}
        }
    }
    Ok((facts, open))
}

async fn ticket_facts(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    ids: &[Uuid],
    since: DateTime<Utc>,
) -> ApiResult<(Vec<TicketFact>, usize)> {
    let rows = MaintenanceTicket::find()
        .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
        .filter(entity::maintenance_ticket::Column::PropertyId.is_in(ids.to_vec()))
        .all(db)
        .await?;
    let open = rows
        .iter()
        .filter(|t| crate::routes::maintenance::is_open(&t.status))
        .count();
    let facts = rows
        .into_iter()
        .filter(|t| t.created_at.with_timezone(&Utc) >= since)
        .map(|t| TicketFact {
            property_id: t.property_id,
            category: t.category,
            asset_id: t.asset_id,
            opened: t.created_at.with_timezone(&Utc),
            resolved: t.resolved_at.map(|r| r.with_timezone(&Utc)),
            resolve_due: t.sla_resolve_due_at.map(|r| r.with_timezone(&Utc)),
            rating: t.rating,
            cost_cents: t.cost_cents.unwrap_or(0),
        })
        .collect();
    Ok((facts, open))
}

/// `GET /analytics/operations` — turns and work orders by month and property,
/// repeat issues, and appliances to replace.
#[rocket_okapi::openapi(tag = "Reports")]
#[get("/analytics/operations?<months>&<property_id>")]
pub async fn operations(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    months: Option<u32>,
    property_id: Option<&str>,
) -> ApiResult<Json<OperationsResp>> {
    user.require(Permission::ReportRead)?;
    let property_id =
        match property_id.filter(|s| !s.is_empty()) {
            Some(s) => Some(Uuid::parse_str(s).map_err(|_| {
                crate::error::ApiError::BadRequest("property_id is not an id".into())
            })?),
            None => None,
        };
    let t = scope.tenant_id;
    let now = Utc::now();
    let today = now.date_naive();
    let span = months.unwrap_or(12).clamp(1, 36);
    let labels = months_back(today, span);
    let from = first_of(&labels[0]).unwrap_or(today);
    let since = from.and_hms_opt(0, 0, 0).unwrap().and_utc();
    let props = properties_in(&db, t, &access, property_id).await?;
    let ids: Vec<Uuid> = props.iter().map(|p| p.id).collect();
    let names: HashMap<Uuid, String> = props.iter().map(|p| (p.id, p.name.clone())).collect();

    let (all_turns, turns_open) = turn_facts(&db, t, &ids).await?;
    let turns: Vec<TurnFact> = all_turns
        .into_iter()
        .filter(|x| x.finished >= from)
        .collect();
    let (tickets, tickets_open) = ticket_facts(&db, t, &ids, since).await?;
    let assets = if ids.is_empty() {
        vec![]
    } else {
        Asset::find()
            .filter(entity::asset::Column::TenantId.eq(t))
            .filter(entity::asset::Column::PropertyId.is_in(ids.clone()))
            .all(&db)
            .await?
    };
    let share = crate::settings::get_i64(&db, t, crate::settings::ANALYTICS_REPLACE_SHARE_PCT)
        .await
        .clamp(1, 1000);

    let month_rows = labels
        .iter()
        .map(|m| MonthRow {
            month: m.clone(),
            turns: turn_stats(turns.iter().filter(|x| &month_of(x.finished) == m)),
            tickets: ticket_stats(
                tickets
                    .iter()
                    .filter(|x| &month_of(x.opened.date_naive()) == m),
                now,
            ),
        })
        .collect();
    let property_rows = props
        .iter()
        .map(|p| PropertyRow {
            property_id: p.id,
            name: p.name.clone(),
            units: p.units,
            occupied: p.occupied_units,
            turns: turn_stats(turns.iter().filter(|x| x.property_id == p.id)),
            tickets: ticket_stats(tickets.iter().filter(|x| x.property_id == p.id), now),
        })
        .collect();
    let mut cats: BTreeMap<String, (usize, i64)> = BTreeMap::new();
    for x in &tickets {
        let e = cats.entry(x.category.clone()).or_default();
        e.0 += 1;
        e.1 += x.cost_cents;
    }
    let mut categories: Vec<CategoryRow> = cats
        .into_iter()
        .map(|(category, (count, spend_cents))| CategoryRow {
            category,
            count,
            spend_cents,
        })
        .collect();
    categories.sort_by_key(|c| std::cmp::Reverse(c.count));

    Ok(Json(OperationsResp {
        from: from.to_string(),
        to: today.to_string(),
        totals: MonthRow {
            month: "all".into(),
            turns: turn_stats(turns.iter()),
            tickets: ticket_stats(tickets.iter(), now),
        },
        months: month_rows,
        properties: property_rows,
        categories,
        repeats: repeat_issues(&tickets, &names, 3),
        appliances: appliance_spend(&tickets, &assets, &names, share),
        replace_share_pct: share,
        turns_open,
        tickets_open,
    }))
}

// ---- leasing -------------------------------------------------------------

#[derive(Serialize, JsonSchema)]
pub struct ListingDays {
    pub listing_id: Uuid,
    pub title: String,
    pub status: String,
    pub rent_cents: i64,
    pub listed_on: String,
    /// Days from listing to the first lease off one of its applications, or
    /// to today while it's still on the market.
    pub days_on_market: i64,
    pub leased: bool,
    pub tours: usize,
    pub applications: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct LeasingResp {
    pub from: String,
    pub tours: usize,
    pub applications: usize,
    pub approved: usize,
    pub leases: usize,
    /// Percent of tours that became an application, and of applications
    /// that became a lease.
    pub tour_to_application_pct: Option<i64>,
    pub application_to_lease_pct: Option<i64>,
    pub avg_days_on_market: Option<f64>,
    pub listings: Vec<ListingDays>,
}

fn pct(a: usize, b: usize) -> Option<i64> {
    (b > 0).then(|| (a as i64 * 100) / b as i64)
}

/// `GET /analytics/leasing` — tours to applications to leases, and days on
/// market.
#[rocket_okapi::openapi(tag = "Reports")]
#[get("/analytics/leasing?<months>")]
pub async fn leasing(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    months: Option<u32>,
) -> ApiResult<Json<LeasingResp>> {
    user.require(Permission::ReportRead)?;
    let t = scope.tenant_id;
    let now = Utc::now();
    let since = now - Duration::days(30 * months.unwrap_or(6).clamp(1, 36) as i64);
    let reach: Option<HashSet<Uuid>> = access.property_ids().map(|v| v.into_iter().collect());
    let listings: Vec<entity::listing::Model> = Listing::find()
        .filter(entity::listing::Column::TenantId.eq(t))
        .all(&db)
        .await?
        .into_iter()
        .filter(|l| match (&reach, l.property_id) {
            (None, _) => true,
            (Some(r), Some(p)) => r.contains(&p),
            (Some(_), None) => false,
        })
        .collect();
    let listing_ids: HashSet<Uuid> = listings.iter().map(|l| l.id).collect();
    let tours: Vec<entity::tour_request::Model> = TourRequest::find()
        .filter(entity::tour_request::Column::TenantId.eq(t))
        .filter(entity::tour_request::Column::CreatedAt.gte(since))
        .all(&db)
        .await?
        .into_iter()
        .filter(|x| reach.is_none() || x.listing_id.is_some_and(|l| listing_ids.contains(&l)))
        .collect();
    let apps: Vec<entity::application::Model> = Application::find()
        .filter(entity::application::Column::TenantId.eq(t))
        .all(&db)
        .await?
        .into_iter()
        .filter(|x| reach.is_none() || x.listing_id.is_some_and(|l| listing_ids.contains(&l)))
        .collect();
    let leases = Lease::find()
        .filter(entity::lease::Column::TenantId.eq(t))
        .filter(entity::lease::Column::ApplicationId.is_not_null())
        .all(&db)
        .await?;
    let lease_by_app: HashMap<Uuid, &entity::lease::Model> = leases
        .iter()
        .filter_map(|l| l.application_id.map(|a| (a, l)))
        .collect();
    let recent_apps: Vec<&entity::application::Model> = apps
        .iter()
        .filter(|a| a.created_at.with_timezone(&Utc) >= since)
        .collect();
    let approved = recent_apps
        .iter()
        .filter(|a| a.status == "approved" || lease_by_app.contains_key(&a.id))
        .count();
    let leased = recent_apps
        .iter()
        .filter(|a| lease_by_app.contains_key(&a.id))
        .count();

    let today = now.date_naive();
    let mut rows: Vec<ListingDays> = listings
        .iter()
        .map(|l| {
            let listed = l.created_at.with_timezone(&Utc).date_naive();
            let mine: Vec<&entity::application::Model> =
                apps.iter().filter(|a| a.listing_id == Some(l.id)).collect();
            let first_lease = mine
                .iter()
                .filter_map(|a| lease_by_app.get(&a.id))
                .map(|le| le.created_at.with_timezone(&Utc).date_naive())
                .min();
            let end = first_lease.unwrap_or(today);
            ListingDays {
                listing_id: l.id,
                title: l.title.clone(),
                status: l.status.clone(),
                rent_cents: l.rent_cents,
                listed_on: listed.to_string(),
                days_on_market: (end - listed).num_days().max(0),
                leased: first_lease.is_some(),
                tours: tours.iter().filter(|x| x.listing_id == Some(l.id)).count(),
                applications: mine.len(),
            }
        })
        .collect();
    rows.sort_by_key(|r| std::cmp::Reverse(r.days_on_market));
    let leased_rows: Vec<&ListingDays> = rows.iter().filter(|r| r.leased).collect();
    let dom = avg(
        leased_rows.iter().map(|r| r.days_on_market as f64).sum(),
        leased_rows.len(),
    );
    Ok(Json(LeasingResp {
        from: since.date_naive().to_string(),
        tours: tours.len(),
        applications: recent_apps.len(),
        approved,
        leases: leased,
        tour_to_application_pct: pct(recent_apps.len(), tours.len()),
        application_to_lease_pct: pct(leased, recent_apps.len()),
        avg_days_on_market: dom,
        listings: rows,
    }))
}

// ---- portfolio map -------------------------------------------------------

#[derive(Serialize, JsonSchema)]
pub struct MapPin {
    pub property_id: Uuid,
    pub name: String,
    pub address: String,
    pub city: String,
    pub state: String,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub units: i32,
    pub occupied: i32,
    /// Percent occupied, when it has units.
    pub occupancy_pct: Option<i64>,
    pub open_tickets: usize,
    pub urgent_tickets: usize,
    pub open_turns: usize,
    pub site_map_id: Option<Uuid>,
    pub image_url: Option<String>,
    pub monthly_rent_cents: i64,
}

#[derive(Serialize, JsonSchema)]
pub struct PortfolioMapResp {
    pub pins: Vec<MapPin>,
    /// Properties with no coordinates yet (refresh their data to place them).
    pub unplaced: usize,
}

/// `GET /portfolio/map` — every property in reach on one map.
#[rocket_okapi::openapi(tag = "Property Profile")]
#[get("/portfolio/map")]
pub async fn portfolio_map(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
) -> ApiResult<Json<PortfolioMapResp>> {
    user.require(Permission::PropertyRead)?;
    let t = scope.tenant_id;
    let props = properties_in(&db, t, &access, None).await?;
    let ids: Vec<Uuid> = props.iter().map(|p| p.id).collect();
    if ids.is_empty() {
        return Ok(Json(PortfolioMapResp {
            pins: vec![],
            unplaced: 0,
        }));
    }
    let coords: HashMap<Uuid, (Option<f64>, Option<f64>)> = PropertyDetail::find()
        .filter(entity::property_detail::Column::TenantId.eq(t))
        .filter(entity::property_detail::Column::PropertyId.is_in(ids.clone()))
        .all(&db)
        .await?
        .into_iter()
        .map(|d| (d.property_id, (d.latitude, d.longitude)))
        .collect();
    let mut open: HashMap<Uuid, (usize, usize)> = HashMap::new();
    for tk in MaintenanceTicket::find()
        .filter(entity::maintenance_ticket::Column::TenantId.eq(t))
        .filter(entity::maintenance_ticket::Column::PropertyId.is_in(ids.clone()))
        .filter(
            entity::maintenance_ticket::Column::Status
                .is_in(crate::routes::maintenance::OPEN_STATUSES.to_vec()),
        )
        .all(&db)
        .await?
    {
        let e = open.entry(tk.property_id).or_default();
        e.0 += 1;
        if tk.priority == "urgent" || tk.priority == "emergency" || tk.priority == "high" {
            e.1 += 1;
        }
    }
    let mut turns: HashMap<Uuid, usize> = HashMap::new();
    for r in Process::find()
        .filter(entity::process::Column::TenantId.eq(t))
        .filter(entity::process::Column::Kind.eq(crate::process::KIND_TURNOVER))
        .filter(entity::process::Column::PropertyId.is_in(ids.clone()))
        .filter(entity::process::Column::FinishedOn.is_null())
        .all(&db)
        .await?
    {
        if r.status != "cancelled" {
            *turns.entry(r.property_id).or_default() += 1;
        }
    }
    let mut maps: HashMap<Uuid, Uuid> = HashMap::new();
    for m in SiteMap::find()
        .filter(entity::site_map::Column::TenantId.eq(t))
        .filter(entity::site_map::Column::PropertyId.is_in(ids))
        .all(&db)
        .await?
    {
        maps.entry(m.property_id).or_insert(m.id);
    }
    let pins: Vec<MapPin> = props
        .into_iter()
        .map(|p| {
            let (lat, lng) = coords.get(&p.id).copied().unwrap_or((None, None));
            let (open_tickets, urgent_tickets) = open.get(&p.id).copied().unwrap_or((0, 0));
            MapPin {
                property_id: p.id,
                occupancy_pct: (p.units > 0)
                    .then(|| (p.occupied_units as i64 * 100) / p.units as i64),
                name: p.name,
                address: p.address,
                city: p.city,
                state: p.state,
                lat,
                lng,
                units: p.units,
                occupied: p.occupied_units,
                open_tickets,
                urgent_tickets,
                open_turns: turns.get(&p.id).copied().unwrap_or(0),
                site_map_id: maps.get(&p.id).copied(),
                image_url: p.image_url,
                monthly_rent_cents: p.monthly_rent_cents,
            }
        })
        .collect();
    let unplaced = pins
        .iter()
        .filter(|p| p.lat.is_none() || p.lng.is_none())
        .count();
    Ok(Json(PortfolioMapResp { pins, unplaced }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }
    fn at(s: &str) -> DateTime<Utc> {
        d(s).and_hms_opt(12, 0, 0).unwrap().and_utc()
    }

    #[test]
    fn months_cross_the_year() {
        assert_eq!(
            months_back(d("2026-02-14"), 4),
            vec!["2025-11", "2025-12", "2026-01", "2026-02"]
        );
    }

    #[test]
    fn turn_averages() {
        let p = Uuid::new_v4();
        let t = [
            TurnFact {
                property_id: p,
                started: d("2026-01-01"),
                finished: d("2026-01-11"),
                cost_cents: 100_000,
            },
            TurnFact {
                property_id: p,
                started: d("2026-02-01"),
                finished: d("2026-02-21"),
                cost_cents: 300_000,
            },
        ];
        let s = turn_stats(t.iter());
        assert_eq!(s.count, 2);
        assert_eq!(s.avg_days, Some(15.0));
        assert_eq!(s.avg_cost_cents, Some(200_000));
        assert_eq!(turn_stats(std::iter::empty()).avg_days, None);
    }

    fn ticket(p: Uuid, cat: &str, asset: Option<Uuid>, opened: &str, cost: i64) -> TicketFact {
        TicketFact {
            property_id: p,
            category: cat.into(),
            asset_id: asset,
            opened: at(opened),
            resolved: None,
            resolve_due: None,
            rating: None,
            cost_cents: cost,
        }
    }

    #[test]
    fn sla_and_ratings() {
        let p = Uuid::new_v4();
        let now = at("2026-03-10");
        let mut late = ticket(p, "plumbing", None, "2026-03-01", 0);
        late.resolve_due = Some(at("2026-03-02"));
        late.resolved = Some(at("2026-03-03"));
        late.rating = Some(3);
        let mut on_time = ticket(p, "plumbing", None, "2026-03-01", 0);
        on_time.resolve_due = Some(at("2026-03-05"));
        on_time.resolved = Some(at("2026-03-02"));
        on_time.rating = Some(5);
        let mut overdue_open = ticket(p, "hvac", None, "2026-03-01", 0);
        overdue_open.resolve_due = Some(at("2026-03-04"));
        let s = ticket_stats([late, on_time, overdue_open].iter(), now);
        assert_eq!((s.opened, s.resolved, s.past_sla), (3, 2, 2));
        assert_eq!(s.avg_rating, Some(4.0));
        assert_eq!(s.avg_hours_to_resolve, Some(36.0));
    }

    #[test]
    fn repeats_need_three() {
        let p = Uuid::new_v4();
        let names = HashMap::from([(p, "Birch".to_string())]);
        let t = vec![
            ticket(p, "plumbing", None, "2026-01-01", 100),
            ticket(p, "plumbing", None, "2026-02-01", 200),
            ticket(p, "plumbing", None, "2026-03-01", 300),
            ticket(p, "hvac", None, "2026-03-01", 900),
        ];
        let r = repeat_issues(&t, &names, 3);
        assert_eq!(r.len(), 1);
        assert_eq!(
            (r[0].category.as_str(), r[0].count, r[0].spend_cents),
            ("plumbing", 3, 600)
        );
        assert_eq!(r[0].last_opened, "2026-03-01");
    }

    #[test]
    fn replace_when_repairs_pass_the_share() {
        let p = Uuid::new_v4();
        let names = HashMap::new();
        let now = chrono::Utc::now().into();
        let asset = |price: Option<i64>| entity::asset::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            property_id: p,
            unit_id: None,
            kind: "water_heater".into(),
            name: "Water heater".into(),
            make: None,
            model: None,
            serial_number: None,
            install_date: None,
            warranty_expires: None,
            location: None,
            purchased_on: None,
            purchase_price_cents: price,
            expected_life_years: None,
            warranty_provider: None,
            warranty_notes: None,
            warranty_starts_on: None,
            warranty_policy_number: None,
            warranty_phone: None,
            warranty_coverage: None,
            warranty_transferable: false,
            care_instructions: None,
            manual_url: None,
            recall_checked_on: None,
            notes: None,
            status: "active".into(),
            created_by: None,
            created_at: now,
            updated_at: now,
        };
        let old = asset(Some(100_000));
        let fine = asset(Some(1_000_000));
        let unpriced = asset(None);
        let t = vec![
            ticket(p, "plumbing", Some(old.id), "2026-01-01", 30_000),
            ticket(p, "plumbing", Some(old.id), "2026-02-01", 25_000),
            ticket(p, "plumbing", Some(fine.id), "2026-02-01", 25_000),
            ticket(p, "plumbing", Some(unpriced.id), "2026-02-01", 25_000),
        ];
        let out = appliance_spend(&t, &[fine.clone(), old.clone(), unpriced], &names, 50);
        assert_eq!(out[0].asset_id, old.id);
        assert!(out[0].replace);
        assert_eq!(out[0].share_pct, Some(55));
        assert_eq!(out[0].repairs, 2);
        let f = out.iter().find(|a| a.asset_id == fine.id).unwrap();
        assert!(!f.replace);
        assert!(out.iter().all(|a| a.price_cents.is_some() || !a.replace));
    }
}
