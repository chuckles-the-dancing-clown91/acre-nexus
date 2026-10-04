//! Running an import: each row becomes (or finds) properties, units, leases,
//! owners or vendors. The same code serves the preview and the commit; a
//! preview runs inside a savepoint that's rolled back, so what it reports is
//! what a commit does. Each row has its own savepoint, so one bad row is
//! reported and the rest go in.

use super::{
    bd_ba, cell, date, decimal, email, int, is_company, lease_status, money, norm, person, trades,
    Table,
};
use chrono::Utc;
use entity::prelude::{Counterparty, Lease, Owner, Property, Unit};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, Set,
};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

/// What happened to one row.
#[derive(Serialize, Clone, schemars::JsonSchema)]
pub struct RowOutcome {
    /// The row's line in the file.
    pub line: usize,
    /// `created` | `updated` | `matched` | `skipped` | `error`.
    pub action: &'static str,
    /// What the row is about, e.g. "Maple Ct · Unit 2 · John Smith".
    pub label: String,
    pub message: String,
}

/// Something the import made, kept on the batch for undo.
#[derive(Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Made {
    /// `property` | `unit` | `lease` | `owner` | `vendor`.
    pub t: String,
    pub id: Uuid,
}

#[derive(Serialize, Default, Clone, schemars::JsonSchema)]
pub struct Counts {
    pub rows: usize,
    pub properties: usize,
    pub units: usize,
    pub leases: usize,
    pub owners: usize,
    pub vendors: usize,
    pub updated: usize,
    pub matched: usize,
    pub skipped: usize,
    pub errors: usize,
}

pub struct Outcome {
    pub rows: Vec<RowOutcome>,
    pub made: Vec<Made>,
    pub counts: Counts,
}

/// Where the rows came from, for the notes on what they make.
pub struct Ctx<'a> {
    pub tenant_id: Uuid,
    pub source_label: &'a str,
    pub today: String,
}

/// A problem with one row: reported, the row skipped.
struct RowErr(String);

impl From<DbErr> for RowErr {
    fn from(e: DbErr) -> Self {
        RowErr(format!("couldn't save: {e}"))
    }
}

fn row_err<T>(m: impl Into<String>) -> Result<T, RowErr> {
    Err(RowErr(m.into()))
}

/// What's already in the workspace, so rows match instead of duplicating.
#[derive(Default)]
struct Known {
    /// Property keys (name, address, address+city) → id.
    properties: HashMap<String, Uuid>,
    /// (property, unit) → id.
    units: HashMap<(Uuid, String), Uuid>,
    /// (property, unit, tenant, start) of every lease.
    leases: HashSet<(Uuid, Option<Uuid>, String, String)>,
    owners: HashMap<String, Uuid>,
    vendors: HashMap<String, Uuid>,
    /// Properties this run touched: unit counts and occupancy are redone.
    touched: HashSet<Uuid>,
    /// Properties this run made: their rent total is filled in.
    new_properties: HashSet<Uuid>,
}

fn prop_keys(name: &str, address: &str, city: &str) -> Vec<String> {
    let mut k = vec![];
    if !address.is_empty() {
        k.push(format!("a:{}|{}", norm(address), norm(city)));
        k.push(format!("a:{}", norm(address)));
    }
    if !name.is_empty() {
        k.push(format!("n:{}", norm(name)));
    }
    k
}

async fn load(db: &impl ConnectionTrait, tenant_id: Uuid, kind: &str) -> Result<Known, DbErr> {
    let mut k = Known::default();
    if kind == "properties" || kind == "tenants" {
        for p in Property::find()
            .filter(entity::property::Column::TenantId.eq(tenant_id))
            .all(db)
            .await?
        {
            for key in prop_keys(&p.name, &p.address, &p.city) {
                k.properties.entry(key).or_insert(p.id);
            }
        }
        for u in Unit::find()
            .filter(entity::unit::Column::TenantId.eq(tenant_id))
            .all(db)
            .await?
        {
            k.units.insert((u.property_id, norm(&u.unit_number)), u.id);
        }
        for l in Lease::find()
            .filter(entity::lease::Column::TenantId.eq(tenant_id))
            .all(db)
            .await?
        {
            k.leases
                .insert((l.property_id, l.unit_id, norm(&l.tenant_name), l.start_date));
        }
    }
    if kind == "owners" {
        for o in Owner::find()
            .filter(entity::owner::Column::TenantId.eq(tenant_id))
            .all(db)
            .await?
        {
            if let Some(e) = &o.email {
                k.owners.insert(format!("e:{}", e.to_lowercase()), o.id);
            }
            k.owners.insert(format!("n:{}", norm(&o.name)), o.id);
        }
    }
    if kind == "vendors" {
        for c in Counterparty::find()
            .filter(entity::counterparty::Column::TenantId.eq(tenant_id))
            .all(db)
            .await?
        {
            k.vendors.insert(norm(&c.name), c.id);
        }
    }
    Ok(k)
}

/// Run the import. With `commit` false, every write is rolled back at the end
/// and only the report is kept.
pub async fn run(
    db: &impl ConnectionTrait,
    ctx: &Ctx<'_>,
    kind: &str,
    table: &Table,
    mapping: &BTreeMap<String, String>,
    commit: bool,
) -> Result<Outcome, DbErr> {
    db.execute_unprepared("SAVEPOINT import_run").await?;
    let result = run_inner(db, ctx, kind, table, mapping).await;
    match (&result, commit) {
        (Ok(_), true) => {
            db.execute_unprepared("RELEASE SAVEPOINT import_run")
                .await?
        }
        _ => {
            db.execute_unprepared("ROLLBACK TO SAVEPOINT import_run")
                .await?
        }
    };
    result
}

async fn run_inner(
    db: &impl ConnectionTrait,
    ctx: &Ctx<'_>,
    kind: &str,
    table: &Table,
    mapping: &BTreeMap<String, String>,
) -> Result<Outcome, DbErr> {
    let mut known = load(db, ctx.tenant_id, kind).await?;
    let mut out = Outcome {
        rows: vec![],
        made: vec![],
        counts: Counts {
            rows: table.rows.len(),
            ..Default::default()
        },
    };
    for (i, row) in table.rows.iter().enumerate() {
        let line = table.lines.get(i).copied().unwrap_or(i + 2);
        db.execute_unprepared("SAVEPOINT import_row").await?;
        let mut made = vec![];
        let r = match kind {
            "properties" => property_row(db, ctx, table, row, mapping, &mut known, &mut made).await,
            "tenants" => tenant_row(db, ctx, table, row, mapping, &mut known, &mut made).await,
            "owners" => owner_row(db, ctx, table, row, mapping, &mut known, &mut made).await,
            "vendors" => vendor_row(db, ctx, table, row, mapping, &mut known, &mut made).await,
            _ => row_err("unknown import kind"),
        };
        match r {
            Ok((action, label, message)) => {
                db.execute_unprepared("RELEASE SAVEPOINT import_row")
                    .await?;
                for m in &made {
                    match m.t.as_str() {
                        "property" => out.counts.properties += 1,
                        "unit" => out.counts.units += 1,
                        "lease" => out.counts.leases += 1,
                        "owner" => out.counts.owners += 1,
                        "vendor" => out.counts.vendors += 1,
                        _ => {}
                    }
                }
                match action {
                    "updated" => out.counts.updated += 1,
                    "matched" => out.counts.matched += 1,
                    "skipped" => out.counts.skipped += 1,
                    _ => {}
                }
                out.made.extend(made);
                out.rows.push(RowOutcome {
                    line,
                    action,
                    label,
                    message,
                });
            }
            Err(RowErr(message)) => {
                db.execute_unprepared("ROLLBACK TO SAVEPOINT import_row")
                    .await?;
                // The row's writes are gone; so is anything it added to what's known.
                if !made.is_empty() {
                    let gone = |id: &Uuid| made.iter().any(|m| m.id == *id);
                    let mut touched = std::mem::take(&mut known.touched);
                    let mut fresh = std::mem::take(&mut known.new_properties);
                    touched.retain(|p| !gone(p));
                    fresh.retain(|p| !gone(p));
                    known = load(db, ctx.tenant_id, kind).await?;
                    known.touched = touched;
                    known.new_properties = fresh;
                }
                out.counts.errors += 1;
                out.rows.push(RowOutcome {
                    line,
                    action: "error",
                    label: row_label(table, row, mapping, kind),
                    message,
                });
            }
        }
    }
    finish_properties(db, &known).await?;
    Ok(out)
}

fn row_label(t: &Table, row: &[String], m: &BTreeMap<String, String>, kind: &str) -> String {
    let parts: Vec<String> = match kind {
        "properties" | "tenants" => {
            let mut p = vec![];
            if let Some(x) = cell(t, row, m, "property").or_else(|| cell(t, row, m, "address")) {
                p.push(x.to_string());
            }
            if let Some(u) = cell(t, row, m, "unit") {
                p.push(format!("Unit {u}"));
            }
            if let Some(n) = cell(t, row, m, "tenant_name") {
                p.push(person(n));
            }
            p
        }
        _ => ["name", "company", "last_name"]
            .iter()
            .filter_map(|f| cell(t, row, m, f))
            .take(1)
            .map(str::to_string)
            .collect(),
    };
    if parts.is_empty() {
        "(blank)".into()
    } else {
        parts.join(" · ")
    }
}

type RowResult = Result<(&'static str, String, String), RowErr>;

/// The property a row names, made if it's new.
async fn property_for(
    db: &impl ConnectionTrait,
    ctx: &Ctx<'_>,
    t: &Table,
    row: &[String],
    m: &BTreeMap<String, String>,
    known: &mut Known,
    made: &mut Vec<Made>,
) -> Result<(Uuid, String, bool), RowErr> {
    let name = cell(t, row, m, "property").unwrap_or("");
    let address = cell(t, row, m, "address").unwrap_or(name);
    let name = if name.is_empty() { address } else { name };
    if name.is_empty() {
        return row_err("no property name or address");
    }
    let city = cell(t, row, m, "city").unwrap_or("");
    if let Some(id) = prop_keys(name, address, city)
        .iter()
        .find_map(|k| known.properties.get(k))
    {
        known.touched.insert(*id);
        return Ok((*id, name.to_string(), false));
    }
    let id = Uuid::new_v4();
    entity::property::ActiveModel {
        id: Set(id),
        tenant_id: Set(ctx.tenant_id),
        llc_id: Set(None),
        portfolio_id: Set(None),
        name: Set(name.to_string()),
        address: Set(address.to_string()),
        city: Set(city.to_string()),
        units: Set(0),
        occupied_units: Set(0),
        monthly_rent_cents: Set(0),
        status: Set("Stabilized".into()),
        year_built: Set(cell(t, row, m, "year_built").and_then(int).unwrap_or(0)),
        manager: Set(String::new()),
        property_type: Set(cell(t, row, m, "property_type").unwrap_or("").to_string()),
        strategy: Set("rental".into()),
        workflow_stage: Set(String::new()),
        purchase_price_cents: Set(None),
        acquired_on: Set(None),
        image_url: Set(None),
        state: Set(crate::geo::state_code(
            cell(t, row, m, "state").unwrap_or(""),
        )),
        postal_code: Set(cell(t, row, m, "zip").unwrap_or("").to_string()),
        photo_status: Set("none".into()),
        photo_attempted_at: Set(None),
        photo_error: Set(None),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await?;
    crate::geo::queue_fetch(db, ctx.tenant_id, id).await;
    for k in prop_keys(name, address, city) {
        known.properties.entry(k).or_insert(id);
    }
    known.touched.insert(id);
    known.new_properties.insert(id);
    made.push(Made {
        t: "property".into(),
        id,
    });
    Ok((id, name.to_string(), true))
}

/// The unit a row names on a property, made if it's new. No unit column (or
/// a blank one) means the whole property, as with a single-family home.
#[allow(clippy::too_many_arguments)]
async fn unit_for(
    db: &impl ConnectionTrait,
    ctx: &Ctx<'_>,
    t: &Table,
    row: &[String],
    m: &BTreeMap<String, String>,
    property_id: Uuid,
    known: &mut Known,
    made: &mut Vec<Made>,
) -> Result<Option<(Uuid, String, bool)>, RowErr> {
    let Some(unit) = cell(t, row, m, "unit") else {
        return Ok(None);
    };
    let key = (property_id, norm(unit));
    if let Some(id) = known.units.get(&key) {
        return Ok(Some((*id, unit.to_string(), false)));
    }
    let (mut beds, mut baths) = cell(t, row, m, "bd_ba").map(bd_ba).unwrap_or((None, None));
    if let Some(b) = cell(t, row, m, "beds").and_then(int) {
        beds = Some(b);
    }
    if let Some(b) = cell(t, row, m, "baths").and_then(decimal) {
        baths = Some(b);
    }
    let rent = cell(t, row, m, "market_rent")
        .and_then(money)
        .filter(|c| *c > 0);
    let id = Uuid::new_v4();
    let now = Utc::now();
    entity::unit::ActiveModel {
        id: Set(id),
        tenant_id: Set(ctx.tenant_id),
        property_id: Set(property_id),
        unit_number: Set(unit.to_string()),
        beds: Set(beds),
        baths: Set(baths),
        sqft: Set(cell(t, row, m, "sqft").and_then(int).filter(|s| *s > 0)),
        market_rent_cents: Set(rent),
        status: Set("vacant".into()),
        floor: Set(None),
        notes: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    known.units.insert(key, id);
    made.push(Made {
        t: "unit".into(),
        id,
    });
    Ok(Some((id, unit.to_string(), true)))
}

async fn property_row(
    db: &impl ConnectionTrait,
    ctx: &Ctx<'_>,
    t: &Table,
    row: &[String],
    m: &BTreeMap<String, String>,
    known: &mut Known,
    made: &mut Vec<Made>,
) -> RowResult {
    let (pid, pname, new_p) = property_for(db, ctx, t, row, m, known, made).await?;
    let unit = unit_for(db, ctx, t, row, m, pid, known, made).await?;
    let label = match &unit {
        Some((_, u, _)) => format!("{pname} · Unit {u}"),
        None => pname.clone(),
    };
    Ok(match (new_p, &unit) {
        (true, Some(_)) => ("created", label, "New property and unit".into()),
        (true, None) => ("created", label, "New property".into()),
        (false, Some((_, _, true))) => {
            ("created", label, "New unit on an existing property".into())
        }
        _ => ("matched", label, "Already here".into()),
    })
}

async fn tenant_row(
    db: &impl ConnectionTrait,
    ctx: &Ctx<'_>,
    t: &Table,
    row: &[String],
    m: &BTreeMap<String, String>,
    known: &mut Known,
    made: &mut Vec<Made>,
) -> RowResult {
    let name = match cell(t, row, m, "tenant_name") {
        Some(n) => person(n),
        None => [cell(t, row, m, "first_name"), cell(t, row, m, "last_name")]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" "),
    };
    let vacant = name.is_empty()
        || matches!(
            norm(&name).as_str(),
            "vacant" | "vacantunit" | "model" | "down"
        );
    let (pid, pname, _) = property_for(db, ctx, t, row, m, known, made).await?;
    let unit = unit_for(db, ctx, t, row, m, pid, known, made).await?;
    let place = match &unit {
        Some((_, u, _)) => format!("{pname} · Unit {u}"),
        None => pname.clone(),
    };
    if vacant {
        return Ok(if made.is_empty() {
            ("skipped", place, "Vacant, nothing to add".into())
        } else {
            ("created", place, "Vacant unit".into())
        });
    }
    let label = format!("{place} · {name}");
    let Some(rent) = cell(t, row, m, "rent").and_then(money) else {
        return row_err("no rent");
    };
    if rent < 0 {
        return row_err("rent is negative");
    }
    let start = cell(t, row, m, "lease_start")
        .and_then(date)
        .or_else(|| cell(t, row, m, "move_in").and_then(date));
    let Some(start) = start else {
        return row_err(
            match cell(t, row, m, "lease_start").or(cell(t, row, m, "move_in")) {
                Some(raw) => format!("can't read the lease start \"{raw}\""),
                None => "no lease start or move-in date".into(),
            },
        );
    };
    let end = cell(t, row, m, "lease_end").and_then(date);
    if let Some(raw) = cell(t, row, m, "lease_end") {
        if end.is_none() && !norm(raw).contains("month") && norm(raw) != "mtm" {
            return row_err(format!("can't read the lease end \"{raw}\""));
        }
    }
    let unit_id = unit.as_ref().map(|u| u.0);
    let key = (pid, unit_id, norm(&name), start.clone());
    if known.leases.contains(&key) {
        return Ok(("matched", label, "This lease is already here".into()));
    }
    let balance = cell(t, row, m, "balance").and_then(money).unwrap_or(0);
    let status = lease_status(cell(t, row, m, "status"), end.as_deref(), &ctx.today);
    let id = Uuid::new_v4();
    let now = Utc::now();
    entity::lease::ActiveModel {
        id: Set(id),
        tenant_id: Set(ctx.tenant_id),
        property_id: Set(pid),
        unit_id: Set(unit_id),
        application_id: Set(None),
        tenant_name: Set(name.clone()),
        tenant_email: Set(cell(t, row, m, "email").and_then(email)),
        tenant_phone: Set(cell(t, row, m, "phone").map(str::to_string)),
        rent_cents: Set(rent),
        deposit_cents: Set(cell(t, row, m, "deposit")
            .and_then(money)
            .filter(|c| *c > 0)),
        start_date: Set(start),
        end_date: Set(end),
        status: Set(status.into()),
        payment_status: Set(if balance > 0 { "late" } else { "current" }.into()),
        balance_cents: Set(balance),
        has_pet: Set(false),
        pet_details: Set(None),
        is_military: Set(false),
        notes: Set(Some(format!(
            "Imported from {} on {}.",
            ctx.source_label, ctx.today
        ))),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    known.leases.insert(key);
    made.push(Made {
        t: "lease".into(),
        id,
    });
    let mut message = match status {
        "active" => "Lease".to_string(),
        s => format!("Lease ({s})"),
    };
    if balance != 0 {
        message.push_str(&format!(", balance {}", crate::dto::usd(balance)));
    }
    Ok(("created", label, message))
}

async fn owner_row(
    db: &impl ConnectionTrait,
    ctx: &Ctx<'_>,
    t: &Table,
    row: &[String],
    m: &BTreeMap<String, String>,
    known: &mut Known,
    made: &mut Vec<Made>,
) -> RowResult {
    let company = cell(t, row, m, "company");
    let name = cell(t, row, m, "name")
        .map(person)
        .or_else(|| {
            let n = [cell(t, row, m, "first_name"), cell(t, row, m, "last_name")]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ");
            (!n.is_empty()).then_some(n)
        })
        .or_else(|| company.map(str::to_string));
    let Some(name) = name else {
        return row_err("no owner name");
    };
    let mail = cell(t, row, m, "email").and_then(email);
    if let Some(id) = mail
        .as_ref()
        .and_then(|e| known.owners.get(&format!("e:{e}")))
        .or_else(|| known.owners.get(&format!("n:{}", norm(&name))))
    {
        let _ = id;
        return Ok(("matched", name, "Already an owner".into()));
    }
    let kind = if company.is_some() || is_company(&name) {
        "company"
    } else {
        "individual"
    };
    let id = Uuid::new_v4();
    entity::owner::ActiveModel {
        id: Set(id),
        tenant_id: Set(ctx.tenant_id),
        kind: Set(kind.into()),
        name: Set(name.clone()),
        email: Set(mail.clone()),
        phone: Set(cell(t, row, m, "phone").map(str::to_string)),
        notes: Set(cell(t, row, m, "notes").map(str::to_string)),
        created_at: Set(Utc::now().into()),
        user_id: Set(None),
        approval_limit_cents: Set(None),
    }
    .insert(db)
    .await?;
    if let Some(e) = mail {
        known.owners.insert(format!("e:{e}"), id);
    }
    known.owners.insert(format!("n:{}", norm(&name)), id);
    made.push(Made {
        t: "owner".into(),
        id,
    });
    Ok(("created", name, format!("Owner ({kind})")))
}

async fn vendor_row(
    db: &impl ConnectionTrait,
    ctx: &Ctx<'_>,
    t: &Table,
    row: &[String],
    m: &BTreeMap<String, String>,
    known: &mut Known,
    made: &mut Vec<Made>,
) -> RowResult {
    let Some(name) = cell(t, row, m, "name") else {
        return row_err("no vendor name");
    };
    let name = name.to_string();
    let found = cell(t, row, m, "trades").map(trades).unwrap_or_default();
    let mail = cell(t, row, m, "email").and_then(email);
    let phone = cell(t, row, m, "phone").map(str::to_string);
    if let Some(id) = known.vendors.get(&norm(&name)).copied() {
        // Fill in what the vendor's record is missing; never overwrite.
        let Some(c) = Counterparty::find_by_id(id).one(db).await? else {
            return Ok(("matched", name, "Already a vendor".into()));
        };
        let have: Vec<String> = serde_json::from_value(c.trades.clone()).unwrap_or_default();
        let mut am: entity::counterparty::ActiveModel = c.clone().into();
        let mut filled = vec![];
        if c.email.is_none() && mail.is_some() {
            am.email = Set(mail);
            filled.push("email");
        }
        if c.phone.is_none() && phone.is_some() {
            am.phone = Set(phone);
            filled.push("phone");
        }
        if have.is_empty() && !found.is_empty() {
            am.trades = Set(serde_json::json!(found));
            filled.push("trades");
        }
        if filled.is_empty() {
            return Ok(("matched", name, "Already a vendor".into()));
        }
        am.updated_at = Set(Utc::now().into());
        am.update(db).await?;
        return Ok((
            "updated",
            name,
            format!("Already a vendor; added {}", filled.join(", ")),
        ));
    }
    let id = Uuid::new_v4();
    let now = Utc::now();
    entity::counterparty::ActiveModel {
        id: Set(id),
        tenant_id: Set(ctx.tenant_id),
        kind: Set("contractor".into()),
        name: Set(name.clone()),
        contact_name: Set(cell(t, row, m, "contact_name").map(person)),
        email: Set(mail),
        phone: Set(phone),
        website: Set(cell(t, row, m, "website").map(str::to_string)),
        address: Set(cell(t, row, m, "address").map(str::to_string)),
        notes: Set(cell(t, row, m, "notes").map(str::to_string)),
        partner_kind: Set(None),
        trades: Set(serde_json::json!(found)),
        partner_base_url: Set(None),
        partner_web_url: Set(None),
        partner_linked_at: Set(None),
        partner_status: Set(None),
        partner_error: Set(None),
        related_llc_id: Set(None),
        related_owner_id: Set(None),
        alpha_invited_at: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    known.vendors.insert(norm(&name), id);
    made.push(Made {
        t: "vendor".into(),
        id,
    });
    let message = if found.is_empty() {
        "Vendor".to_string()
    } else {
        format!(
            "Vendor: {}",
            found
                .iter()
                .map(|t| crate::servicedesk::trade_label(t))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    Ok(("created", name, message))
}

/// Unit counts, occupancy and (for new properties) the rent total, once all
/// the rows are in.
async fn finish_properties(db: &impl ConnectionTrait, known: &Known) -> Result<(), DbErr> {
    for pid in &known.touched {
        let Some(p) = Property::find_by_id(*pid).one(db).await? else {
            continue;
        };
        let units = Unit::find()
            .filter(entity::unit::Column::PropertyId.eq(*pid))
            .all(db)
            .await?;
        let leases = Lease::find()
            .filter(entity::lease::Column::PropertyId.eq(*pid))
            .filter(entity::lease::Column::Status.is_in(["active", "notice"]))
            .all(db)
            .await?;
        let count = (units.len() as i32).max(if units.is_empty() { 1 } else { 0 });
        let mut am: entity::property::ActiveModel = p.clone().into();
        let mut changed = false;
        if count > p.units {
            am.units = Set(count);
            changed = true;
        }
        if known.new_properties.contains(pid) {
            let rent: i64 = if leases.is_empty() {
                units.iter().filter_map(|u| u.market_rent_cents).sum()
            } else {
                leases.iter().map(|l| l.rent_cents).sum()
            };
            am.monthly_rent_cents = Set(rent);
            if leases.is_empty() {
                am.status = Set("Vacant".into());
            }
            changed = true;
        }
        if changed {
            am.update(db).await?;
        }
        crate::rentals_occupancy::sync_property_occupancy(db, *pid).await;
    }
    Ok(())
}
