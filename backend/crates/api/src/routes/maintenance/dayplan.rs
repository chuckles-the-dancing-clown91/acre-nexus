//! **Plan the day**: the shopping list by day and store, a proposed route for
//! a technician's day with times worked out from the jobs, and accepting it,
//! which books the visits, settles each work order's parts against stock,
//! and hands the back office what to order and what's running low.

use super::parts::{generate, part_dtos, TicketPartDto};
use crate::appointments as appt;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::{Access, TenantScope};
use crate::workforce::{haversine_m, Fix};
use chrono::{DateTime, Duration, NaiveDate, NaiveTime, TimeZone, Utc};
use entity::prelude::{
    Appointment, InventoryItem, MaintenanceTicket, Property, PropertyDetail, TicketPart,
    TicketTask, User,
};
use rocket::serde::json::Json;
use rocket::{get, post, State};
use schemars::JsonSchema;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Shopping by day and store
// ---------------------------------------------------------------------------

/// One part to buy or pull, with the work order it's for.
#[derive(Serialize, JsonSchema, Clone)]
pub struct ShopItem {
    #[serde(flatten)]
    pub part: TicketPartDto,
    pub ticket_title: String,
    pub property_id: Uuid,
    pub property_name: String,
    /// The day the work order is due (or scheduled), when it has one.
    pub day: Option<String>,
}

#[derive(Serialize, JsonSchema, Clone)]
pub struct StoreGroup {
    /// "Home Depot", "Amazon", a vendor, or "Any store".
    pub store: String,
    pub items: Vec<ShopItem>,
    /// Sum of quantity × unit cost where a cost is known.
    pub est_cents: i64,
}

/// A stock item that runs low once the pulls go through.
#[derive(Serialize, JsonSchema, Clone)]
pub struct LowStock {
    pub inventory_item_id: Uuid,
    pub name: String,
    pub on_hand: i32,
    pub after: i32,
    pub reorder_level: i32,
}

#[derive(Serialize, JsonSchema)]
pub struct DayShopping {
    pub day: String,
    pub stores: Vec<StoreGroup>,
    /// On the shelf: pull these before leaving.
    pub from_stock: Vec<ShopItem>,
}

#[derive(Serialize, JsonSchema)]
pub struct Shopping {
    pub from: String,
    pub to: String,
    pub days: Vec<DayShopping>,
    /// Parts on open work orders with no date yet.
    pub undated: Vec<StoreGroup>,
    pub low_stock: Vec<LowStock>,
    pub total_cents: i64,
}

/// Statuses that still need buying or pulling.
const WANTED: &[&str] = &["needed", "to_order", "from_stock", "pick_up"];

fn store_name(p: &TicketPartDto) -> String {
    p.store
        .clone()
        .or_else(|| p.vendor.clone())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Any store".into())
}

fn group_by_store(items: Vec<ShopItem>) -> Vec<StoreGroup> {
    let mut m: HashMap<String, Vec<ShopItem>> = HashMap::new();
    for i in items {
        m.entry(store_name(&i.part)).or_default().push(i);
    }
    let mut out: Vec<StoreGroup> = m
        .into_iter()
        .map(|(store, items)| StoreGroup {
            est_cents: items
                .iter()
                .map(|i| i.part.unit_cost_cents.unwrap_or(0) * i.part.quantity as i64)
                .sum(),
            store,
            items,
        })
        .collect();
    // Named stores first, "Any store" last, then by name.
    out.sort_by(|a, b| {
        (a.store == "Any store")
            .cmp(&(b.store == "Any store"))
            .then_with(|| a.store.cmp(&b.store))
    });
    out
}

/// Pull from stock when the shelf has it (or the office already said so).
fn pulls_from_stock(p: &TicketPartDto) -> bool {
    p.status == "from_stock" || (p.status == "needed" && p.in_stock.unwrap_or(0) >= p.quantity)
}

async fn tickets_in_reach(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    access: &Access,
    ids: Option<Vec<Uuid>>,
) -> ApiResult<Vec<entity::maintenance_ticket::Model>> {
    let mut q = MaintenanceTicket::find()
        .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
        .filter(entity::maintenance_ticket::Column::Status.is_in(super::OPEN_STATUSES.to_vec()));
    if let Some(p) = access.property_ids() {
        q = q.filter(entity::maintenance_ticket::Column::PropertyId.is_in(p));
    }
    if let Some(ids) = ids {
        q = q.filter(entity::maintenance_ticket::Column::Id.is_in(ids));
    }
    Ok(q.all(db).await?)
}

/// The day each open work order is on: a confirmed visit's day first, else
/// its due date.
async fn day_of_tickets(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    tickets: &[entity::maintenance_ticket::Model],
    tz: &chrono_tz::Tz,
) -> ApiResult<HashMap<Uuid, Option<String>>> {
    let ids: Vec<Uuid> = tickets.iter().map(|t| t.id).collect();
    let visits = Appointment::find()
        .filter(entity::appointment::Column::TenantId.eq(tenant_id))
        .filter(entity::appointment::Column::SubjectType.eq("ticket"))
        .filter(entity::appointment::Column::SubjectId.is_in(ids))
        .filter(entity::appointment::Column::Status.eq("confirmed"))
        .all(db)
        .await?;
    let mut by_visit: HashMap<Uuid, String> = HashMap::new();
    for v in visits {
        if let (Some(tid), Some(s)) = (v.subject_id, v.starts_at) {
            by_visit.insert(tid, s.with_timezone(tz).date_naive().to_string());
        }
    }
    Ok(tickets
        .iter()
        .map(|t| {
            (
                t.id,
                by_visit.get(&t.id).cloned().or_else(|| t.due_date.clone()),
            )
        })
        .collect())
}

async fn low_after(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    pulls: &[ShopItem],
) -> ApiResult<Vec<LowStock>> {
    let mut need: HashMap<Uuid, i32> = HashMap::new();
    for p in pulls {
        if let Some(i) = p.part.inventory_item_id {
            *need.entry(i).or_default() += p.part.quantity;
        }
    }
    if need.is_empty() {
        return Ok(vec![]);
    }
    let items = InventoryItem::find()
        .filter(entity::inventory_item::Column::TenantId.eq(tenant_id))
        .filter(entity::inventory_item::Column::Id.is_in(need.keys().copied().collect::<Vec<_>>()))
        .all(db)
        .await?;
    let mut out: Vec<LowStock> = items
        .into_iter()
        .filter_map(|i| {
            let after = i.quantity - need.get(&i.id).copied().unwrap_or(0);
            (after < i.reorder_level.max(1)).then_some(LowStock {
                inventory_item_id: i.id,
                name: i.name,
                on_hand: i.quantity,
                after,
                reorder_level: i.reorder_level,
            })
        })
        .collect();
    out.sort_by_key(|a| a.after);
    Ok(out)
}

/// Everything to buy or pull for the given work orders, as shop items.
async fn shop_items(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    tickets: &[entity::maintenance_ticket::Model],
    days: &HashMap<Uuid, Option<String>>,
) -> ApiResult<Vec<ShopItem>> {
    let ids: Vec<Uuid> = tickets.iter().map(|t| t.id).collect();
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let parts = part_dtos(
        db,
        tenant_id,
        TicketPart::find()
            .filter(entity::ticket_part::Column::TenantId.eq(tenant_id))
            .filter(entity::ticket_part::Column::TicketId.is_in(ids))
            .filter(entity::ticket_part::Column::Status.is_in(WANTED.to_vec()))
            .order_by_asc(entity::ticket_part::Column::CreatedAt)
            .all(db)
            .await?,
    )
    .await?;
    let props: HashMap<Uuid, String> = Property::find()
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .filter(
            entity::property::Column::Id
                .is_in(tickets.iter().map(|t| t.property_id).collect::<Vec<_>>()),
        )
        .all(db)
        .await?
        .into_iter()
        .map(|p| (p.id, p.name))
        .collect();
    Ok(parts
        .into_iter()
        .filter_map(|p| {
            let t = tickets.iter().find(|t| t.id == p.ticket_id)?;
            Some(ShopItem {
                ticket_title: t.title.clone(),
                property_id: t.property_id,
                property_name: props.get(&t.property_id).cloned().unwrap_or_default(),
                day: days.get(&t.id).cloned().flatten(),
                part: p,
            })
        })
        .collect())
}

/// `GET /shopping?from&to` — what to buy and what to pull, by day and store,
/// for the open work orders in the window (default: today through a week
/// out), plus parts on work orders with no date yet.
#[rocket_okapi::openapi(tag = "Parts")]
#[get("/shopping?<from>&<to>")]
pub async fn shopping(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    from: Option<String>,
    to: Option<String>,
) -> ApiResult<Json<Shopping>> {
    user.require(Permission::MaintenanceRead)?;
    let t = scope.tenant_id;
    let tz = appt::tz_for(&db, t).await;
    let today = Utc::now().with_timezone(&tz).date_naive();
    let from = match from.filter(|d| !d.is_empty()) {
        Some(d) => crate::workforce::parse_date(&d, "from")?,
        None => today,
    };
    let to = match to.filter(|d| !d.is_empty()) {
        Some(d) => crate::workforce::parse_date(&d, "to")?,
        None => from + Duration::days(7),
    };
    if to < from {
        return Err(ApiError::BadRequest("to must not be before from".into()));
    }
    let tickets = tickets_in_reach(&db, t, &access, None).await?;
    let days = day_of_tickets(&db, t, &tickets, &tz).await?;
    let items = shop_items(&db, t, &tickets, &days).await?;
    let mut by_day: HashMap<String, (Vec<ShopItem>, Vec<ShopItem>)> = HashMap::new();
    let mut undated = Vec::new();
    let mut all_pulls = Vec::new();
    for i in items {
        match i.day.as_deref().and_then(|d| d.parse::<NaiveDate>().ok()) {
            Some(d) if d >= from && d <= to => {
                let slot = by_day.entry(d.to_string()).or_default();
                if pulls_from_stock(&i.part) {
                    all_pulls.push(i.clone());
                    slot.1.push(i);
                } else {
                    slot.0.push(i);
                }
            }
            // Overdue work still needs its parts: it lands on the first day.
            Some(d) if d < from => {
                let slot = by_day.entry(from.to_string()).or_default();
                if pulls_from_stock(&i.part) {
                    all_pulls.push(i.clone());
                    slot.1.push(i);
                } else {
                    slot.0.push(i);
                }
            }
            Some(_) => {}
            None => {
                if !pulls_from_stock(&i.part) {
                    undated.push(i);
                }
            }
        }
    }
    let mut days_out: Vec<DayShopping> = by_day
        .into_iter()
        .map(|(day, (buy, pull))| DayShopping {
            day,
            stores: group_by_store(buy),
            from_stock: pull,
        })
        .collect();
    days_out.sort_by(|a, b| a.day.cmp(&b.day));
    let total_cents = days_out
        .iter()
        .flat_map(|d| d.stores.iter())
        .map(|s| s.est_cents)
        .sum();
    Ok(Json(Shopping {
        from: from.to_string(),
        to: to.to_string(),
        days: days_out,
        undated: group_by_store(undated),
        low_stock: low_after(&db, t, &all_pulls).await?,
        total_cents,
    }))
}

// ---------------------------------------------------------------------------
// The route
// ---------------------------------------------------------------------------

#[derive(Deserialize, JsonSchema)]
pub struct ProposeReq {
    /// The day, YYYY-MM-DD (default: tomorrow in the workspace's zone).
    pub date: Option<String>,
    /// Whose day: their work orders and visits, plus unassigned work due that
    /// day. Leave out for everything due that day.
    pub assignee_user_id: Option<Uuid>,
    /// Start of the day as HH:MM in the workspace's zone (default: the
    /// `routes.day_start` setting).
    pub start: Option<String>,
}

/// One stop on the day.
#[derive(Serialize, JsonSchema, Clone)]
pub struct Stop {
    /// `store` for the supply run, else `job`.
    pub kind: String,
    pub ticket_id: Option<Uuid>,
    pub title: String,
    pub property_id: Option<Uuid>,
    pub property_name: String,
    pub address: String,
    pub priority: String,
    pub status: String,
    pub assignee_user_id: Option<Uuid>,
    /// True when it's booked already with the resident; the time is theirs.
    pub fixed: bool,
    /// Minutes on site, from the tasks (or the default).
    pub minutes: i64,
    /// Minutes driving from the previous stop.
    pub drive_minutes: i64,
    pub start: String,
    pub end: String,
    /// "9:00 AM to 10:30 AM".
    pub when_words: String,
    pub tasks_total: i64,
    pub tasks_done: i64,
    pub to_buy: i64,
    pub from_stock: i64,
    pub with_name: Option<String>,
    pub with_phone: Option<String>,
    pub access_notes: Option<String>,
    /// Why it couldn't be placed (only on `unplaced`).
    pub note: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct Route {
    pub date: String,
    pub assignee_user_id: Option<Uuid>,
    pub assignee_name: Option<String>,
    pub day_start: String,
    pub day_end: String,
    pub stops: Vec<Stop>,
    /// Work that didn't fit in the day, in the order it would come next.
    pub unplaced: Vec<Stop>,
    pub total_minutes: i64,
    pub drive_minutes: i64,
    /// What to buy before leaving, by store.
    pub stores: Vec<StoreGroup>,
    /// What to pull from the shelf before leaving.
    pub from_stock: Vec<ShopItem>,
    pub low_stock: Vec<LowStock>,
}

/// Booked visits by work order: start, end, who's going.
type FixedVisits = HashMap<Uuid, (DateTime<Utc>, DateTime<Utc>, Option<Uuid>)>;

struct Candidate {
    t: entity::maintenance_ticket::Model,
    fix: Option<Fix>,
    minutes: i64,
    fixed_at: Option<(DateTime<Utc>, DateTime<Utc>)>,
    tasks_total: i64,
    tasks_done: i64,
    to_buy: i64,
    from_stock: i64,
    with_name: Option<String>,
    with_phone: Option<String>,
    property_name: String,
    address: String,
}

fn drive(a: Option<Fix>, b: Option<Fix>, default: i64) -> i64 {
    match (a, b) {
        (Some(a), Some(b)) => {
            let km = haversine_m(a, b) / 1000.0;
            // City driving: about 35 km/h door to door, plus parking.
            ((km / 35.0 * 60.0).round() as i64 + 5).max(5)
        }
        _ => default,
    }
}

fn hhmm(t: DateTime<Utc>, tz: &chrono_tz::Tz) -> String {
    t.with_timezone(tz).format("%-I:%M %p").to_string()
}

fn prio(p: &str) -> u8 {
    match p {
        "urgent" => 0,
        "high" => 1,
        "normal" | "medium" => 2,
        _ => 3,
    }
}

struct Settings {
    day_start: NaiveTime,
    day_minutes: i64,
    drive_default: i64,
    store_minutes: i64,
    job_default: i64,
}

async fn settings(db: &impl ConnectionTrait, tenant_id: Uuid) -> Settings {
    let start = crate::settings::get_string(db, tenant_id, crate::settings::ROUTES_DAY_START).await;
    Settings {
        day_start: NaiveTime::parse_from_str(start.trim(), "%H:%M")
            .unwrap_or_else(|_| NaiveTime::from_hms_opt(8, 0, 0).unwrap()),
        day_minutes: crate::settings::get_i64(db, tenant_id, crate::settings::ROUTES_DAY_MINUTES)
            .await
            .clamp(60, 16 * 60),
        drive_default: crate::settings::get_i64(
            db,
            tenant_id,
            crate::settings::ROUTES_DRIVE_MINUTES,
        )
        .await
        .clamp(0, 180),
        store_minutes: crate::settings::get_i64(
            db,
            tenant_id,
            crate::settings::ROUTES_STORE_MINUTES,
        )
        .await
        .clamp(0, 180),
        job_default: crate::settings::get_i64(db, tenant_id, crate::settings::ROUTES_JOB_MINUTES)
            .await
            .clamp(15, 8 * 60),
    }
}

/// The work for a day: due that day (or overdue and still open), or with a
/// confirmed visit that day; for one person when asked, with unassigned work
/// included so it can be handed out.
async fn candidates(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    access: &Access,
    day: NaiveDate,
    who: Option<Uuid>,
    tz: &chrono_tz::Tz,
    s: &Settings,
) -> ApiResult<Vec<Candidate>> {
    let open = tickets_in_reach(db, tenant_id, access, None).await?;
    let ids: Vec<Uuid> = open.iter().map(|t| t.id).collect();
    let visits = Appointment::find()
        .filter(entity::appointment::Column::TenantId.eq(tenant_id))
        .filter(entity::appointment::Column::SubjectType.eq("ticket"))
        .filter(entity::appointment::Column::SubjectId.is_in(ids.clone()))
        .filter(entity::appointment::Column::Status.eq("confirmed"))
        .all(db)
        .await?;
    let mut fixed: FixedVisits = HashMap::new();
    for v in visits {
        if let (Some(tid), Some(st), Some(en)) = (v.subject_id, v.starts_at, v.ends_at) {
            let st = st.with_timezone(&Utc);
            if st.with_timezone(tz).date_naive() == day {
                fixed.insert(tid, (st, en.with_timezone(&Utc), v.assignee_user_id));
            }
        }
    }
    let picked: Vec<entity::maintenance_ticket::Model> = open
        .into_iter()
        .filter(|t| {
            let on_day = fixed.contains_key(&t.id)
                || t.due_date
                    .as_deref()
                    .and_then(|d| d.parse::<NaiveDate>().ok())
                    .is_some_and(|d| d <= day);
            if !on_day {
                return false;
            }
            match who {
                None => true,
                Some(u) => {
                    t.assignee_user_id == Some(u)
                        || fixed.get(&t.id).is_some_and(|f| f.2 == Some(u))
                        || (t.assignee_user_id.is_none() && t.assignee_entity_id.is_none())
                }
            }
        })
        .collect();
    if picked.is_empty() {
        return Ok(vec![]);
    }
    let pids: Vec<Uuid> = picked.iter().map(|t| t.property_id).collect();
    let props: HashMap<Uuid, entity::property::Model> = Property::find()
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .filter(entity::property::Column::Id.is_in(pids.clone()))
        .all(db)
        .await?
        .into_iter()
        .map(|p| (p.id, p))
        .collect();
    let fixes: HashMap<Uuid, Fix> = PropertyDetail::find()
        .filter(entity::property_detail::Column::TenantId.eq(tenant_id))
        .filter(entity::property_detail::Column::PropertyId.is_in(pids))
        .all(db)
        .await?
        .into_iter()
        .filter_map(|d| {
            Some((
                d.property_id,
                Fix {
                    lat: d.latitude?,
                    lng: d.longitude?,
                },
            ))
        })
        .collect();
    let tids: Vec<Uuid> = picked.iter().map(|t| t.id).collect();
    let tasks = TicketTask::find()
        .filter(entity::ticket_task::Column::TenantId.eq(tenant_id))
        .filter(entity::ticket_task::Column::TicketId.is_in(tids.clone()))
        .all(db)
        .await?;
    let parts = part_dtos(
        db,
        tenant_id,
        TicketPart::find()
            .filter(entity::ticket_part::Column::TenantId.eq(tenant_id))
            .filter(entity::ticket_part::Column::TicketId.is_in(tids))
            .filter(entity::ticket_part::Column::Status.is_in(WANTED.to_vec()))
            .all(db)
            .await?,
    )
    .await?;
    let mut out = Vec::new();
    for t in picked {
        let mine: Vec<_> = tasks.iter().filter(|x| x.ticket_id == t.id).collect();
        let left: i64 = mine
            .iter()
            .filter(|x| x.status != "done" && x.status != "skipped")
            .map(|x| x.est_minutes.unwrap_or(0) as i64)
            .sum();
        let minutes = if left > 0 { left } else { s.job_default };
        let lease = appt::resident_for_ticket(db, tenant_id, &t).await;
        let p = props.get(&t.property_id);
        let my_parts: Vec<_> = parts.iter().filter(|x| x.ticket_id == t.id).collect();
        out.push(Candidate {
            fix: fixes.get(&t.property_id).copied(),
            minutes,
            fixed_at: fixed.get(&t.id).map(|f| (f.0, f.1)),
            tasks_total: mine.len() as i64,
            tasks_done: mine.iter().filter(|x| x.status == "done").count() as i64,
            to_buy: my_parts.iter().filter(|x| !pulls_from_stock(x)).count() as i64,
            from_stock: my_parts.iter().filter(|x| pulls_from_stock(x)).count() as i64,
            with_name: lease.as_ref().map(|l| l.tenant_name.clone()),
            with_phone: lease.as_ref().and_then(|l| l.tenant_phone.clone()),
            property_name: p.map(|p| p.name.clone()).unwrap_or_default(),
            address: p.map(crate::geo::full_address).unwrap_or_default(),
            t,
        });
    }
    Ok(out)
}

fn stop_of(
    c: &Candidate,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    drive: i64,
    tz: &chrono_tz::Tz,
) -> Stop {
    Stop {
        kind: "job".into(),
        ticket_id: Some(c.t.id),
        title: c.t.title.clone(),
        property_id: Some(c.t.property_id),
        property_name: c.property_name.clone(),
        address: c.address.clone(),
        priority: c.t.priority.clone(),
        status: c.t.status.clone(),
        assignee_user_id: c.t.assignee_user_id,
        fixed: c.fixed_at.is_some(),
        minutes: (end - start).num_minutes(),
        drive_minutes: drive,
        start: start.to_rfc3339(),
        end: end.to_rfc3339(),
        when_words: format!("{} to {}", hhmm(start, tz), hhmm(end, tz)),
        tasks_total: c.tasks_total,
        tasks_done: c.tasks_done,
        to_buy: c.to_buy,
        from_stock: c.from_stock,
        with_name: c.with_name.clone(),
        with_phone: c.with_phone.clone(),
        access_notes: c.t.access_notes.clone(),
        note: None,
    }
}

/// Lay the day out: booked visits keep their times; the rest go nearest
/// first between them, urgent work ahead of the queue when distance ties.
fn lay_out(
    mut cands: Vec<Candidate>,
    day_start: DateTime<Utc>,
    day_minutes: i64,
    store_minutes: Option<i64>,
    drive_default: i64,
    tz: &chrono_tz::Tz,
) -> (Vec<Stop>, Vec<Stop>, i64) {
    let day_end = day_start + Duration::minutes(day_minutes);
    let mut stops: Vec<Stop> = Vec::new();
    let mut cursor = day_start;
    let mut here: Option<Fix> = None;
    let mut driven = 0i64;
    if let Some(m) = store_minutes {
        let end = cursor + Duration::minutes(m);
        stops.push(Stop {
            kind: "store".into(),
            ticket_id: None,
            title: "Supply run".into(),
            property_id: None,
            property_name: String::new(),
            address: String::new(),
            priority: "normal".into(),
            status: String::new(),
            assignee_user_id: None,
            fixed: false,
            minutes: m,
            drive_minutes: 0,
            start: cursor.to_rfc3339(),
            end: end.to_rfc3339(),
            when_words: format!("{} to {}", hhmm(cursor, tz), hhmm(end, tz)),
            tasks_total: 0,
            tasks_done: 0,
            to_buy: 0,
            from_stock: 0,
            with_name: None,
            with_phone: None,
            access_notes: None,
            note: None,
        });
        cursor = end;
    }
    let mut fixed: Vec<Candidate> = Vec::new();
    let mut free: Vec<Candidate> = Vec::new();
    for c in cands.drain(..) {
        if c.fixed_at.is_some() {
            fixed.push(c);
        } else {
            free.push(c);
        }
    }
    fixed.sort_by_key(|c| c.fixed_at.map(|f| f.0));
    let mut unplaced: Vec<Stop> = Vec::new();
    // Where the day starts isn't known (the office, home); until the first
    // stop, order by distance from the first booked visit, else from the
    // most urgent job, so the morning clusters sensibly.
    let anchor: Option<Fix> = fixed.first().and_then(|c| c.fix).or_else(|| {
        free.iter()
            .min_by(|a, b| {
                prio(&a.t.priority)
                    .cmp(&prio(&b.t.priority))
                    .then_with(|| a.t.created_at.cmp(&b.t.created_at))
            })
            .and_then(|c| c.fix)
    });
    loop {
        let next_fixed_start = fixed.first().and_then(|c| c.fixed_at).map(|f| f.0);
        let from = here.or(anchor);
        // Which free job comes next: nearest, then most urgent, then oldest.
        let pick = free
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                drive(from, a.fix, drive_default)
                    .cmp(&drive(from, b.fix, drive_default))
                    .then_with(|| prio(&a.t.priority).cmp(&prio(&b.t.priority)))
                    .then_with(|| a.t.created_at.cmp(&b.t.created_at))
            })
            .map(|(i, _)| i);
        let fits_before_fixed = |c: &Candidate| -> bool {
            let d = drive(here, c.fix, drive_default);
            let end = cursor + Duration::minutes(d + c.minutes);
            match (next_fixed_start, fixed.first()) {
                (Some(fs), Some(f)) => {
                    end + Duration::minutes(drive(c.fix, f.fix, drive_default)) <= fs
                }
                _ => true,
            }
        };
        match pick {
            Some(i) if fits_before_fixed(&free[i]) => {
                let c = free.remove(i);
                let d = drive(here, c.fix, drive_default);
                let start = cursor + Duration::minutes(d);
                let end = start + Duration::minutes(c.minutes);
                if end > day_end {
                    let mut s = stop_of(&c, start, end, d, tz);
                    s.note = Some("Doesn't fit in the day".into());
                    unplaced.push(s);
                    continue;
                }
                driven += d;
                stops.push(stop_of(&c, start, end, d, tz));
                cursor = end;
                here = c.fix.or(here);
            }
            _ => {
                if fixed.is_empty() {
                    // Nothing fixed left and the free one didn't fit: the
                    // rest spills over.
                    for c in free.drain(..) {
                        let d = drive(here, c.fix, drive_default);
                        let start = cursor + Duration::minutes(d);
                        let end = start + Duration::minutes(c.minutes);
                        let mut s = stop_of(&c, start, end, d, tz);
                        s.note = Some("Doesn't fit in the day".into());
                        unplaced.push(s);
                    }
                    break;
                }
                let c = fixed.remove(0);
                let (fs, fe) = c.fixed_at.unwrap();
                let d = drive(here, c.fix, drive_default);
                driven += d;
                let mut s = stop_of(&c, fs, fe, d, tz);
                if cursor + Duration::minutes(d) > fs {
                    s.note = Some(format!(
                        "Tight: arriving about {} late",
                        (cursor + Duration::minutes(d) - fs).num_minutes()
                    ));
                }
                let end = fe.max(cursor + Duration::minutes(d));
                s.end = end.to_rfc3339();
                s.when_words = format!("{} to {}", hhmm(fs, tz), hhmm(end, tz));
                stops.push(s);
                cursor = end;
                here = c.fix.or(here);
            }
        }
        if free.is_empty() && fixed.is_empty() {
            break;
        }
    }
    (stops, unplaced, driven)
}

async fn build_route(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    access: &Access,
    day: NaiveDate,
    who: Option<Uuid>,
    start_hhmm: Option<String>,
) -> ApiResult<Route> {
    let tz = appt::tz_for(db, tenant_id).await;
    let s = settings(db, tenant_id).await;
    let start_time = match start_hhmm
        .as_deref()
        .map(str::trim)
        .filter(|x| !x.is_empty())
    {
        Some(x) => NaiveTime::parse_from_str(x, "%H:%M")
            .map_err(|_| ApiError::BadRequest("start must be HH:MM".into()))?,
        None => s.day_start,
    };
    let day_start = tz
        .from_local_datetime(&day.and_time(start_time))
        .earliest()
        .map(|t| t.to_utc())
        .ok_or_else(|| ApiError::BadRequest("that time doesn't exist".into()))?;
    let cands = candidates(db, tenant_id, access, day, who, &tz, &s).await?;
    let tickets: Vec<_> = cands.iter().map(|c| c.t.clone()).collect();
    let days: HashMap<Uuid, Option<String>> = tickets
        .iter()
        .map(|t| (t.id, Some(day.to_string())))
        .collect();
    let items = shop_items(db, tenant_id, &tickets, &days).await?;
    let (pull, buy): (Vec<ShopItem>, Vec<ShopItem>) =
        items.into_iter().partition(|i| pulls_from_stock(&i.part));
    let store_minutes = (!buy.is_empty()).then_some(s.store_minutes);
    let (stops, unplaced, driven) = lay_out(
        cands,
        day_start,
        s.day_minutes,
        store_minutes,
        s.drive_default,
        &tz,
    );
    let assignee_name = match who {
        Some(u) => User::find_by_id(u).one(db).await?.map(|u| u.name),
        None => None,
    };
    let total_minutes = stops.iter().map(|s| s.minutes + s.drive_minutes).sum();
    Ok(Route {
        date: day.to_string(),
        assignee_user_id: who,
        assignee_name,
        day_start: day_start.to_rfc3339(),
        day_end: (day_start + Duration::minutes(s.day_minutes)).to_rfc3339(),
        stops,
        unplaced,
        total_minutes,
        drive_minutes: driven,
        stores: group_by_store(buy),
        low_stock: low_after(db, tenant_id, &pull).await?,
        from_stock: pull,
    })
}

/// `POST /routes/propose` — lay out a day: the jobs due, how long each takes
/// from its tasks, driving between them, booked visits at their times, and
/// the supply run first when there's anything to buy.
#[rocket_okapi::openapi(tag = "Maintenance")]
#[post("/routes/propose", data = "<body>")]
pub async fn propose(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    body: Json<ProposeReq>,
) -> ApiResult<Json<Route>> {
    user.require(Permission::MaintenanceRead)?;
    let b = body.into_inner();
    let tz = appt::tz_for(&db, scope.tenant_id).await;
    let day = match b.date.filter(|d| !d.is_empty()) {
        Some(d) => crate::workforce::parse_date(&d, "date")?,
        None => Utc::now().with_timezone(&tz).date_naive() + Duration::days(1),
    };
    Ok(Json(
        build_route(
            &db,
            scope.tenant_id,
            &access,
            day,
            b.assignee_user_id,
            b.start,
        )
        .await?,
    ))
}

#[derive(Deserialize, JsonSchema)]
pub struct AcceptStop {
    pub ticket_id: Uuid,
    /// RFC 3339, or `YYYY-MM-DDTHH:MM` in the workspace's zone.
    pub start: String,
    pub end: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct AcceptReq {
    pub date: String,
    /// Who takes the day. Unassigned work orders on the route go to them.
    pub assignee_user_id: Uuid,
    pub stops: Vec<AcceptStop>,
}

#[derive(Serialize, JsonSchema)]
pub struct Accepted {
    pub date: String,
    pub assignee_name: String,
    pub booked: usize,
    /// Visits that were already confirmed at that time and were left alone.
    pub kept: usize,
    /// What the back office now has to order, by store.
    pub to_order: Vec<StoreGroup>,
    pub from_stock: Vec<ShopItem>,
    pub low_stock: Vec<LowStock>,
}

/// `POST /routes/accept` — book the route: each stop becomes a confirmed
/// visit with the person on it (the resident hears the time), unassigned
/// work goes to them, each work order's parts are settled against stock,
/// parts to buy get a need-by of the day before, and the office hears what
/// to order and what's running low.
#[rocket_okapi::openapi(tag = "Maintenance")]
#[post("/routes/accept", data = "<body>")]
pub async fn accept(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    body: Json<AcceptReq>,
) -> ApiResult<Json<Accepted>> {
    user.require(Permission::MaintenanceManage)?;
    let b = body.into_inner();
    let t = scope.tenant_id;
    let day = crate::workforce::parse_date(&b.date, "date")?;
    if b.stops.is_empty() {
        return Err(ApiError::BadRequest("the route has no stops".into()));
    }
    let who = User::find_by_id(b.assignee_user_id)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("person not found".into()))?;
    let tz = appt::tz_for(&db, t).await;
    let ids: Vec<Uuid> = b.stops.iter().map(|s| s.ticket_id).collect();
    let tickets = tickets_in_reach(&db, t, &access, Some(ids.clone())).await?;
    if tickets.len() != {
        let mut u = ids.clone();
        u.sort();
        u.dedup();
        u.len()
    } {
        return Err(ApiError::NotFound(
            "a work order on the route isn't open or isn't yours to book".into(),
        ));
    }
    let existing = Appointment::find()
        .filter(entity::appointment::Column::TenantId.eq(t))
        .filter(entity::appointment::Column::SubjectType.eq("ticket"))
        .filter(entity::appointment::Column::SubjectId.is_in(ids))
        .filter(entity::appointment::Column::Status.eq("confirmed"))
        .all(&db)
        .await?;
    let now = Utc::now();
    let mut booked = 0usize;
    let mut kept = 0usize;
    let mut settled: Vec<entity::maintenance_ticket::Model> = Vec::new();
    for s in &b.stops {
        let Some(tk) = tickets.iter().find(|x| x.id == s.ticket_id) else {
            continue;
        };
        let w = appt::Window {
            start: crate::routes::appointments::parse_when(&s.start, &tz)?,
            end: crate::routes::appointments::parse_when(&s.end, &tz)?,
        };
        if w.end <= w.start {
            return Err(ApiError::BadRequest(format!(
                "{}: the end is before the start",
                tk.title
            )));
        }
        // The work order goes to the person when nobody has it.
        if tk.assignee_user_id.is_none() && tk.assignee_entity_id.is_none() {
            let mut am: entity::maintenance_ticket::ActiveModel = tk.clone().into();
            am.assignee_user_id = Set(Some(who.id));
            am.updated_at = Set(now.into());
            am.update(&db).await?;
        }
        let same = existing.iter().find(|a| {
            a.subject_id == Some(tk.id)
                && a.starts_at.map(|x| x.with_timezone(&Utc)) == Some(w.start)
                && a.assignee_user_id == Some(who.id)
        });
        if same.is_some() {
            kept += 1;
        } else {
            let lease = appt::resident_for_ticket(&db, t, tk).await;
            // Any open offer or booking on the work order gives way.
            for a in Appointment::find()
                .filter(entity::appointment::Column::TenantId.eq(t))
                .filter(entity::appointment::Column::SubjectType.eq("ticket"))
                .filter(entity::appointment::Column::SubjectId.eq(tk.id))
                .filter(entity::appointment::Column::Status.is_in(["proposed", "confirmed"]))
                .all(&db)
                .await?
            {
                let mut am: entity::appointment::ActiveModel = a.into();
                am.status = Set("cancelled".into());
                am.outcome_note = Set(Some("Rebooked on the day's route".into()));
                am.updated_at = Set(now.into());
                am.update(&db).await?;
            }
            let a = entity::appointment::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(t),
                property_id: Set(tk.property_id),
                unit_id: Set(tk.unit_id),
                kind: Set("repair".into()),
                subject_type: Set("ticket".into()),
                subject_id: Set(Some(tk.id)),
                title: Set(tk.title.clone()),
                status: Set("proposed".into()),
                windows: Set(json!([w])),
                starts_at: Set(None),
                ends_at: Set(None),
                with_name: Set(lease.as_ref().map(|l| l.tenant_name.clone())),
                with_email: Set(lease
                    .as_ref()
                    .and_then(|l| l.tenant_email.clone())
                    .map(|e| e.trim().to_lowercase())),
                with_phone: Set(lease.as_ref().and_then(|l| l.tenant_phone.clone())),
                with_role: Set("resident".into()),
                assignee_user_id: Set(Some(who.id)),
                vendor_entity_id: Set(None),
                note: Set(Some("Booked on the day's route".into())),
                access_notes: Set(tk.access_notes.clone()),
                token_hash: Set(None),
                confirmed_by: Set(None),
                confirmed_at: Set(None),
                proposed_start: Set(None),
                proposed_end: Set(None),
                reminded: Set(json!([])),
                outcome_note: Set(None),
                created_by: Set(Some(user.user_id)),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }
            .insert(&db)
            .await?;
            appt::confirm(&db, t, a, w, "staff").await?;
            booked += 1;
        }
        // Settle the parts: on the shelf or to buy, with the day before as
        // the need-by.
        generate(&db, t, tk).await?;
        let need_by = (day - Duration::days(1)).to_string();
        for p in TicketPart::find()
            .filter(entity::ticket_part::Column::TenantId.eq(t))
            .filter(entity::ticket_part::Column::TicketId.eq(tk.id))
            .filter(entity::ticket_part::Column::Status.eq("to_order"))
            .filter(entity::ticket_part::Column::NeedBy.is_null())
            .all(&db)
            .await?
        {
            let mut am: entity::ticket_part::ActiveModel = p.into();
            am.need_by = Set(Some(need_by.clone()));
            am.updated_at = Set(now.into());
            am.update(&db).await?;
        }
        settled.push(tk.clone());
    }
    let days: HashMap<Uuid, Option<String>> = settled
        .iter()
        .map(|t| (t.id, Some(day.to_string())))
        .collect();
    let items = shop_items(&db, t, &settled, &days).await?;
    let (pull, buy): (Vec<ShopItem>, Vec<ShopItem>) =
        items.into_iter().partition(|i| pulls_from_stock(&i.part));
    let to_order = group_by_store(buy);
    let low_stock = low_after(&db, t, &pull).await?;

    // The office hears what to order and what's running low; the person
    // hears their day is set.
    let when = day.format("%a, %b %-d").to_string();
    let lines = |g: &[StoreGroup]| {
        g.iter()
            .flat_map(|s| {
                s.items.iter().map(move |i| {
                    format!(
                        "- {} × {} ({}) for {} at {}",
                        i.part.quantity, i.part.name, s.store, i.ticket_title, i.property_name
                    )
                })
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let pulls = pull
        .iter()
        .map(|i| {
            format!(
                "- {} × {} for {}",
                i.part.quantity, i.part.name, i.ticket_title
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let lows = low_stock
        .iter()
        .map(|l| {
            format!(
                "- {}: {} on hand, {} after this day (reorder at {})",
                l.name, l.on_hand, l.after, l.reorder_level
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let link = format!(
        "{}/console/maintenance/plan?date={}&who={}",
        crate::oauth::public_app_url(),
        day,
        who.id
    );
    if !to_order.is_empty() || !low_stock.is_empty() {
        crate::notify::notify_staff(
            &db,
            t,
            "maintenance:manage",
            "route_parts_needed",
            json!({
                "assignee": who.name,
                "when": when,
                "stops": b.stops.len(),
                "to_order": if to_order.is_empty() { "Nothing to order.".to_string() } else { format!("To order:\n{}", lines(&to_order)) },
                "from_stock": if pulls.is_empty() { String::new() } else { format!("\n\nPull from stock:\n{pulls}") },
                "low": if lows.is_empty() { String::new() } else { format!("\n\nRunning low:\n{lows}") },
                "link": link,
            }),
            None,
            &format!("route_parts:{day}:{}:{}", who.id, now.timestamp()),
            None,
        )
        .await;
    }
    if who.id != user.user_id {
        crate::notify::in_app(
            &db,
            t,
            &who,
            "route_assigned",
            &json!({
                "when": when,
                "stops": b.stops.len(),
                "by": user_name(&db, user.user_id).await,
                "link": format!("{}/console/my-day", crate::oauth::public_app_url()),
            }),
            None,
            &format!("route_assigned:{day}:{}:{}", who.id, now.timestamp()),
        )
        .await;
    }
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::ROUTE_ACCEPTED,
        Some("user"),
        Some(who.id.to_string()),
        Some(t),
        Some(json!({ "date": day.to_string(), "stops": b.stops.len(), "booked": booked, "to_order": to_order.iter().map(|s| s.items.len()).sum::<usize>() })),
    )
    .await;
    Ok(Json(Accepted {
        date: day.to_string(),
        assignee_name: who.name,
        booked,
        kept,
        to_order,
        from_stock: pull,
        low_stock,
    }))
}

async fn user_name(db: &impl ConnectionTrait, id: Uuid) -> String {
    User::find_by_id(id)
        .one(db)
        .await
        .ok()
        .flatten()
        .map(|u| u.name)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(
        title: &str,
        lat: f64,
        minutes: i64,
        fixed: Option<(i64, i64)>,
        base: DateTime<Utc>,
    ) -> Candidate {
        let now = Utc::now();
        Candidate {
            t: entity::maintenance_ticket::Model {
                id: Uuid::new_v4(),
                tenant_id: Uuid::nil(),
                property_id: Uuid::new_v4(),
                unit_id: None,
                lease_id: None,
                title: title.into(),
                description: None,
                category: "general".into(),
                priority: "normal".into(),
                status: "open".into(),
                assignee_user_id: None,
                assignee_entity_id: None,
                reporter: None,
                location: None,
                access_notes: None,
                permission_to_enter: false,
                asset_id: None,
                waiting_on: None,
                follow_up_date: None,
                rating: None,
                review_comment: None,
                due_date: None,
                reviewed_at: None,
                cost_cents: None,
                first_response_at: None,
                resolved_at: None,
                sla_response_due_at: None,
                sla_resolve_due_at: None,
                partner_counterparty_id: None,
                partner_job_id: None,
                partner_status: None,
                partner_synced_at: None,
                created_at: now.into(),
                updated_at: now.into(),
                track_time: true,
            },
            fix: Some(Fix { lat, lng: -122.6 }),
            minutes,
            fixed_at: fixed
                .map(|(a, b)| (base + Duration::minutes(a), base + Duration::minutes(b))),
            tasks_total: 0,
            tasks_done: 0,
            to_buy: 0,
            from_stock: 0,
            with_name: None,
            with_phone: None,
            property_name: String::new(),
            address: String::new(),
        }
    }

    #[test]
    fn nearest_first_around_a_booked_visit() {
        let tz: chrono_tz::Tz = "America/Los_Angeles".parse().unwrap();
        let base = Utc.with_ymd_and_hms(2026, 10, 6, 15, 0, 0).unwrap(); // 8 AM Pacific
                                                                         // Far job (north), near job, and a visit booked 10:00 to 11:00.
        let cands = vec![
            cand("Far", 45.60, 60, None, base),
            cand("Near", 45.50, 60, None, base),
            cand("Booked", 45.52, 60, Some((120, 180)), base),
        ];
        let (stops, unplaced, _) = lay_out(cands, base, 480, Some(30), 20, &tz);
        let titles: Vec<&str> = stops.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, vec!["Supply run", "Near", "Booked", "Far"]);
        assert!(unplaced.is_empty());
        assert!(stops[2].fixed);
        assert_eq!(stops[2].when_words, "10:00 AM to 11:00 AM");
        // Near: 8:30 start, 20 min default drive (no "here" yet), 60 on site.
        assert_eq!(stops[1].when_words, "8:50 AM to 9:50 AM");
    }

    #[test]
    fn overflow_is_unplaced() {
        let tz: chrono_tz::Tz = "America/Los_Angeles".parse().unwrap();
        let base = Utc.with_ymd_and_hms(2026, 10, 6, 15, 0, 0).unwrap();
        let cands = vec![
            cand("A", 45.50, 240, None, base),
            cand("B", 45.51, 240, None, base),
            cand("C", 45.52, 60, None, base),
        ];
        let (stops, unplaced, _) = lay_out(cands, base, 480, None, 20, &tz);
        // A fills the morning; B would run past 4 PM, so the short C takes
        // the afternoon and B spills over.
        let titles: Vec<&str> = stops.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, vec!["A", "C"]);
        assert_eq!(unplaced.len(), 1);
        assert_eq!(unplaced[0].title, "B");
    }

    #[test]
    fn stores_group_named_first() {
        let g = group_by_store(vec![]);
        assert!(g.is_empty());
    }
}
