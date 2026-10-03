//! **Exports**: the workspace as CSV files, one per kind of record, or all of
//! them in one zip. The property, tenant, owner and vendor files use the
//! importer's own column names (and a `vantedge_id` column), so they go
//! straight back into another workspace, and open cleanly in a spreadsheet or
//! another tool's import.

use chrono::Utc;
use entity::prelude::{
    Counterparty, Lease, LedgerAccount, LedgerEntry, LedgerTxn, Llc, MaintenanceTicket, Owner,
    Property, Unit,
};
use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder};
use serde::Serialize;
use std::collections::HashMap;
use std::io::Write;
use uuid::Uuid;

#[derive(Serialize, Clone, Copy, schemars::JsonSchema)]
pub struct Dataset {
    pub key: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    /// Whether the importer reads this file back.
    pub importable: bool,
}

pub const DATASETS: &[Dataset] = &[
    Dataset {
        key: "properties",
        label: "Properties and units",
        description: "One row per unit, or per property for a single-family home.",
        importable: true,
    },
    Dataset {
        key: "tenants",
        label: "Tenants and leases",
        description: "Every lease with its rent, deposit, dates, balance and status.",
        importable: true,
    },
    Dataset {
        key: "owners",
        label: "Owners",
        description: "People and companies who own properties.",
        importable: true,
    },
    Dataset {
        key: "vendors",
        label: "Vendors",
        description: "Contractors with their trades and contact details.",
        importable: true,
    },
    Dataset {
        key: "work_orders",
        label: "Work orders",
        description: "Every maintenance ticket, its status, vendor and cost.",
        importable: false,
    },
    Dataset {
        key: "ledger",
        label: "General ledger",
        description: "Every journal line: date, entity, account, debit, credit.",
        importable: false,
    },
];

fn cell(s: &str) -> String {
    // A leading = + - @ makes spreadsheets run the cell as a formula.
    let s = if s.starts_with(['=', '+', '@']) || (s.starts_with('-') && s.parse::<f64>().is_err()) {
        format!("'{s}")
    } else {
        s.to_string()
    };
    if s.contains(['"', ',', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s
    }
}

fn csv(headers: &[&str], rows: Vec<Vec<String>>) -> String {
    let mut out = headers.join(",");
    out.push('\n');
    for r in rows {
        out.push_str(&r.iter().map(|c| cell(c)).collect::<Vec<_>>().join(","));
        out.push('\n');
    }
    out
}

fn dollars(cents: i64) -> String {
    format!(
        "{}{}.{:02}",
        if cents < 0 { "-" } else { "" },
        cents.abs() / 100,
        cents.abs() % 100
    )
}

fn opt<T: ToString>(v: Option<T>) -> String {
    v.map(|x| x.to_string()).unwrap_or_default()
}

/// One dataset as CSV text, and how many rows it has.
pub async fn dataset(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    key: &str,
) -> Result<Option<(String, usize)>, DbErr> {
    let props: HashMap<Uuid, entity::property::Model> = Property::find()
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?
        .into_iter()
        .map(|p| (p.id, p))
        .collect();
    let units: HashMap<Uuid, entity::unit::Model> = Unit::find()
        .filter(entity::unit::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?
        .into_iter()
        .map(|u| (u.id, u))
        .collect();
    let mut sorted_props: Vec<&entity::property::Model> = props.values().collect();
    sorted_props.sort_by(|a, b| a.name.cmp(&b.name));
    let rows: (Vec<&str>, Vec<Vec<String>>) = match key {
        "properties" => {
            let mut rows = vec![];
            for p in &sorted_props {
                let mut mine: Vec<&entity::unit::Model> =
                    units.values().filter(|u| u.property_id == p.id).collect();
                mine.sort_by(|a, b| a.unit_number.cmp(&b.unit_number));
                let base = |id: Uuid| {
                    vec![
                        id.to_string(),
                        p.name.clone(),
                        p.address.clone(),
                        p.city.clone(),
                        p.state.clone(),
                        p.postal_code.clone(),
                        p.property_type.clone(),
                        if p.year_built > 0 {
                            p.year_built.to_string()
                        } else {
                            String::new()
                        },
                    ]
                };
                if mine.is_empty() {
                    let mut r = base(p.id);
                    r.extend([
                        String::new(),
                        String::new(),
                        String::new(),
                        String::new(),
                        String::new(),
                    ]);
                    rows.push(r);
                }
                for u in mine {
                    let mut r = base(u.id);
                    r.extend([
                        u.unit_number.clone(),
                        opt(u.beds),
                        opt(u.baths),
                        opt(u.sqft),
                        u.market_rent_cents.map(dollars).unwrap_or_default(),
                    ]);
                    rows.push(r);
                }
            }
            (
                vec![
                    "vantedge_id",
                    "property",
                    "address",
                    "city",
                    "state",
                    "zip",
                    "property_type",
                    "year_built",
                    "unit",
                    "beds",
                    "baths",
                    "sqft",
                    "market_rent",
                ],
                rows,
            )
        }
        "tenants" => {
            let leases = Lease::find()
                .filter(entity::lease::Column::TenantId.eq(tenant_id))
                .order_by_asc(entity::lease::Column::StartDate)
                .all(db)
                .await?;
            let rows = leases
                .into_iter()
                .map(|l| {
                    let p = props.get(&l.property_id);
                    vec![
                        l.id.to_string(),
                        p.map(|p| p.name.clone()).unwrap_or_default(),
                        p.map(|p| p.address.clone()).unwrap_or_default(),
                        p.map(|p| p.city.clone()).unwrap_or_default(),
                        p.map(|p| p.state.clone()).unwrap_or_default(),
                        p.map(|p| p.postal_code.clone()).unwrap_or_default(),
                        l.unit_id
                            .and_then(|u| units.get(&u))
                            .map(|u| u.unit_number.clone())
                            .unwrap_or_default(),
                        l.tenant_name,
                        opt(l.tenant_email),
                        opt(l.tenant_phone),
                        dollars(l.rent_cents),
                        l.deposit_cents.map(dollars).unwrap_or_default(),
                        l.start_date,
                        opt(l.end_date),
                        dollars(l.balance_cents),
                        l.status,
                    ]
                })
                .collect();
            (
                vec![
                    "vantedge_id",
                    "property",
                    "address",
                    "city",
                    "state",
                    "zip",
                    "unit",
                    "tenant_name",
                    "email",
                    "phone",
                    "rent",
                    "deposit",
                    "lease_start",
                    "lease_end",
                    "balance",
                    "status",
                ],
                rows,
            )
        }
        "owners" => {
            let rows = Owner::find()
                .filter(entity::owner::Column::TenantId.eq(tenant_id))
                .order_by_asc(entity::owner::Column::Name)
                .all(db)
                .await?
                .into_iter()
                .filter(|o| o.kind != "firm")
                .map(|o| {
                    vec![
                        o.id.to_string(),
                        o.name.clone(),
                        if o.kind == "company" {
                            o.name
                        } else {
                            String::new()
                        },
                        opt(o.email),
                        opt(o.phone),
                        opt(o.notes),
                    ]
                })
                .collect();
            (
                vec!["vantedge_id", "name", "company", "email", "phone", "notes"],
                rows,
            )
        }
        "vendors" => {
            let rows = Counterparty::find()
                .filter(entity::counterparty::Column::TenantId.eq(tenant_id))
                .order_by_asc(entity::counterparty::Column::Name)
                .all(db)
                .await?
                .into_iter()
                .filter(|c| {
                    c.kind == "contractor" || c.trades.as_array().is_some_and(|a| !a.is_empty())
                })
                .map(|c| {
                    let trades: Vec<String> =
                        serde_json::from_value(c.trades.clone()).unwrap_or_default();
                    vec![
                        c.id.to_string(),
                        c.name,
                        opt(c.contact_name),
                        opt(c.email),
                        opt(c.phone),
                        opt(c.website),
                        opt(c.address),
                        trades
                            .iter()
                            .map(|t| crate::servicedesk::trade_label(t))
                            .collect::<Vec<_>>()
                            .join("; "),
                        opt(c.notes),
                    ]
                })
                .collect();
            (
                vec![
                    "vantedge_id",
                    "name",
                    "contact_name",
                    "email",
                    "phone",
                    "website",
                    "address",
                    "trades",
                    "notes",
                ],
                rows,
            )
        }
        "work_orders" => {
            let vendors: HashMap<Uuid, String> = Counterparty::find()
                .filter(entity::counterparty::Column::TenantId.eq(tenant_id))
                .all(db)
                .await?
                .into_iter()
                .map(|c| (c.id, c.name))
                .collect();
            let rows = MaintenanceTicket::find()
                .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
                .order_by_asc(entity::maintenance_ticket::Column::CreatedAt)
                .all(db)
                .await?
                .into_iter()
                .map(|t| {
                    vec![
                        t.id.to_string(),
                        props
                            .get(&t.property_id)
                            .map(|p| p.name.clone())
                            .unwrap_or_default(),
                        t.unit_id
                            .and_then(|u| units.get(&u))
                            .map(|u| u.unit_number.clone())
                            .unwrap_or_default(),
                        t.title,
                        t.category,
                        t.priority,
                        t.status,
                        t.created_at.date_naive().to_string(),
                        t.resolved_at
                            .map(|d| d.date_naive().to_string())
                            .unwrap_or_default(),
                        t.assignee_entity_id
                            .and_then(|v| vendors.get(&v).cloned())
                            .unwrap_or_default(),
                        t.cost_cents.map(dollars).unwrap_or_default(),
                        opt(t.description),
                    ]
                })
                .collect();
            (
                vec![
                    "vantedge_id",
                    "property",
                    "unit",
                    "title",
                    "category",
                    "priority",
                    "status",
                    "opened",
                    "resolved",
                    "vendor",
                    "cost",
                    "description",
                ],
                rows,
            )
        }
        "ledger" => {
            let accounts: HashMap<Uuid, entity::ledger_account::Model> = LedgerAccount::find()
                .filter(entity::ledger_account::Column::TenantId.eq(tenant_id))
                .all(db)
                .await?
                .into_iter()
                .map(|a| (a.id, a))
                .collect();
            let entities: HashMap<Uuid, String> = Llc::find()
                .filter(entity::llc::Column::TenantId.eq(tenant_id))
                .all(db)
                .await?
                .into_iter()
                .map(|l| (l.id, l.name))
                .collect();
            let txns: HashMap<Uuid, entity::ledger_txn::Model> = LedgerTxn::find()
                .filter(entity::ledger_txn::Column::TenantId.eq(tenant_id))
                .all(db)
                .await?
                .into_iter()
                .map(|t| (t.id, t))
                .collect();
            let mut entries = LedgerEntry::find()
                .filter(entity::ledger_entry::Column::TenantId.eq(tenant_id))
                .all(db)
                .await?;
            entries.sort_by(|a, b| {
                let da = txns
                    .get(&a.txn_id)
                    .map(|t| t.txn_date.as_str())
                    .unwrap_or("");
                let dbb = txns
                    .get(&b.txn_id)
                    .map(|t| t.txn_date.as_str())
                    .unwrap_or("");
                da.cmp(dbb)
                    .then(a.txn_id.cmp(&b.txn_id))
                    .then(a.side.cmp(&b.side).reverse())
            });
            let rows = entries
                .into_iter()
                .map(|e| {
                    let t = txns.get(&e.txn_id);
                    let a = accounts.get(&e.account_id);
                    let debit = e.side == "debit";
                    vec![
                        e.txn_id.to_string(),
                        t.map(|t| t.txn_date.clone()).unwrap_or_default(),
                        t.and_then(|t| entities.get(&t.entity_id).cloned())
                            .unwrap_or_default(),
                        a.map(|a| a.code.clone()).unwrap_or_default(),
                        a.map(|a| a.name.clone()).unwrap_or_default(),
                        if debit {
                            dollars(e.amount_cents)
                        } else {
                            String::new()
                        },
                        if debit {
                            String::new()
                        } else {
                            dollars(e.amount_cents)
                        },
                        t.map(|t| t.memo.clone()).unwrap_or_default(),
                        e.property_id
                            .and_then(|p| props.get(&p))
                            .map(|p| p.name.clone())
                            .unwrap_or_default(),
                        t.map(|t| t.source_type.clone()).unwrap_or_default(),
                    ]
                })
                .collect();
            (
                vec![
                    "transaction",
                    "date",
                    "entity",
                    "account_code",
                    "account",
                    "debit",
                    "credit",
                    "memo",
                    "property",
                    "source",
                ],
                rows,
            )
        }
        _ => return Ok(None),
    };
    let n = rows.1.len();
    Ok(Some((csv(&rows.0, rows.1), n)))
}

/// Every dataset in one zip, with a note on what's inside.
pub async fn bundle(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    workspace: &str,
) -> Result<Vec<u8>, DbErr> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let mut readme = format!(
            "{workspace}: data exported from Vantedge on {}.\n\n",
            Utc::now().format("%Y-%m-%d %H:%M UTC")
        );
        for d in DATASETS {
            if let Some((text, n)) = dataset(db, tenant_id, d.key).await? {
                readme.push_str(&format!(
                    "{}.csv: {} ({n} row{}). {}\n",
                    d.key,
                    d.label,
                    if n == 1 { "" } else { "s" },
                    d.description
                ));
                let _ = zip.start_file(format!("{}.csv", d.key), opts);
                // Excel reads UTF-8 CSV correctly with the byte-order mark.
                let _ = zip.write_all(b"\xEF\xBB\xBF");
                let _ = zip.write_all(text.as_bytes());
            }
        }
        readme.push_str(
            "\nProperties, tenants, owners and vendors use the column names Vantedge's \
             importer reads, so these files can be imported into another workspace as they are.\n\
             Amounts are in dollars; dates are YYYY-MM-DD.\n",
        );
        let _ = zip.start_file("README.txt", opts);
        let _ = zip.write_all(readme.as_bytes());
        let _ = zip.finish();
    }
    Ok(buf.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_are_safe_in_a_spreadsheet() {
        assert_eq!(cell("=HYPERLINK(\"x\")"), "\"'=HYPERLINK(\"\"x\"\")\"");
        assert_eq!(cell("-45.10"), "-45.10");
        assert_eq!(cell("Smith, John"), "\"Smith, John\"");
        assert_eq!(dollars(-4510), "-45.10");
        assert_eq!(dollars(125000), "1250.00");
    }

    #[test]
    fn exported_headers_map_back_exactly() {
        let headers: Vec<String> = [
            "vantedge_id",
            "property",
            "address",
            "city",
            "state",
            "zip",
            "unit",
            "tenant_name",
            "email",
            "phone",
            "rent",
            "deposit",
            "lease_start",
            "lease_end",
            "balance",
            "status",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(super::super::detect_source(&headers), "vantedge");
        let m = super::super::auto_map("tenants", &headers);
        for f in [
            "property",
            "unit",
            "tenant_name",
            "rent",
            "lease_start",
            "lease_end",
            "balance",
        ] {
            assert_eq!(m.get(f).map(String::as_str), Some(f), "{f}");
        }
    }
}
