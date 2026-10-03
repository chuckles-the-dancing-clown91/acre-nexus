//! **Queues**: who on the team can take work and how much they have, and a
//! person's own queue of tasks across work orders.

use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::tenancy::{Access, TenantScope};
use entity::prelude::{
    Assignment, Counterparty, MaintenanceTicket, Membership, Property, TicketTask, User,
};
use rocket::get;
use rocket::serde::json::Json;
use schemars::JsonSchema;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::Serialize;
use std::collections::{BTreeSet, HashMap};
use uuid::Uuid;

/// Statuses a work order is still being worked in.
pub const OPEN: &[&str] = &["open", "triage", "scheduled", "in_progress", "on_hold"];

#[derive(Serialize, JsonSchema, Debug, PartialEq)]
pub struct TechDto {
    pub user_id: Uuid,
    pub name: String,
    pub email: String,
    /// Their role here (`maintenance`, `property_manager`, …).
    pub role: String,
    pub title: Option<String>,
    /// Work orders they have that are still open.
    pub open_tickets: i64,
    /// Tasks of their own still to do.
    pub open_tasks: i64,
    /// Assigned to the property asked about (or company-wide, which can work
    /// anywhere).
    pub on_property: bool,
}

/// Fewest open items first; people on the property before those who aren't.
pub fn order_techs(techs: &mut [TechDto]) {
    techs.sort_by(|a, b| {
        (!a.on_property, a.open_tickets + a.open_tasks, &a.name).cmp(&(
            !b.on_property,
            b.open_tickets + b.open_tasks,
            &b.name,
        ))
    });
}

/// `GET /ticket-techs?<property_id>` — the team members who can be given
/// work, with what each already has. Those assigned to the property (or with
/// company-wide reach) come first, then the lightest load.
#[rocket_okapi::openapi(tag = "Service Desk")]
#[get("/ticket-techs?<property_id>")]
pub async fn list_techs(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    property_id: Option<String>,
) -> ApiResult<Json<Vec<TechDto>>> {
    user.require(Permission::MaintenanceRead)?;
    let property = property_id
        .filter(|s| !s.is_empty())
        .and_then(|s| Uuid::parse_str(&s).ok());
    if let Some(p) = property {
        if !access.sees(p) {
            return Ok(Json(Vec::new()));
        }
    }
    let members = Membership::find()
        .filter(entity::membership::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::membership::Column::Status.eq("active"))
        .filter(entity::membership::Column::ProfileType.ne("renter"))
        .filter(entity::membership::Column::ProfileType.ne("landlord"))
        .all(&db)
        .await?;
    let ids: Vec<Uuid> = members.iter().map(|m| m.user_id).collect();
    let users: HashMap<Uuid, entity::user::Model> = User::find()
        .filter(entity::user::Column::Id.is_in(ids.clone()))
        .all(&db)
        .await?
        .into_iter()
        .map(|u| (u.id, u))
        .collect();
    let assigned_here: BTreeSet<Uuid> = match property {
        Some(p) => Assignment::find()
            .filter(entity::assignment::Column::TenantId.eq(scope.tenant_id))
            .filter(entity::assignment::Column::SubjectType.eq("property"))
            .filter(entity::assignment::Column::SubjectId.eq(p))
            .all(&db)
            .await?
            .into_iter()
            .map(|a| a.user_id)
            .collect(),
        None => BTreeSet::new(),
    };
    let tickets = MaintenanceTicket::find()
        .filter(entity::maintenance_ticket::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::maintenance_ticket::Column::Status.is_in(OPEN.to_vec()))
        .filter(entity::maintenance_ticket::Column::AssigneeUserId.is_in(ids.clone()))
        .all(&db)
        .await?;
    let mut open_tickets: HashMap<Uuid, i64> = HashMap::new();
    for t in &tickets {
        if let Some(u) = t.assignee_user_id {
            *open_tickets.entry(u).or_default() += 1;
        }
    }
    let tasks = TicketTask::find()
        .filter(entity::ticket_task::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::ticket_task::Column::AssigneeUserId.is_in(ids))
        .filter(entity::ticket_task::Column::Status.is_in(["todo", "doing"]))
        .all(&db)
        .await?;
    let mut open_tasks: HashMap<Uuid, i64> = HashMap::new();
    for t in &tasks {
        if let Some(u) = t.assignee_user_id {
            *open_tasks.entry(u).or_default() += 1;
        }
    }
    // Company-wide roles can work anywhere; field roles only where assigned.
    let company_wide = |role: &str| !crate::tenancy::SCOPED_PERSONAS.contains(&role);
    let mut seen = BTreeSet::new();
    let mut out: Vec<TechDto> = Vec::new();
    for m in members {
        let Some(u) = users.get(&m.user_id) else {
            continue;
        };
        if !seen.insert(u.id) {
            continue;
        }
        // A field person who isn't assigned to the property isn't offered.
        let on_property =
            property.is_none() || company_wide(&m.profile_type) || assigned_here.contains(&u.id);
        if property.is_some() && !on_property {
            continue;
        }
        out.push(TechDto {
            user_id: u.id,
            name: u.name.clone(),
            email: u.email.clone(),
            role: m.profile_type,
            title: m.title,
            open_tickets: open_tickets.get(&u.id).copied().unwrap_or(0),
            open_tasks: open_tasks.get(&u.id).copied().unwrap_or(0),
            on_property: property.is_some() && assigned_here.contains(&u.id),
        });
    }
    order_techs(&mut out);
    Ok(Json(out))
}

#[derive(Serialize, JsonSchema)]
pub struct QueueTask {
    pub task_id: Uuid,
    pub ticket_id: Uuid,
    pub title: String,
    pub trade: String,
    pub est_minutes: Option<i32>,
    /// `todo` | `doing`.
    pub status: String,
    pub ticket_title: String,
    pub property_id: Uuid,
    pub property_name: String,
    pub location: Option<String>,
    pub priority: String,
    pub due_date: Option<String>,
    pub ticket_status: String,
    pub waiting_on: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct QueueResp {
    pub tasks: Vec<QueueTask>,
}

fn pri(p: &str) -> u8 {
    match p {
        "urgent" => 0,
        "high" => 1,
        "normal" => 2,
        _ => 3,
    }
}

/// Urgent first, then soonest due, then the order they were given.
pub fn order_queue(rows: &mut [QueueTask]) {
    rows.sort_by(|a, b| {
        (
            a.status != "doing",
            pri(&a.priority),
            a.due_date.is_none(),
            a.due_date.clone(),
        )
            .cmp(&(
                b.status != "doing",
                pri(&b.priority),
                b.due_date.is_none(),
                b.due_date.clone(),
            ))
    });
}

/// `GET /ticket-queue` — the tasks given to the caller that aren't done, with
/// the work order each belongs to. Only work orders on properties in the
/// caller's reach.
#[rocket_okapi::openapi(tag = "Service Desk")]
#[get("/ticket-queue")]
pub async fn my_queue(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
) -> ApiResult<Json<QueueResp>> {
    user.require(Permission::MaintenanceRead)?;
    let tasks = TicketTask::find()
        .filter(entity::ticket_task::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::ticket_task::Column::AssigneeUserId.eq(user.user_id))
        .filter(entity::ticket_task::Column::Status.is_in(["todo", "doing"]))
        .order_by_asc(entity::ticket_task::Column::CreatedAt)
        .all(&db)
        .await?;
    let ticket_ids: Vec<Uuid> = tasks.iter().map(|t| t.ticket_id).collect();
    let tickets: HashMap<Uuid, entity::maintenance_ticket::Model> = MaintenanceTicket::find()
        .filter(entity::maintenance_ticket::Column::Id.is_in(ticket_ids))
        .filter(entity::maintenance_ticket::Column::Status.is_in(OPEN.to_vec()))
        .all(&db)
        .await?
        .into_iter()
        .filter(|t| access.sees(t.property_id))
        .map(|t| (t.id, t))
        .collect();
    let property_ids: Vec<Uuid> = tickets.values().map(|t| t.property_id).collect();
    let names: HashMap<Uuid, String> = Property::find()
        .filter(entity::property::Column::Id.is_in(property_ids))
        .all(&db)
        .await?
        .into_iter()
        .map(|p| (p.id, p.name))
        .collect();
    let mut rows: Vec<QueueTask> = tasks
        .into_iter()
        .filter_map(|t| {
            let k = tickets.get(&t.ticket_id)?;
            Some(QueueTask {
                task_id: t.id,
                ticket_id: k.id,
                title: t.title,
                trade: t.trade,
                est_minutes: t.est_minutes,
                status: t.status,
                ticket_title: k.title.clone(),
                property_id: k.property_id,
                property_name: names.get(&k.property_id).cloned().unwrap_or_default(),
                location: k.location.clone(),
                priority: k.priority.clone(),
                due_date: k.due_date.clone(),
                ticket_status: k.status.clone(),
                waiting_on: k.waiting_on.clone(),
            })
        })
        .collect();
    order_queue(&mut rows);
    Ok(Json(QueueResp { tasks: rows }))
}

/// Names for the people and vendors on a page of work orders, and how far
/// along each one's tasks are. Fills the list fields of [`TicketDto`].
pub async fn decorate(
    db: &impl sea_orm::ConnectionTrait,
    tenant_id: Uuid,
    rows: Vec<entity::maintenance_ticket::Model>,
) -> ApiResult<Vec<super::dto::TicketDto>> {
    let user_ids: Vec<Uuid> = rows.iter().filter_map(|t| t.assignee_user_id).collect();
    let entity_ids: Vec<Uuid> = rows.iter().filter_map(|t| t.assignee_entity_id).collect();
    let ticket_ids: Vec<Uuid> = rows.iter().map(|t| t.id).collect();
    let people: HashMap<Uuid, String> = if user_ids.is_empty() {
        HashMap::new()
    } else {
        User::find()
            .filter(entity::user::Column::Id.is_in(user_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|u| (u.id, u.name))
            .collect()
    };
    let vendors: HashMap<Uuid, String> = if entity_ids.is_empty() {
        HashMap::new()
    } else {
        Counterparty::find()
            .filter(entity::counterparty::Column::TenantId.eq(tenant_id))
            .filter(entity::counterparty::Column::Id.is_in(entity_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|c| (c.id, c.name))
            .collect()
    };
    let mut counts: HashMap<Uuid, (i64, i64)> = HashMap::new();
    if !ticket_ids.is_empty() {
        for t in TicketTask::find()
            .filter(entity::ticket_task::Column::TenantId.eq(tenant_id))
            .filter(entity::ticket_task::Column::TicketId.is_in(ticket_ids))
            .all(db)
            .await?
        {
            let c = counts.entry(t.ticket_id).or_default();
            if t.status != "skipped" {
                c.0 += 1;
            }
            if t.status == "done" {
                c.1 += 1;
            }
        }
    }
    Ok(rows
        .into_iter()
        .map(|t| {
            let (total, done) = counts.get(&t.id).copied().unwrap_or((0, 0));
            let (name, kind) = match (t.assignee_user_id, t.assignee_entity_id) {
                (Some(u), _) => (people.get(&u).cloned(), Some("tech")),
                (None, Some(e)) => (vendors.get(&e).cloned(), Some("vendor")),
                _ => (None, None),
            };
            let mut d = super::dto::TicketDto::from(t);
            d.assignee_name = name;
            d.assignee_kind = kind.map(String::from);
            d.tasks_total = total;
            d.tasks_done = done;
            d
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tech(name: &str, here: bool, t: i64, k: i64) -> TechDto {
        TechDto {
            user_id: Uuid::new_v4(),
            name: name.into(),
            email: format!("{name}@x.com"),
            role: "maintenance".into(),
            title: None,
            open_tickets: t,
            open_tasks: k,
            on_property: here,
        }
    }

    #[test]
    fn people_on_the_property_come_first_then_lightest_load() {
        let mut v = vec![
            tech("Busy", true, 5, 4),
            tech("Elsewhere", false, 0, 0),
            tech("Free", true, 0, 1),
        ];
        order_techs(&mut v);
        let names: Vec<&str> = v.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["Free", "Busy", "Elsewhere"]);
    }

    fn qt(status: &str, priority: &str, due: Option<&str>) -> QueueTask {
        QueueTask {
            task_id: Uuid::new_v4(),
            ticket_id: Uuid::new_v4(),
            title: format!("{status}-{priority}-{due:?}"),
            trade: "general".into(),
            est_minutes: None,
            status: status.into(),
            ticket_title: "t".into(),
            property_id: Uuid::nil(),
            property_name: String::new(),
            location: None,
            priority: priority.into(),
            due_date: due.map(String::from),
            ticket_status: "open".into(),
            waiting_on: None,
        }
    }

    #[test]
    fn the_queue_puts_what_you_started_then_urgent_then_soonest() {
        let mut v = vec![
            qt("todo", "normal", None),
            qt("todo", "urgent", Some("2026-03-09")),
            qt("todo", "urgent", Some("2026-03-02")),
            qt("doing", "low", None),
            qt("todo", "high", None),
        ];
        order_queue(&mut v);
        let got: Vec<String> = v.iter().map(|t| t.title.clone()).collect();
        assert_eq!(
            got,
            [
                "doing-low-None",
                "todo-urgent-Some(\"2026-03-02\")",
                "todo-urgent-Some(\"2026-03-09\")",
                "todo-high-None",
                "todo-normal-None",
            ]
        );
    }
}
