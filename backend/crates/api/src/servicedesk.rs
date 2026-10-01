//! **Service desk** rules: job kits, trades, and estimates.
//!
//! A job kit is an issue-catalog entry with the work broken into tasks (each
//! with its trade, time, and whether it needs a contractor) and the parts it
//! takes, with typical costs. Picking "Shower replacement" opens a work order
//! that already lists the demo, the plumbing, the drywall and the tile, and a
//! shopping list for the valve, cement board and caulk. Routine maintenance can
//! carry a kit too, so every filter change starts with its filter.

use crate::error::ApiResult;
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
// The starter kits
// ---------------------------------------------------------------------------

struct Kit {
    name: &'static str,
    area: &'static str,
    category: &'static str,
    priority: &'static str,
    description: &'static str,
    /// `(title, trade, minutes, needs a contractor)`.
    tasks: &'static [(&'static str, &'static str, i32, bool)],
    /// `(name, quantity, typical unit cost in cents)`.
    parts: &'static [(&'static str, i32, i64)],
}

const KITS: &[Kit] = &[
    Kit {
        name: "Shower replacement",
        area: "Bathroom",
        category: "plumbing",
        priority: "normal",
        description: "Tear out the old shower, check the framing and valve, hang \
                      cement board, waterproof, and set a new surround and trim.",
        tasks: &[
            ("Shut off water and protect the room", "general", 30, false),
            ("Demo old surround and haul debris", "demo", 240, false),
            ("Inspect framing and studs for rot", "carpentry", 45, false),
            (
                "Replace shower valve and supply lines",
                "plumbing",
                180,
                true,
            ),
            (
                "Hang cement board and drywall around the shower",
                "drywall",
                180,
                false,
            ),
            ("Tape, mud and sand the drywall", "drywall", 240, false),
            (
                "Waterproof membrane over the cement board",
                "tile",
                120,
                false,
            ),
            ("Install the new surround or tile", "tile", 480, false),
            ("Set trim, shower head and handle", "plumbing", 60, true),
            ("Caulk, touch up paint and clean", "paint", 120, false),
            ("Pressure test and final walkthrough", "plumbing", 30, true),
        ],
        parts: &[
            ("Shower valve, pressure-balance", 1, 14_900),
            ("Shower trim kit", 1, 12_900),
            ("Shower surround kit", 1, 54_900),
            ("Cement board 3x5", 4, 1_450),
            ("Drywall 4x8 1/2in moisture-resistant", 2, 1_850),
            ("Joint compound", 1, 1_600),
            ("Drywall tape", 1, 600),
            ("Waterproofing membrane", 1, 8_900),
            ("PEX pipe 1/2in, 10 ft", 2, 850),
            ("PEX fittings and crimp rings", 1, 2_400),
            ("Silicone caulk, kitchen and bath", 2, 900),
            ("Cement board screws", 1, 1_100),
            ("Contractor bags", 1, 2_200),
        ],
    },
    Kit {
        name: "Toilet replacement",
        area: "Bathroom",
        category: "plumbing",
        priority: "normal",
        description: "Pull the old toilet, check the flange and subfloor, set a new one.",
        tasks: &[
            ("Shut off and drain the toilet", "plumbing", 15, false),
            ("Remove the old toilet and wax ring", "plumbing", 30, false),
            ("Inspect flange and subfloor", "carpentry", 20, false),
            (
                "Set the new toilet on a new wax ring",
                "plumbing",
                45,
                false,
            ),
            ("Connect supply and test for leaks", "plumbing", 20, false),
            ("Caulk the base and clean up", "cleaning", 15, false),
        ],
        parts: &[
            ("Toilet, elongated 1.28 gpf", 1, 21_900),
            ("Wax ring with sleeve", 1, 800),
            ("Closet bolts", 1, 500),
            ("Toilet supply line 12in", 1, 900),
            ("Silicone caulk, kitchen and bath", 1, 900),
        ],
    },
    Kit {
        name: "Water heater replacement",
        area: "Utility",
        category: "plumbing",
        priority: "high",
        description: "Swap the tank: drain, remove, set the new heater with a pan \
                      and expansion tank, connect, and test.",
        tasks: &[
            (
                "Shut off gas or power and water, drain the tank",
                "plumbing",
                45,
                false,
            ),
            ("Remove and haul the old heater", "demo", 60, false),
            (
                "Set the new heater, pan and expansion tank",
                "plumbing",
                120,
                true,
            ),
            (
                "Connect water, gas or electric, and venting",
                "plumbing",
                90,
                true,
            ),
            ("Fill, purge air, light and test", "plumbing", 30, true),
            ("Check the permit and seismic straps", "general", 20, false),
        ],
        parts: &[
            ("Water heater, 50 gal", 1, 89_900),
            ("Expansion tank", 1, 5_500),
            ("Water heater drain pan", 1, 3_200),
            ("Flex water connector", 2, 1_600),
            ("Seismic strap kit", 1, 3_400),
            ("Gas flex line", 1, 2_800),
        ],
    },
    Kit {
        name: "Drywall patch and paint",
        area: "Any room",
        category: "structural",
        priority: "low",
        description: "Cut out the damage, patch, match the texture and paint.",
        tasks: &[
            ("Cut out the damaged drywall", "drywall", 30, false),
            ("Patch, tape and mud, three coats", "drywall", 180, false),
            ("Sand and match the texture", "drywall", 60, false),
            ("Prime and paint", "paint", 90, false),
        ],
        parts: &[
            ("Drywall 4x8 1/2in", 1, 1_600),
            ("Joint compound", 1, 1_600),
            ("Drywall tape", 1, 600),
            ("Primer, 1 qt", 1, 1_800),
            ("Interior paint, 1 gal", 1, 4_200),
            ("Sanding sponge", 2, 400),
        ],
    },
    Kit {
        name: "Unit turn: interior repaint",
        area: "Whole unit",
        category: "general",
        priority: "normal",
        description: "Repaint a vacant unit: patch, prime, two coats on walls, trim and doors.",
        tasks: &[
            ("Patch nail holes and dings", "drywall", 90, false),
            ("Mask and protect floors", "paint", 60, false),
            ("Prime stains", "paint", 60, false),
            ("Paint walls, two coats", "paint", 600, false),
            ("Paint trim and doors", "paint", 240, false),
            ("Clean up and touch up", "cleaning", 90, false),
        ],
        parts: &[
            ("Interior paint, 5 gal", 1, 17_900),
            ("Primer, 1 gal", 1, 3_200),
            ("Spackle", 1, 900),
            ("Painter's tape", 3, 700),
            ("Drop cloth", 2, 1_500),
            ("Roller cover", 4, 600),
        ],
    },
    Kit {
        name: "HVAC seasonal service",
        area: "Mechanical",
        category: "hvac",
        priority: "normal",
        description: "Spring and fall service: filter, coil, refrigerant, drain line, thermostat.",
        tasks: &[
            ("Replace the air filter", "hvac", 15, false),
            ("Clean the condenser coil", "hvac", 45, false),
            ("Check refrigerant and electrical", "hvac", 45, true),
            ("Flush the condensate line", "hvac", 20, false),
            (
                "Test the thermostat and run a full cycle",
                "hvac",
                20,
                false,
            ),
        ],
        parts: &[
            ("Air filter 16x25x1 MERV 8", 1, 1_200),
            ("Condensate line tablets", 1, 900),
            ("Coil cleaner", 1, 1_400),
        ],
    },
];

/// Add any starter kit the workspace doesn't have yet (by name). Kits a
/// workspace retired stay retired.
pub async fn ensure_kits(db: &impl ConnectionTrait, tenant_id: Uuid) -> ApiResult<()> {
    let have: Vec<String> = IssueTemplate::find()
        .filter(entity::issue_template::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?
        .into_iter()
        .map(|t| t.name.to_lowercase())
        .collect();
    let now = Utc::now();
    for k in KITS {
        if have.contains(&k.name.to_lowercase()) {
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
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

/// A kit's part line as stored.
#[derive(Deserialize, Clone)]
struct StoredPart {
    name: String,
    #[serde(default)]
    quantity: i32,
    inventory_item_id: Option<Uuid>,
    unit_cost_cents: Option<i64>,
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
        if item.is_none() && p.unit_cost_cents.is_some() {
            let mut am: entity::ticket_part::ActiveModel = added.into();
            am.unit_cost_cents = Set(p.unit_cost_cents);
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
    fn the_shower_kit_calls_for_a_plumber() {
        let k = KITS
            .iter()
            .find(|k| k.name == "Shower replacement")
            .unwrap();
        assert!(k.tasks.iter().any(|t| t.1 == "plumbing" && t.3));
        assert!(k.tasks.iter().any(|t| t.1 == "drywall"));
        assert!(k.parts.iter().any(|p| p.0.contains("valve")));
        for k in KITS {
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
