//! **The audit trail**: who changed what, on which property (roadmap area 1).
//!
//! * `GET /properties/<id>/history` — everything that touched a property: its
//!   own edits, and its units, appliances, work orders, leases, listings, maps
//!   and turns, newest first, with the before → after of each change.
//! * `GET /audit/events` — the workspace's trail with filters (property, kind,
//!   target, person, action, dates, "Vantedge support only") and a cursor.
//! * `GET /audit/events.csv` — the same, as a download.
//!
//! `audit_log` is not under row-level security, so every query here filters
//! the workspace itself.

use crate::audit::change::Change;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::routes::reports::ReportFile;
use crate::tenancy::TenantScope;
use chrono::{DateTime, Utc};
use entity::prelude::{AuditLog, User};
use rocket::get;
use rocket::serde::json::Json;
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use serde::Serialize;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema, Clone)]
pub struct TrailEvent {
    pub id: Uuid,
    pub at: String,
    pub actor_id: Option<Uuid>,
    /// Who did it: a person's name, or "System" for a background job.
    pub actor_name: String,
    /// A Vantedge employee made this change on the customer's behalf.
    pub support: bool,
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub property_id: Option<Uuid>,
    /// What it was done to, as a person would say it ("Unit 4B").
    pub label: String,
    /// "changed rent, status" / "created" / "removed".
    pub summary: String,
    pub changes: Vec<Change>,
    pub reason: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct TrailPage {
    pub events: Vec<TrailEvent>,
    /// Pass back as `before` for the next page; absent on the last page.
    pub next: Option<String>,
}

#[derive(Default, Clone)]
struct Filter {
    property_id: Option<Uuid>,
    target_type: Option<String>,
    target_id: Option<String>,
    actor: Option<Uuid>,
    action: Option<String>,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
    support_only: bool,
    before: Option<DateTime<Utc>>,
}

fn parse_time(s: &Option<String>, what: &str) -> ApiResult<Option<DateTime<Utc>>> {
    match s.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(v) => {
            if let Ok(d) = DateTime::parse_from_rfc3339(v) {
                return Ok(Some(d.with_timezone(&Utc)));
            }
            if let Ok(d) = chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d") {
                return Ok(d.and_hms_opt(0, 0, 0).map(|t| t.and_utc()));
            }
            Err(ApiError::BadRequest(format!(
                "{what} must be a date or timestamp"
            )))
        }
    }
}

fn parse_uuid(s: &Option<String>, what: &str) -> ApiResult<Option<Uuid>> {
    match s.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(v) => Uuid::parse_str(v)
            .map(Some)
            .map_err(|_| ApiError::BadRequest(format!("invalid {what}"))),
    }
}

async fn run(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    f: &Filter,
    limit: u64,
) -> ApiResult<TrailPage> {
    let mut cond = Condition::all()
        .add(entity::audit_log::Column::TenantId.eq(tenant_id))
        .add(entity::audit_log::Column::Action.ne(crate::audit::actions::HTTP_REQUEST))
        // Domain events only: the ones that name what they were about.
        .add(entity::audit_log::Column::TargetType.is_not_null());
    if let Some(p) = f.property_id {
        cond = cond.add(
            Condition::any()
                .add(entity::audit_log::Column::PropertyId.eq(p))
                .add(
                    Condition::all()
                        .add(entity::audit_log::Column::TargetType.eq("property"))
                        .add(entity::audit_log::Column::TargetId.eq(p.to_string())),
                ),
        );
    }
    if let Some(t) = &f.target_type {
        // "process" covers a turn and its steps.
        cond = if t == "process" {
            cond.add(
                entity::audit_log::Column::TargetType
                    .is_in(["process", "process_step", "process_template"]),
            )
        } else {
            cond.add(entity::audit_log::Column::TargetType.eq(t.clone()))
        };
    }
    if let Some(t) = &f.target_id {
        cond = cond.add(entity::audit_log::Column::TargetId.eq(t.clone()));
    }
    if let Some(a) = f.actor {
        cond = cond.add(entity::audit_log::Column::ActorUserId.eq(a));
    }
    if let Some(a) = &f.action {
        cond = cond.add(entity::audit_log::Column::Action.eq(a.clone()));
    }
    if let Some(d) = f.from {
        cond = cond.add(entity::audit_log::Column::CreatedAt.gte(d));
    }
    if let Some(d) = f.to {
        cond = cond.add(entity::audit_log::Column::CreatedAt.lt(d));
    }
    if f.support_only {
        cond = cond.add(entity::audit_log::Column::Support.eq(true));
    }
    if let Some(d) = f.before {
        cond = cond.add(entity::audit_log::Column::CreatedAt.lt(d));
    }
    let mut rows = AuditLog::find()
        .filter(cond)
        .order_by_desc(entity::audit_log::Column::CreatedAt)
        .limit(limit + 1)
        .all(db)
        .await?;
    let next = if rows.len() as u64 > limit {
        rows.truncate(limit as usize);
        rows.last().map(|r| r.created_at.to_rfc3339())
    } else {
        None
    };
    let ids: Vec<Uuid> = rows.iter().filter_map(|r| r.actor_user_id).collect();
    let names: HashMap<Uuid, String> = if ids.is_empty() {
        HashMap::new()
    } else {
        User::find()
            .filter(entity::user::Column::Id.is_in(ids))
            .all(db)
            .await?
            .into_iter()
            .map(|u| (u.id, u.name))
            .collect()
    };
    let events = rows
        .into_iter()
        .map(|r| {
            let meta = r.metadata.clone().unwrap_or_default();
            let changes: Vec<Change> = meta
                .get("changes")
                .and_then(|c| serde_json::from_value(c.clone()).ok())
                .unwrap_or_default();
            let text = |k: &str| meta.get(k).and_then(|v| v.as_str()).map(str::to_string);
            // Older rows have only a status in their metadata: say what we can.
            let summary = text("summary").unwrap_or_else(|| {
                r.action
                    .rsplit('.')
                    .next()
                    .unwrap_or("changed")
                    .replace('_', " ")
            });
            TrailEvent {
                id: r.id,
                at: r.created_at.to_rfc3339(),
                actor_id: r.actor_user_id,
                actor_name: r
                    .actor_user_id
                    .and_then(|a| names.get(&a).cloned())
                    .unwrap_or_else(|| "System".into()),
                support: r.support,
                label: text("label")
                    .or_else(|| text("unit_number").map(|u| format!("Unit {u}")))
                    .unwrap_or_else(|| {
                        format!(
                            "{} {}",
                            r.target_type.clone().unwrap_or_default().replace('_', " "),
                            r.target_id
                                .clone()
                                .unwrap_or_default()
                                .chars()
                                .take(8)
                                .collect::<String>()
                        )
                        .trim()
                        .to_string()
                    }),
                action: r.action,
                target_type: r.target_type,
                target_id: r.target_id,
                property_id: r.property_id,
                summary,
                changes,
                reason: text("reason"),
            }
        })
        .collect();
    Ok(TrailPage { events, next })
}

fn limit_of(limit: Option<u64>) -> u64 {
    limit.unwrap_or(50).clamp(1, 200)
}

/// `GET /properties/<id>/history` — who changed what on this property.
#[rocket_okapi::openapi(tag = "Audit")]
#[get("/properties/<id>/history?<limit>&<before>&<kind>&<support>")]
#[allow(clippy::too_many_arguments)]
pub async fn property_history(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    limit: Option<u64>,
    before: Option<String>,
    kind: Option<String>,
    support: Option<bool>,
) -> ApiResult<Json<TrailPage>> {
    user.require(Permission::PropertyRead)?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    entity::prelude::Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    let f = Filter {
        property_id: Some(pid),
        target_type: kind.filter(|k| !k.trim().is_empty()),
        support_only: support.unwrap_or(false),
        before: parse_time(&before, "before")?,
        ..Filter::default()
    };
    Ok(Json(run(&db, scope.tenant_id, &f, limit_of(limit)).await?))
}

#[allow(clippy::too_many_arguments)]
fn filter_of(
    property_id: Option<String>,
    target_type: Option<String>,
    target_id: Option<String>,
    actor: Option<String>,
    action: Option<String>,
    from: Option<String>,
    to: Option<String>,
    support: Option<bool>,
    before: Option<String>,
) -> ApiResult<Filter> {
    Ok(Filter {
        property_id: parse_uuid(&property_id, "property_id")?,
        target_type: target_type.filter(|s| !s.trim().is_empty()),
        target_id: target_id.filter(|s| !s.trim().is_empty()),
        actor: parse_uuid(&actor, "actor")?,
        action: action.filter(|s| !s.trim().is_empty()),
        from: parse_time(&from, "from")?,
        to: parse_time(&to, "to")?,
        support_only: support.unwrap_or(false),
        before: parse_time(&before, "before")?,
    })
}

/// `GET /audit/events` — the workspace's trail, filterable.
#[rocket_okapi::openapi(tag = "Audit")]
#[get("/audit/events?<property_id>&<target_type>&<target_id>&<actor>&<action>&<from>&<to>&<support>&<before>&<limit>")]
#[allow(clippy::too_many_arguments)]
pub async fn events(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    property_id: Option<String>,
    target_type: Option<String>,
    target_id: Option<String>,
    actor: Option<String>,
    action: Option<String>,
    from: Option<String>,
    to: Option<String>,
    support: Option<bool>,
    before: Option<String>,
    limit: Option<u64>,
) -> ApiResult<Json<TrailPage>> {
    user.require(Permission::AuditRead)?;
    let f = filter_of(
        property_id,
        target_type,
        target_id,
        actor,
        action,
        from,
        to,
        support,
        before,
    )?;
    Ok(Json(run(&db, scope.tenant_id, &f, limit_of(limit)).await?))
}

fn csv_cell(s: &str) -> String {
    // Quote anything with a comma, quote or newline; neutralize spreadsheet
    // formulas (a cell that starts with = + - @ would run in Excel).
    let s = if s.starts_with(['=', '+', '-', '@']) {
        format!("'{s}")
    } else {
        s.to_string()
    };
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s
    }
}

fn show(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "—".into(),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// `GET /audit/events.csv` — the trail as a download (up to 5,000 rows).
#[rocket_okapi::openapi(skip)]
#[get("/audit/events.csv?<property_id>&<target_type>&<target_id>&<actor>&<action>&<from>&<to>&<support>")]
#[allow(clippy::too_many_arguments)]
pub async fn events_csv(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    property_id: Option<String>,
    target_type: Option<String>,
    target_id: Option<String>,
    actor: Option<String>,
    action: Option<String>,
    from: Option<String>,
    to: Option<String>,
    support: Option<bool>,
) -> ApiResult<ReportFile> {
    user.require(Permission::AuditRead)?;
    let f = filter_of(
        property_id,
        target_type,
        target_id,
        actor,
        action,
        from,
        to,
        support,
        None,
    )?;
    let page = run(&db, scope.tenant_id, &f, 200).await?;
    let mut events = page.events;
    let mut next = page.next;
    while let Some(cursor) = next.take() {
        if events.len() >= 5000 {
            break;
        }
        let mut g = f.clone();
        g.before = parse_time(&Some(cursor), "before")?;
        let more = run(&db, scope.tenant_id, &g, 200).await?;
        events.extend(more.events);
        next = more.next;
    }
    let mut out = String::from("When,Who,Vantedge support,What,Kind,Summary,Changes,Reason\n");
    for e in &events {
        let changes = e
            .changes
            .iter()
            .map(|c| format!("{}: {} → {}", c.field, show(&c.from), show(&c.to)))
            .collect::<Vec<_>>()
            .join("; ");
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{}\n",
            csv_cell(&e.at),
            csv_cell(&e.actor_name),
            if e.support { "yes" } else { "" },
            csv_cell(&e.label),
            csv_cell(e.target_type.as_deref().unwrap_or("")),
            csv_cell(&e.summary),
            csv_cell(&changes),
            csv_cell(e.reason.as_deref().unwrap_or("")),
        ));
    }
    Ok(ReportFile::new(
        out.into_bytes(),
        "text/csv",
        "audit-trail.csv".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::csv_cell;

    #[test]
    fn csv_cells_are_quoted_and_formulas_neutralized() {
        assert_eq!(csv_cell("plain"), "plain");
        assert_eq!(csv_cell("a,b"), "\"a,b\"");
        assert_eq!(csv_cell("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_cell("=SUM(A1)"), "'=SUM(A1)");
        assert_eq!(csv_cell("-5"), "'-5");
    }
}
