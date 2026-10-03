//! **Service desk** rules: job kits, trades, and estimates.
//!
//! A job kit is an issue-catalog entry with the work broken into tasks (each
//! with its trade, time, and whether it needs a contractor) and the parts it
//! takes, with typical costs. Picking "Shower replacement" opens a work order
//! that already lists the demo, the plumbing, the drywall and the tile, and a
//! shopping list for the valve, cement board and caulk. Routine maintenance can
//! carry a kit too, so every filter change starts with its filter.

use crate::error::ApiResult;
use crate::kit_catalog::KITS;
use crate::settings as cfg;
use chrono::Utc;
use entity::prelude::{InventoryItem, IssueTemplate, TicketTask};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

/// The trades a task can call for.
pub const TRADES: &[&str] = &[
    "general",
    "demo",
    "carpentry",
    "plumbing",
    "electrical",
    "hvac",
    "drywall",
    "paint",
    "tile",
    "flooring",
    "roofing",
    "appliance",
    "landscaping",
    "pest",
    "cleaning",
    "exterior",
];

pub const TASK_STATUSES: &[&str] = &["todo", "doing", "done", "skipped"];

/// One line of work in a kit.
#[derive(Serialize, Deserialize, Clone, Debug, schemars::JsonSchema)]
pub struct KitTask {
    pub title: String,
    #[serde(default = "general")]
    pub trade: String,
    pub est_minutes: Option<i32>,
    #[serde(default)]
    pub needs_contractor: bool,
}

fn general() -> String {
    "general".into()
}

/// Clean a kit's tasks: drop blanks, keep known trades, sane minutes.
pub fn clean_tasks(tasks: Vec<KitTask>) -> Vec<KitTask> {
    tasks
        .into_iter()
        .filter(|t| !t.title.trim().is_empty())
        .map(|t| KitTask {
            title: t.title.trim().to_string(),
            trade: if TRADES.contains(&t.trade.trim()) {
                t.trade.trim().to_string()
            } else {
                "general".into()
            },
            est_minutes: t.est_minutes.filter(|m| *m > 0 && *m <= 60 * 24 * 14),
            needs_contractor: t.needs_contractor,
        })
        .collect()
}

/// "hvac" → "HVAC", "plumbing" → "Plumbing".
pub fn trade_label(t: &str) -> String {
    if t == "hvac" {
        return "HVAC".into();
    }
    let mut c = t.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

/// Keep only known trades, once each, in the given order.
pub fn clean_trades(raw: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for t in raw {
        let t = t.trim().to_lowercase();
        if TRADES.contains(&t.as_str()) && !out.contains(&t) {
            out.push(t);
        }
    }
    out
}

/// Hourly rates used for estimates, in cents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rates {
    pub in_house: i64,
    pub contractor: i64,
}

pub async fn rates(db: &impl ConnectionTrait, tenant_id: Uuid) -> Rates {
    Rates {
        in_house: cfg::get_i64(db, tenant_id, cfg::MAINTENANCE_LABOR_RATE)
            .await
            .max(0),
        contractor: cfg::get_i64(db, tenant_id, cfg::MAINTENANCE_CONTRACTOR_RATE)
            .await
            .max(0),
    }
}

/// What a task should cost, from its time and who does it.
pub fn task_cost(minutes: Option<i32>, needs_contractor: bool, rates: Rates) -> Option<i64> {
    let m = minutes? as i64;
    let rate = if needs_contractor {
        rates.contractor
    } else {
        rates.in_house
    };
    Some((m * rate + 30) / 60)
}

/// The trades a set of tasks calls for, in first-seen order.
pub fn trades_of(tasks: &[KitTask]) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for t in tasks {
        if !out.contains(&t.trade) {
            out.push(t.trade.clone());
        }
    }
    out
}

// ---------------------------------------------------------------------------
// The starter kits: crate::kit_catalog
// ---------------------------------------------------------------------------

/// Give a workspace every catalog kit it hasn't had yet, by key. A kit the
/// workspace renamed, changed or retired keeps its key, so it isn't added
/// back; a kit new in this release is. A name the workspace already uses
/// (its own kit) is left alone.
pub async fn ensure_kits(db: &impl ConnectionTrait, tenant_id: Uuid) -> ApiResult<()> {
    let rows = IssueTemplate::find()
        .filter(entity::issue_template::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?;
    let keys: Vec<String> = rows.iter().filter_map(|t| t.kit_key.clone()).collect();
    let names: Vec<String> = rows.iter().map(|t| t.name.to_lowercase()).collect();
    let now = Utc::now();
    for k in KITS {
        if keys.iter().any(|x| x == k.key) || names.contains(&k.name.to_lowercase()) {
            continue;
        }
        let tasks: Vec<KitTask> = k
            .tasks
            .iter()
            .map(|(title, trade, minutes, contractor)| KitTask {
                title: title.to_string(),
                trade: trade.to_string(),
                est_minutes: Some(*minutes),
                needs_contractor: *contractor,
            })
            .collect();
        let parts: Vec<serde_json::Value> = k
            .parts
            .iter()
            .map(|(name, qty, cost)| {
                json!({ "name": name, "quantity": qty, "inventory_item_id": null,
                        "unit_cost_cents": cost })
            })
            .collect();
        let minutes: i32 = k.tasks.iter().map(|t| t.2).sum();
        entity::issue_template::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            name: Set(k.name.into()),
            area: Set(Some(k.area.into())),
            category: Set(k.category.into()),
            priority: Set(k.priority.into()),
            description: Set(Some(k.description.into())),
            est_minutes: Set(Some(minutes)),
            checklist: Set(json!([])),
            parts: Set(json!(parts)),
            tasks: Set(json!(tasks)),
            active: Set(true),
            seeded: Set(true),
            kit_key: Set(Some(k.key.into())),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

/// A buy link as entered: `http(s)://…`, at most 1,000 characters; blank is
/// none.
pub fn clean_url(raw: &str) -> Result<Option<String>, String> {
    let t = raw.trim();
    if t.is_empty() {
        return Ok(None);
    }
    if !(t.starts_with("https://") || t.starts_with("http://")) || t.len() > 1000 || t.contains(' ')
    {
        return Err("a part link must be a web address (https://…)".into());
    }
    Ok(Some(t.to_string()))
}

/// The store a product link is at, by its domain.
pub fn store_of(url: &str) -> Option<&'static str> {
    let host = url
        .split("://")
        .nth(1)?
        .split(['/', '?', '#'])
        .next()?
        .to_lowercase();
    let host = host.trim_start_matches("www.");
    const STORES: &[(&str, &str)] = &[
        ("homedepot.com", "Home Depot"),
        ("lowes.com", "Lowe's"),
        ("amazon.com", "Amazon"),
        ("amzn.to", "Amazon"),
        ("a.co", "Amazon"),
        ("walmart.com", "Walmart"),
        ("menards.com", "Menards"),
        ("acehardware.com", "Ace Hardware"),
        ("grainger.com", "Grainger"),
        ("supplyhouse.com", "SupplyHouse"),
        ("ferguson.com", "Ferguson"),
        ("repairclinic.com", "RepairClinic"),
        ("appliancepartspros.com", "AppliancePartsPros"),
        ("partselect.com", "PartSelect"),
        ("build.com", "Build.com"),
        ("zoro.com", "Zoro"),
    ];
    STORES
        .iter()
        .find(|(d, _)| host == *d || host.ends_with(&format!(".{d}")))
        .map(|(_, n)| *n)
}

/// A kit's part line as stored.
#[derive(Deserialize, Clone)]
struct StoredPart {
    name: String,
    #[serde(default)]
    quantity: i32,
    inventory_item_id: Option<Uuid>,
    unit_cost_cents: Option<i64>,
    #[serde(default)]
    url: Option<String>,
}

/// Put a kit's tasks and parts on a work order (after anything already there).
/// Parts match stock by id or name; unmatched parts carry the kit's typical
/// cost so the estimate is real. Returns how many tasks and parts were added.
pub async fn apply_kit(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    ticket_id: Uuid,
    kit: &entity::issue_template::Model,
    user: Option<Uuid>,
) -> ApiResult<(usize, usize)> {
    let rates = rates(db, tenant_id).await;
    let tasks: Vec<KitTask> =
        clean_tasks(serde_json::from_value(kit.tasks.clone()).unwrap_or_default());
    let start = TicketTask::find()
        .filter(entity::ticket_task::Column::TenantId.eq(tenant_id))
        .filter(entity::ticket_task::Column::TicketId.eq(ticket_id))
        .order_by_desc(entity::ticket_task::Column::Position)
        .one(db)
        .await?
        .map(|t| t.position + 1)
        .unwrap_or(0);
    let now = Utc::now();
    for (i, t) in tasks.iter().enumerate() {
        entity::ticket_task::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            ticket_id: Set(ticket_id),
            position: Set(start + i as i32),
            title: Set(t.title.clone()),
            trade: Set(t.trade.clone()),
            est_minutes: Set(t.est_minutes),
            est_cost_cents: Set(task_cost(t.est_minutes, t.needs_contractor, rates)),
            needs_contractor: Set(t.needs_contractor),
            assignee_entity_id: Set(None),
            status: Set("todo".into()),
            done_at: Set(None),
            done_by: Set(None),
            dispatched_at: Set(None),
            assignee_user_id: Set(None),
            dispatch_via: Set(None),
            dispatch_note: Set(None),
            created_by: Set(user),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(db)
        .await?;
    }

    let stock = InventoryItem::find()
        .filter(entity::inventory_item::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?;
    let parts: Vec<StoredPart> = serde_json::from_value(kit.parts.clone()).unwrap_or_default();
    for p in &parts {
        let item = p
            .inventory_item_id
            .and_then(|i| stock.iter().find(|s| s.id == i))
            .or_else(|| {
                stock
                    .iter()
                    .find(|s| s.name.trim().eq_ignore_ascii_case(p.name.trim()))
            });
        let added = crate::routes::maintenance::parts::add_part(
            db,
            tenant_id,
            ticket_id,
            item.map(|i| i.id),
            item.map(|i| i.name.as_str()).unwrap_or(&p.name),
            p.quantity.max(1),
            "needed",
            "plan",
            None,
            user,
        )
        .await?;
        let url = p.url.as_deref().and_then(|u| clean_url(u).ok().flatten());
        if (item.is_none() && p.unit_cost_cents.is_some()) || url.is_some() {
            let mut am: entity::ticket_part::ActiveModel = added.into();
            if item.is_none() && p.unit_cost_cents.is_some() {
                am.unit_cost_cents = Set(p.unit_cost_cents);
            }
            if let Some(u) = url {
                am.vendor = Set(store_of(&u).map(str::to_string));
                am.url = Set(Some(u));
            }
            am.update(db).await?;
        }
    }
    Ok((tasks.len(), parts.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_costs_use_the_right_rate() {
        let r = Rates {
            in_house: 6_000,
            contractor: 12_000,
        };
        assert_eq!(task_cost(Some(90), false, r), Some(9_000));
        assert_eq!(task_cost(Some(90), true, r), Some(18_000));
        assert_eq!(task_cost(None, true, r), None);
    }

    #[test]
    fn tasks_are_cleaned() {
        let t = clean_tasks(vec![
            KitTask {
                title: "  Hang board ".into(),
                trade: "drywall".into(),
                est_minutes: Some(60),
                needs_contractor: false,
            },
            KitTask {
                title: " ".into(),
                trade: "drywall".into(),
                est_minutes: None,
                needs_contractor: false,
            },
            KitTask {
                title: "Mystery".into(),
                trade: "wizardry".into(),
                est_minutes: Some(-5),
                needs_contractor: true,
            },
        ]);
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].title, "Hang board");
        assert_eq!(t[1].trade, "general");
        assert_eq!(t[1].est_minutes, None);
        assert_eq!(trades_of(&t), vec!["drywall", "general"]);
    }

    #[test]
    fn part_links_name_their_store() {
        assert_eq!(
            store_of("https://www.homedepot.com/p/Moen-1222/100026379"),
            Some("Home Depot")
        );
        assert_eq!(store_of("https://www.amazon.com/dp/B00ABC"), Some("Amazon"));
        assert_eq!(store_of("https://amzn.to/3xyz"), Some("Amazon"));
        assert_eq!(store_of("https://www.lowes.com/pd/x/1000"), Some("Lowe's"));
        assert_eq!(store_of("https://example.com/part"), None);
        assert_eq!(store_of("https://notamazon.com.evil.io/x"), None);
        assert_eq!(clean_url("  ").unwrap(), None);
        assert!(clean_url("javascript:alert(1)").is_err());
        assert!(clean_url("homedepot.com/p/1").is_err());
    }

    #[test]
    fn the_shower_kit_calls_for_a_plumber() {
        let k = KITS.iter().find(|k| k.key == "replace-shower").unwrap();
        assert!(k.tasks.iter().any(|t| t.1 == "plumbing" && t.3));
        assert!(k.tasks.iter().any(|t| t.1 == "drywall"));
        assert!(k.parts.iter().any(|p| p.0.contains("valve")));
        let mut keys = std::collections::HashSet::new();
        for k in KITS {
            assert!(keys.insert(k.key), "duplicate kit key {}", k.key);
            assert!(
                [
                    "plumbing",
                    "electrical",
                    "hvac",
                    "appliance",
                    "structural",
                    "general"
                ]
                .contains(&k.category),
                "{} has category {}",
                k.name,
                k.category
            );
            for t in k.tasks {
                assert!(
                    TRADES.contains(&t.1),
                    "{} uses unknown trade {}",
                    k.name,
                    t.1
                );
            }
        }
    }
}
