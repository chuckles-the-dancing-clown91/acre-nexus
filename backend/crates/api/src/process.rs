//! The **step engine**: ordered work with dependencies, used for unit
//! turnovers today and house onboarding and site turns next.
//!
//! The rules are pure functions over [`StepState`] so they are unit-tested
//! without a database:
//!
//! * a step is `blocked` until every step it waits on is `done` or `skipped`,
//!   then `ready`; a person moves it to `doing`, then `done` (or `skipped`
//!   with a reason);
//! * a process may finish when every *required* step is done or skipped, or
//!   with an override reason (which is audited);
//! * a template's step graph must have unique keys, known dependencies, and
//!   no cycles.
//!
//! The database half instantiates a template into a run, keeps readiness
//! current, and links steps to work orders.

use crate::error::{ApiError, ApiResult};
use chrono::{Days, NaiveDate, Utc};
use entity::prelude::{
    MaintenanceTicket, Process, ProcessStep, ProcessTemplate, ProcessTemplateStep, Unit,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub const KIND_TURNOVER: &str = "turnover";
pub const KINDS: &[&str] = &[KIND_TURNOVER, "onboarding", "site_turn"];
pub const OWNER_ROLES: &[&str] = &["office", "maintenance", "vendor", "leasing", "owner"];

pub const BLOCKED: &str = "blocked";
pub const READY: &str = "ready";
pub const DOING: &str = "doing";
pub const DONE: &str = "done";
pub const SKIPPED: &str = "skipped";

/// Is this step finished for the purpose of unblocking others?
pub fn finished(status: &str) -> bool {
    status == DONE || status == SKIPPED
}

/// What the rules need to know about one step.
#[derive(Clone, Debug, PartialEq)]
pub struct StepState {
    pub key: String,
    pub title: String,
    pub status: String,
    pub required: bool,
    pub depends_on: Vec<String>,
}

/// Check a template's step graph: non-empty unique keys, dependencies that
/// exist, and no cycles. The error names the problem.
pub fn validate_graph(steps: &[(String, Vec<String>)]) -> Result<(), String> {
    if steps.is_empty() {
        return Err("a template needs at least one step".into());
    }
    let mut keys = HashSet::new();
    for (k, _) in steps {
        if k.trim().is_empty() {
            return Err("every step needs a key".into());
        }
        if !keys.insert(k.as_str()) {
            return Err(format!("duplicate step key: {k}"));
        }
    }
    for (k, deps) in steps {
        for d in deps {
            if d == k {
                return Err(format!("step {k} cannot wait on itself"));
            }
            if !keys.contains(d.as_str()) {
                return Err(format!("step {k} waits on unknown step {d}"));
            }
        }
    }
    // Kahn's algorithm: anything left over sits on a cycle.
    let mut indeg: HashMap<&str, usize> =
        steps.iter().map(|(k, d)| (k.as_str(), d.len())).collect();
    let mut queue: Vec<&str> = indeg
        .iter()
        .filter(|(_, n)| **n == 0)
        .map(|(k, _)| *k)
        .collect();
    let mut seen = 0;
    while let Some(k) = queue.pop() {
        seen += 1;
        for (other, deps) in steps {
            if deps.iter().any(|d| d == k) {
                let n = indeg.get_mut(other.as_str()).expect("known key");
                *n -= 1;
                if *n == 0 {
                    queue.push(other.as_str());
                }
            }
        }
    }
    if seen != steps.len() {
        return Err("the steps wait on each other in a loop".into());
    }
    Ok(())
}

/// Recompute `blocked`/`ready` for every step that has not been started.
/// Returns the keys whose status changed. Started and finished steps never move.
pub fn refresh_readiness(steps: &mut [StepState]) -> Vec<String> {
    let done: HashSet<String> = steps
        .iter()
        .filter(|s| finished(&s.status))
        .map(|s| s.key.clone())
        .collect();
    let mut changed = Vec::new();
    for s in steps.iter_mut() {
        if s.status != BLOCKED && s.status != READY {
            continue;
        }
        let open = s.depends_on.iter().all(|d| done.contains(d));
        let want = if open { READY } else { BLOCKED };
        if s.status != want {
            s.status = want.into();
            changed.push(s.key.clone());
        }
    }
    changed
}

/// Titles of required steps that are not yet done or skipped.
pub fn unmet_required(steps: &[StepState]) -> Vec<String> {
    steps
        .iter()
        .filter(|s| s.required && !finished(&s.status))
        .map(|s| s.title.clone())
        .collect()
}

/// `(finished, total)` steps.
pub fn progress(steps: &[StepState]) -> (usize, usize) {
    (
        steps.iter().filter(|s| finished(&s.status)).count(),
        steps.len(),
    )
}

/// The ISO date `offset` days after `start`.
pub fn due_on(start: &str, offset: i32) -> Option<String> {
    let d = NaiveDate::parse_from_str(start, "%Y-%m-%d").ok()?;
    let d = if offset >= 0 {
        d.checked_add_days(Days::new(offset as u64))?
    } else {
        d.checked_sub_days(Days::new(offset.unsigned_abs() as u64))?
    };
    Some(d.format("%Y-%m-%d").to_string())
}

/// Whole days between two ISO dates (0 when either is unparseable).
pub fn days_between(from: &str, to: &str) -> i64 {
    match (
        NaiveDate::parse_from_str(from, "%Y-%m-%d"),
        NaiveDate::parse_from_str(to, "%Y-%m-%d"),
    ) {
        (Ok(a), Ok(b)) => (b - a).num_days().max(0),
        _ => 0,
    }
}

pub fn today() -> String {
    Utc::now().format("%Y-%m-%d").to_string()
}

// ---------------------------------------------------------------------------
// Default templates
// ---------------------------------------------------------------------------

/// `(key, title, description, owner, depends_on, due_offset_days, required,
/// requires_photo, ticket_category)`.
type Seed = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    i32,
    bool,
    bool,
    Option<&'static str>,
);

/// The default 13-step unit turnover.
pub const TURNOVER_STEPS: &[Seed] = &[
    (
        "notice",
        "Move-out notice logged",
        "Confirm the move-out date and forwarding address.",
        "office",
        &[],
        0,
        true,
        false,
        None,
    ),
    (
        "inspect",
        "Move-out inspection",
        "Walk the unit and record condition against move-in.",
        "maintenance",
        &["notice"],
        1,
        true,
        true,
        None,
    ),
    (
        "deposit",
        "Deposit accounting",
        "Itemize deductions and send the disposition within the legal window.",
        "office",
        &["inspect"],
        14,
        true,
        false,
        None,
    ),
    (
        "trash_out",
        "Trash-out",
        "Remove anything left behind.",
        "maintenance",
        &["inspect"],
        2,
        false,
        false,
        Some("general"),
    ),
    (
        "repairs",
        "Repairs",
        "Work the repair list from the inspection.",
        "maintenance",
        &["inspect"],
        5,
        true,
        false,
        Some("general"),
    ),
    (
        "paint",
        "Paint",
        "Patch and paint.",
        "vendor",
        &["repairs", "trash_out"],
        7,
        false,
        false,
        Some("general"),
    ),
    (
        "flooring",
        "Flooring",
        "Replace or deep-clean flooring.",
        "vendor",
        &["repairs", "trash_out"],
        7,
        false,
        false,
        Some("general"),
    ),
    (
        "clean",
        "Deep clean",
        "Final clean after trades are out.",
        "vendor",
        &["paint", "flooring"],
        9,
        true,
        true,
        Some("general"),
    ),
    (
        "rekey",
        "Rekey locks",
        "Rekey and log keys.",
        "maintenance",
        &["repairs"],
        6,
        true,
        false,
        Some("general"),
    ),
    (
        "systems",
        "Systems check",
        "Smoke and CO detectors, filters, water heater, HVAC.",
        "maintenance",
        &["repairs"],
        8,
        true,
        false,
        Some("hvac"),
    ),
    (
        "final",
        "Final walk",
        "Sign off that the unit is rent-ready.",
        "office",
        &["clean", "rekey", "systems"],
        10,
        true,
        true,
        None,
    ),
    (
        "photos",
        "Listing photos",
        "Shoot photos for the listing.",
        "leasing",
        &["final"],
        11,
        false,
        true,
        None,
    ),
    (
        "list",
        "List the unit",
        "Publish the listing and mark the unit vacant.",
        "leasing",
        &["final"],
        12,
        true,
        false,
        None,
    ),
];

/// Make sure the workspace has a default template for `kind`; returns it.
pub async fn ensure_default_template(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    kind: &str,
) -> ApiResult<entity::process_template::Model> {
    if let Some(t) = ProcessTemplate::find()
        .filter(entity::process_template::Column::TenantId.eq(tenant_id))
        .filter(entity::process_template::Column::Kind.eq(kind))
        .filter(entity::process_template::Column::IsDefault.eq(true))
        .filter(entity::process_template::Column::Active.eq(true))
        .one(db)
        .await?
    {
        return Ok(t);
    }
    let seeds: &[Seed] = match kind {
        KIND_TURNOVER => TURNOVER_STEPS,
        _ => &[],
    };
    let now = Utc::now();
    let tpl = entity::process_template::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        kind: Set(kind.to_string()),
        name: Set(match kind {
            KIND_TURNOVER => "Standard turnover".into(),
            _ => format!("Standard {kind}"),
        }),
        is_default: Set(true),
        active: Set(true),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    for (i, s) in seeds.iter().enumerate() {
        entity::process_template_step::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            template_id: Set(tpl.id),
            position: Set(i as i32),
            key: Set(s.0.into()),
            title: Set(s.1.into()),
            description: Set(Some(s.2.into())),
            owner_role: Set(s.3.into()),
            depends_on: Set(json!(s.4)),
            due_offset_days: Set(s.5),
            required: Set(s.6),
            requires_photo: Set(s.7),
            ticket_category: Set(s.8.map(Into::into)),
            ticket_priority: Set(None),
        }
        .insert(db)
        .await?;
    }
    Ok(tpl)
}

pub fn deps_of(v: &serde_json::Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn state_of(s: &entity::process_step::Model) -> StepState {
    StepState {
        key: s.key.clone(),
        title: s.title.clone(),
        status: s.status.clone(),
        required: s.required,
        depends_on: deps_of(&s.depends_on),
    }
}

pub async fn steps_of(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    process_id: Uuid,
) -> ApiResult<Vec<entity::process_step::Model>> {
    Ok(ProcessStep::find()
        .filter(entity::process_step::Column::TenantId.eq(tenant_id))
        .filter(entity::process_step::Column::ProcessId.eq(process_id))
        .order_by_asc(entity::process_step::Column::Position)
        .all(db)
        .await?)
}

/// Persist `blocked`/`ready` changes after any step moved.
pub async fn refresh(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    process_id: Uuid,
) -> ApiResult<Vec<entity::process_step::Model>> {
    let rows = steps_of(db, tenant_id, process_id).await?;
    let mut states: Vec<StepState> = rows.iter().map(state_of).collect();
    let changed: HashSet<String> = refresh_readiness(&mut states).into_iter().collect();
    if changed.is_empty() {
        return Ok(rows);
    }
    let by_key: HashMap<&str, &StepState> = states.iter().map(|s| (s.key.as_str(), s)).collect();
    for r in rows.iter().filter(|r| changed.contains(&r.key)) {
        let mut am: entity::process_step::ActiveModel = r.clone().into();
        am.status = Set(by_key[r.key.as_str()].status.clone());
        am.updated_at = Set(Utc::now().into());
        am.update(db).await?;
    }
    steps_of(db, tenant_id, process_id).await
}

/// Start a run of the workspace's default template for `kind`. One active run
/// per unit (or per property when there is no unit); the database enforces it.
#[allow(clippy::too_many_arguments)]
pub async fn start(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    kind: &str,
    property_id: Uuid,
    unit_id: Option<Uuid>,
    lease_id: Option<Uuid>,
    template_id: Option<Uuid>,
    started_on: &str,
    title: String,
    created_by: Option<Uuid>,
) -> ApiResult<entity::process::Model> {
    let tpl = match template_id {
        Some(id) => ProcessTemplate::find_by_id(id)
            .filter(entity::process_template::Column::TenantId.eq(tenant_id))
            .filter(entity::process_template::Column::Kind.eq(kind))
            .one(db)
            .await?
            .ok_or_else(|| ApiError::NotFound("template not found".into()))?,
        None => ensure_default_template(db, tenant_id, kind).await?,
    };
    let mut q = Process::find()
        .filter(entity::process::Column::TenantId.eq(tenant_id))
        .filter(entity::process::Column::Kind.eq(kind))
        .filter(entity::process::Column::Status.eq("active"));
    q = match unit_id {
        Some(u) => q.filter(entity::process::Column::UnitId.eq(u)),
        None => q
            .filter(entity::process::Column::PropertyId.eq(property_id))
            .filter(entity::process::Column::UnitId.is_null()),
    };
    if q.count(db).await? > 0 {
        return Err(ApiError::Conflict(format!(
            "a {kind} is already in progress here"
        )));
    }
    let tsteps = ProcessTemplateStep::find()
        .filter(entity::process_template_step::Column::TenantId.eq(tenant_id))
        .filter(entity::process_template_step::Column::TemplateId.eq(tpl.id))
        .order_by_asc(entity::process_template_step::Column::Position)
        .all(db)
        .await?;
    let now = Utc::now();
    let last_offset = tsteps.iter().map(|s| s.due_offset_days).max().unwrap_or(0);
    let proc = entity::process::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        kind: Set(kind.to_string()),
        property_id: Set(property_id),
        unit_id: Set(unit_id),
        lease_id: Set(lease_id),
        template_id: Set(Some(tpl.id)),
        title: Set(title),
        status: Set("active".into()),
        started_on: Set(started_on.to_string()),
        target_date: Set(due_on(started_on, last_offset)),
        finished_on: Set(None),
        override_reason: Set(None),
        created_by: Set(created_by),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    for s in &tsteps {
        entity::process_step::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            process_id: Set(proc.id),
            position: Set(s.position),
            key: Set(s.key.clone()),
            title: Set(s.title.clone()),
            description: Set(s.description.clone()),
            owner_role: Set(s.owner_role.clone()),
            assignee_user_id: Set(None),
            depends_on: Set(s.depends_on.clone()),
            due_on: Set(due_on(started_on, s.due_offset_days)),
            required: Set(s.required),
            requires_photo: Set(s.requires_photo),
            status: Set(BLOCKED.into()),
            started_at: Set(None),
            done_at: Set(None),
            done_by: Set(None),
            skip_reason: Set(None),
            note: Set(None),
            ticket_id: Set(None),
            cost_cents: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(db)
        .await?;
    }
    refresh(db, tenant_id, proc.id).await?;
    Ok(proc)
}

/// Required steps still open for the unit's active turn (empty when there is
/// no active turn). The unit-status gate uses it.
pub async fn unmet_for_unit(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    unit_id: Uuid,
) -> ApiResult<Vec<String>> {
    let Some(p) = Process::find()
        .filter(entity::process::Column::TenantId.eq(tenant_id))
        .filter(entity::process::Column::Kind.eq(KIND_TURNOVER))
        .filter(entity::process::Column::UnitId.eq(unit_id))
        .filter(entity::process::Column::Status.eq("active"))
        .one(db)
        .await?
    else {
        return Ok(vec![]);
    };
    let rows = steps_of(db, tenant_id, p.id).await?;
    Ok(unmet_required(
        &rows.iter().map(state_of).collect::<Vec<_>>(),
    ))
}

/// A work order reached resolved/closed: the step that opened it is done.
pub async fn on_ticket_resolved(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    ticket_id: Uuid,
    by: Option<Uuid>,
) -> ApiResult<()> {
    let Some(step) = ProcessStep::find()
        .filter(entity::process_step::Column::TenantId.eq(tenant_id))
        .filter(entity::process_step::Column::TicketId.eq(ticket_id))
        .one(db)
        .await?
    else {
        return Ok(());
    };
    if finished(&step.status) {
        return Ok(());
    }
    let cost = MaintenanceTicket::find_by_id(ticket_id)
        .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .and_then(|t| t.cost_cents);
    let pid = step.process_id;
    let mut am: entity::process_step::ActiveModel = step.into();
    am.status = Set(DONE.into());
    am.done_at = Set(Some(Utc::now().into()));
    am.done_by = Set(by);
    am.cost_cents = Set(cost);
    am.updated_at = Set(Utc::now().into());
    am.update(db).await?;
    refresh(db, tenant_id, pid).await?;
    Ok(())
}

/// The turn that the move-out inspection starts: opens the run and links the
/// make-ready ticket to the "repairs" step. Best-effort wrapper lives at the
/// call site.
pub async fn start_turn_for_move_out(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    inspection: &entity::inspection::Model,
    ticket: Option<&entity::maintenance_ticket::Model>,
    by: Uuid,
) -> ApiResult<Option<entity::process::Model>> {
    let Some(unit_id) = inspection.unit_id else {
        return Ok(None);
    };
    let unit = Unit::find_by_id(unit_id)
        .filter(entity::unit::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?;
    let label = unit
        .map(|u| format!("Turn unit {}", u.unit_number))
        .unwrap_or_else(|| "Turn unit".into());
    let proc = match start(
        db,
        tenant_id,
        KIND_TURNOVER,
        inspection.property_id,
        Some(unit_id),
        Some(inspection.lease_id),
        None,
        &today(),
        label,
        Some(by),
    )
    .await
    {
        Ok(p) => p,
        Err(ApiError::Conflict(_)) => return Ok(None),
        Err(e) => return Err(e),
    };
    // The notice and inspection are already behind us.
    for key in ["notice", "inspect"] {
        if let Some(s) = ProcessStep::find()
            .filter(entity::process_step::Column::TenantId.eq(tenant_id))
            .filter(entity::process_step::Column::ProcessId.eq(proc.id))
            .filter(entity::process_step::Column::Key.eq(key))
            .one(db)
            .await?
        {
            let mut am: entity::process_step::ActiveModel = s.into();
            am.status = Set(DONE.into());
            am.done_at = Set(Some(Utc::now().into()));
            am.done_by = Set(Some(by));
            am.updated_at = Set(Utc::now().into());
            am.update(db).await?;
        }
    }
    if let Some(t) = ticket {
        if let Some(s) = ProcessStep::find()
            .filter(entity::process_step::Column::TenantId.eq(tenant_id))
            .filter(entity::process_step::Column::ProcessId.eq(proc.id))
            .filter(entity::process_step::Column::Key.eq("repairs"))
            .one(db)
            .await?
        {
            let mut am: entity::process_step::ActiveModel = s.into();
            am.ticket_id = Set(Some(t.id));
            am.updated_at = Set(Utc::now().into());
            am.update(db).await?;
        }
    }
    refresh(db, tenant_id, proc.id).await?;
    Ok(Some(proc))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(key: &str, status: &str, req: bool, deps: &[&str]) -> StepState {
        StepState {
            key: key.into(),
            title: key.to_uppercase(),
            status: status.into(),
            required: req,
            depends_on: deps.iter().map(|d| d.to_string()).collect(),
        }
    }

    fn g(v: &[(&str, &[&str])]) -> Vec<(String, Vec<String>)> {
        v.iter()
            .map(|(k, d)| (k.to_string(), d.iter().map(|x| x.to_string()).collect()))
            .collect()
    }

    #[test]
    fn graph_rejects_cycles_unknowns_and_duplicates() {
        assert!(validate_graph(&g(&[("a", &[]), ("b", &["a"])])).is_ok());
        assert!(validate_graph(&g(&[("a", &["b"]), ("b", &["a"])])).is_err());
        assert!(validate_graph(&g(&[("a", &["a"])])).is_err());
        assert!(validate_graph(&g(&[("a", &["zzz"])])).is_err());
        assert!(validate_graph(&g(&[("a", &[]), ("a", &[])])).is_err());
        assert!(validate_graph(&[]).is_err());
    }

    #[test]
    fn default_turnover_is_a_valid_graph() {
        let steps: Vec<_> = TURNOVER_STEPS
            .iter()
            .map(|s| (s.0.to_string(), s.4.iter().map(|d| d.to_string()).collect()))
            .collect();
        assert!(validate_graph(&steps).is_ok());
        assert_eq!(TURNOVER_STEPS.len(), 13);
    }

    #[test]
    fn readiness_follows_dependencies() {
        let mut s = vec![
            st("a", BLOCKED, true, &[]),
            st("b", BLOCKED, true, &["a"]),
            st("c", BLOCKED, false, &["a", "b"]),
        ];
        assert_eq!(refresh_readiness(&mut s), vec!["a"]);
        assert_eq!(s[0].status, READY);
        assert_eq!(s[1].status, BLOCKED);
        s[0].status = DONE.into();
        refresh_readiness(&mut s);
        assert_eq!(s[1].status, READY);
        assert_eq!(s[2].status, BLOCKED);
        s[1].status = SKIPPED.into();
        refresh_readiness(&mut s);
        assert_eq!(s[2].status, READY);
    }

    #[test]
    fn reopening_a_step_re_blocks_dependents_that_have_not_started() {
        let mut s = vec![st("a", DONE, true, &[]), st("b", READY, true, &["a"])];
        s[0].status = READY.into(); // reopened
        refresh_readiness(&mut s);
        assert_eq!(s[1].status, BLOCKED);
    }

    #[test]
    fn started_steps_never_move() {
        let mut s = vec![st("a", READY, true, &[]), st("b", DOING, true, &["a"])];
        refresh_readiness(&mut s);
        assert_eq!(s[1].status, DOING);
    }

    #[test]
    fn gate_lists_unmet_required_only() {
        let s = vec![
            st("a", DONE, true, &[]),
            st("b", READY, true, &[]),
            st("c", BLOCKED, false, &[]),
            st("d", SKIPPED, true, &[]),
        ];
        assert_eq!(unmet_required(&s), vec!["B"]);
        assert_eq!(progress(&s), (2, 4));
    }

    #[test]
    fn dates() {
        assert_eq!(due_on("2026-01-30", 3).as_deref(), Some("2026-02-02"));
        assert_eq!(due_on("2026-01-30", -1).as_deref(), Some("2026-01-29"));
        assert_eq!(due_on("nope", 1), None);
        assert_eq!(days_between("2026-01-01", "2026-01-11"), 10);
        assert_eq!(days_between("2026-01-11", "2026-01-01"), 0);
    }
}
