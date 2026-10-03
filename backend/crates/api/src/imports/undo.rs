//! Undoing an import: remove what it made, newest first (leases, then units,
//! then properties, owners and vendors), unless something has happened to a
//! record since. A lease with a payment on it, a property with a work order or
//! a listing, a vendor on a job, or anything edited after the import stays,
//! and the report says why.
//!
//! What points at a record is found from the database's own catalogue (every
//! table with a `property_id`, `unit_id`, … column), so a table added later is
//! counted without this file changing. Only the data the import itself set off
//! (enrichment: parcel, tax, schools, valuations, utilities) goes with the
//! property.

use super::apply::Made;
use sea_orm::{ConnectionTrait, DbBackend, DbErr, Statement, Value};
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use uuid::Uuid;

/// Written by enrichment when a property is added, not by people.
const DERIVED: &[&str] = &[
    "property_detail",
    "property_tax",
    "property_school",
    "property_valuation",
    "property_utility",
    "enrichment_run",
];

#[derive(Serialize, Clone, schemars::JsonSchema)]
pub struct Kept {
    pub t: String,
    pub id: Uuid,
    pub reason: String,
}

#[derive(Serialize, Default, schemars::JsonSchema)]
pub struct UndoReport {
    /// Removed, by kind.
    pub removed: BTreeMap<String, usize>,
    pub kept: Vec<Kept>,
}

fn table_of(t: &str) -> Option<(&'static str, &'static [&'static str])> {
    Some(match t {
        "lease" => ("lease", &["lease_id"]),
        "unit" => ("unit", &["unit_id"]),
        "property" => ("property", &["property_id"]),
        "owner" => ("owner", &["owner_id"]),
        "vendor" => (
            "counterparty",
            &[
                "counterparty_id",
                "assignee_entity_id",
                "vendor_id",
                "entity_id",
                "partner_counterparty_id",
            ],
        ),
        _ => return None,
    })
}

/// Tables with any of these columns, other than the record's own.
async fn referrers(
    db: &impl ConnectionTrait,
    own: &str,
    cols: &[&str],
) -> Result<Vec<(String, String)>, DbErr> {
    let list = cols
        .iter()
        .map(|c| format!("'{c}'"))
        .collect::<Vec<_>>()
        .join(",");
    let rows = db
        .query_all(Statement::from_string(
            DbBackend::Postgres,
            format!(
                "SELECT table_name::text AS t, column_name::text AS c FROM information_schema.columns \
                 WHERE table_schema = current_schema() AND column_name IN ({list}) \
                 AND table_name <> '{own}' AND table_name <> 'audit_log' ORDER BY table_name"
            ),
        ))
        .await?;
    rows.iter()
        .map(|r| Ok((r.try_get::<String>("", "t")?, r.try_get::<String>("", "c")?)))
        .collect()
}

async fn count(db: &impl ConnectionTrait, table: &str, col: &str, id: Uuid) -> Result<i64, DbErr> {
    let r = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!("SELECT COUNT(*)::bigint AS n FROM \"{table}\" WHERE \"{col}\" = $1"),
            [Value::from(id)],
        ))
        .await?;
    Ok(r.map(|r| r.try_get::<i64>("", "n").unwrap_or(0))
        .unwrap_or(0))
}

/// Whether the record changed after the import (tables with `updated_at`).
async fn edited_since(
    db: &impl ConnectionTrait,
    table: &str,
    id: Uuid,
    since: chrono::DateTime<chrono::FixedOffset>,
) -> Result<bool, DbErr> {
    let has = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT 1 AS x FROM information_schema.columns WHERE table_schema = current_schema() \
             AND table_name = $1 AND column_name = 'updated_at'",
            [Value::from(table.to_string())],
        ))
        .await?
        .is_some();
    if !has {
        return Ok(false);
    }
    // A minute's grace: the import itself touches rows (occupancy) as it ends.
    let r = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!(
                "SELECT 1 AS x FROM \"{table}\" WHERE id = $1 AND updated_at > $2::timestamptz + interval '1 minute'"
            ),
            [Value::from(id), Value::from(since.to_rfc3339())],
        ))
        .await?;
    Ok(r.is_some())
}

fn label(table: &str) -> String {
    match table {
        "lease_payment" => "payments".into(),
        "lease_charge" => "charges".into(),
        "maintenance_ticket" => "work orders".into(),
        "listing" => "a listing".into(),
        "ledger_entry" => "ledger entries".into(),
        "lease" => "leases".into(),
        "unit" => "units".into(),
        "entity_ownership" | "ownership" => "ownership records".into(),
        t => t.replace('_', " "),
    }
}

/// Remove what a batch made, as far as it safely can.
pub async fn undo(
    db: &impl ConnectionTrait,
    made: &[Made],
    since: chrono::DateTime<chrono::FixedOffset>,
) -> Result<UndoReport, DbErr> {
    let mut report = UndoReport::default();
    let order = ["lease", "unit", "property", "owner", "vendor"];
    let mut kept_ids: HashSet<Uuid> = HashSet::new();
    for kind in order {
        let Some((table, cols)) = table_of(kind) else {
            continue;
        };
        let refs = referrers(db, table, cols).await?;
        for m in made.iter().rev().filter(|m| m.t == kind) {
            let mut why: Vec<String> = vec![];
            if edited_since(db, table, m.id, since).await? {
                why.push("edited since the import".into());
            }
            for (t, c) in &refs {
                if DERIVED.contains(&t.as_str()) && kind == "property" {
                    continue;
                }
                let n = count(db, t, c, m.id).await?;
                if n > 0 {
                    why.push(format!("has {}", label(t)));
                }
            }
            if !why.is_empty() {
                why.dedup();
                kept_ids.insert(m.id);
                report.kept.push(Kept {
                    t: kind.into(),
                    id: m.id,
                    reason: why.join(", "),
                });
                continue;
            }
            db.execute_unprepared("SAVEPOINT import_undo").await?;
            let mut ok = true;
            if kind == "property" {
                for d in DERIVED {
                    if refs.iter().any(|(t, _)| t == d) {
                        if let Err(e) = db
                            .execute(Statement::from_sql_and_values(
                                DbBackend::Postgres,
                                format!("DELETE FROM \"{d}\" WHERE property_id = $1"),
                                [Value::from(m.id)],
                            ))
                            .await
                        {
                            tracing::warn!("undo: {d}: {e}");
                            ok = false;
                            break;
                        }
                    }
                }
            }
            if ok {
                ok = db
                    .execute(Statement::from_sql_and_values(
                        DbBackend::Postgres,
                        format!("DELETE FROM \"{table}\" WHERE id = $1"),
                        [Value::from(m.id)],
                    ))
                    .await
                    .is_ok();
            }
            if ok {
                db.execute_unprepared("RELEASE SAVEPOINT import_undo")
                    .await?;
                *report.removed.entry(kind.to_string()).or_default() += 1;
            } else {
                db.execute_unprepared("ROLLBACK TO SAVEPOINT import_undo")
                    .await?;
                kept_ids.insert(m.id);
                report.kept.push(Kept {
                    t: kind.into(),
                    id: m.id,
                    reason: "couldn't be removed".into(),
                });
            }
        }
    }
    let _ = kept_ids;
    Ok(report)
}
