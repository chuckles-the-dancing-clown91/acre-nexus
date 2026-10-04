//! Lease-document **template renderer**.
//!
//! Turns a tenant's `theme.legal_templates` plus the concrete lease, its charges
//! (fees / discounts / amenities), the resident's attributes (pets), and their
//! vehicles into a finished residential-lease agreement. Interpolation is a small
//! pure `{placeholder}` substitution (no external templating crate — keeps the
//! dependency rule), so the same engine renders both the boilerplate templates and
//! each charge's per-item verbiage.
//!
//! Supported placeholders: `{landlord}`, `{tenant}`, `{property_address}`,
//! `{unit}`, `{rent}`, `{deposit}`, `{monthly_total}`, `{start_date}`,
//! `{end_date}`, `{late_fee}`, `{grace_days}`, `{amount}` (per-charge),
//! `{pet_details}`, `{vehicles}`.

use crate::dto::usd;
use entity::{lease, lease_charge, lease_renewal, property, unit, vehicle};
use std::collections::HashMap;

/// Replace every `{key}` in `template` from `vars`; unknown keys are left intact.
pub fn interpolate(template: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = String::with_capacity(template.len());
    let bytes = template.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            if let Some(end) = template[i + 1..].find('}') {
                let key = &template[i + 1..i + 1 + end];
                if let Some(val) = vars.get(key) {
                    out.push_str(val);
                    i += end + 2;
                    continue;
                }
            }
        }
        out.push(template[i..].chars().next().unwrap());
        i += template[i..].chars().next().unwrap().len_utf8();
    }
    out
}

fn template_str(templates: &serde_json::Value, key: &str) -> Option<String> {
    templates
        .get(key)
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

/// A one-line human description of a vehicle, e.g. "2021 Toyota Tacoma (Silver, plate ABC-1234)".
pub fn describe_vehicle(v: &vehicle::Model) -> String {
    let mut s = String::new();
    if let Some(y) = v.year {
        s.push_str(&format!("{y} "));
    }
    s.push_str(&format!("{} {}", v.make, v.model));
    let mut extras = Vec::new();
    if let Some(c) = &v.color {
        extras.push(c.clone());
    }
    if let Some(p) = &v.license_plate {
        let plate = match &v.plate_state {
            Some(st) => format!("plate {st} {p}"),
            None => format!("plate {p}"),
        };
        extras.push(plate);
    }
    if !extras.is_empty() {
        s.push_str(&format!(" ({})", extras.join(", ")));
    }
    s
}

/// The total recurring monthly amount: base rent plus all recurring charges
/// (discounts/rebates are negative). Not floored — if discounts exceed rent the
/// resident carries a credit, and the printed line items must sum to this total.
pub fn monthly_total_cents(lease: &lease::Model, charges: &[lease_charge::Model]) -> i64 {
    let add: i64 = charges
        .iter()
        .filter(|c| c.recurring)
        .map(|c| c.amount_cents)
        .sum();
    lease.rent_cents + add
}

/// One block of a lease article.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    /// A paragraph.
    P {
        text: String,
    },
    /// Label and value pairs.
    Facts {
        rows: Vec<(String, String)>,
    },
    /// A table; money columns are right-aligned by the reader.
    Table {
        head: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    List {
        items: Vec<String>,
    },
    /// A highlighted remark.
    Note {
        text: String,
    },
}

/// An article or addendum of the lease.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema, PartialEq)]
pub struct Section {
    pub key: String,
    pub title: String,
    /// `article` | `addendum`
    pub kind: String,
    pub blocks: Vec<Block>,
}

/// Everything a lease is written from: the lease, its property and unit, the
/// resident's profile, the utilities, the equipment and the charges.
pub struct LeaseInput<'a> {
    pub templates: &'a serde_json::Value,
    pub lease: &'a lease::Model,
    pub property: &'a property::Model,
    pub unit: Option<&'a unit::Model>,
    pub charges: &'a [lease_charge::Model],
    pub vehicles: &'a [vehicle::Model],
    pub resident: Option<&'a crate::resident::Extras>,
    pub utilities: &'a [crate::utilities::UtilityTerm],
    pub equipment: &'a [entity::asset::Model],
}

pub struct Built {
    pub sections: Vec<Section>,
    pub body: String,
}

fn p(text: impl Into<String>) -> Block {
    Block::P { text: text.into() }
}

fn facts(rows: Vec<(&str, String)>) -> Block {
    Block::Facts {
        rows: rows
            .into_iter()
            .filter(|(_, v)| !v.trim().is_empty())
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    }
}

/// Does a custom addendum apply to this lease?
fn addendum_applies(
    when: &str,
    has_pets: bool,
    has_vehicles: bool,
    built_before_1978: bool,
) -> bool {
    match when {
        "has_pet" => has_pets,
        "has_vehicle" => has_vehicles,
        "built_before_1978" => built_before_1978,
        _ => true,
    }
}

/// Write the lease as articles and addenda, and as plain text for signing.
pub fn build(i: &LeaseInput) -> Built {
    let LeaseInput {
        templates,
        lease,
        property,
        unit,
        charges,
        vehicles,
        resident,
        utilities,
        equipment,
    } = i;
    let landlord = if property.manager.trim().is_empty() {
        "Landlord".to_string()
    } else {
        property.manager.clone()
    };
    let premises = {
        let mut s = format!("{}, {}", property.address, property.city);
        if let Some(u) = unit.filter(|u| u.unit_number != "Home") {
            s.push_str(&format!(", Unit {}", u.unit_number));
        }
        s
    };
    let pets: Vec<crate::resident::Pet> = resident.map(|r| r.pets.clone()).unwrap_or_default();
    let has_pets = lease.has_pet || !pets.is_empty();
    let pet_text = crate::resident::pet_details(&pets)
        .or_else(|| lease.pet_details.clone())
        .unwrap_or_else(|| "as disclosed".into());
    let vehicles_desc = if vehicles.is_empty() {
        "none on file".to_string()
    } else {
        vehicles
            .iter()
            .map(describe_vehicle)
            .collect::<Vec<_>>()
            .join("; ")
    };
    let monthly = monthly_total_cents(lease, charges);
    let end = lease
        .end_date
        .clone()
        .unwrap_or_else(|| "month-to-month".into());

    let mut vars: HashMap<&str, String> = HashMap::new();
    vars.insert("landlord", landlord.clone());
    vars.insert("tenant", lease.tenant_name.clone());
    vars.insert(
        "property_address",
        format!("{}, {}", property.address, property.city),
    );
    vars.insert(
        "unit",
        unit.map(|u| u.unit_number.clone())
            .unwrap_or_else(|| "—".into()),
    );
    vars.insert("rent", usd(lease.rent_cents));
    vars.insert("deposit", usd(lease.deposit_cents.unwrap_or(0)));
    vars.insert("monthly_total", usd(monthly));
    vars.insert("start_date", lease.start_date.clone());
    vars.insert("end_date", end.clone());
    vars.insert("grace_days", "5".into());
    vars.insert("late_fee", usd(5000));
    vars.insert("pet_details", pet_text.clone());
    vars.insert("vehicles", vehicles_desc.clone());

    let mut sections: Vec<Section> = Vec::new();
    let mut art = |key: &str, title: &str, blocks: Vec<Block>| {
        sections.push(Section {
            key: key.into(),
            title: title.into(),
            kind: "article".into(),
            blocks,
        });
    };

    let intro = template_str(templates, "lease_intro")
        .map(|t| interpolate(&t, &vars))
        .unwrap_or_else(|| {
            format!(
                "This Residential Lease Agreement is entered into between {landlord} and {}.",
                lease.tenant_name
            )
        });
    let mut parties = vec![p(intro)];
    let mut fact_rows = vec![
        ("Landlord", landlord.clone()),
        ("Resident", lease.tenant_name.clone()),
        (
            "Resident email",
            lease.tenant_email.clone().unwrap_or_default(),
        ),
        ("Premises", premises.clone()),
    ];
    if let Some(u) = unit {
        let mut d = Vec::new();
        if let Some(b) = u.beds {
            d.push(format!("{b} bed"));
        }
        if let Some(b) = u.baths {
            d.push(format!("{b} bath"));
        }
        if let Some(s) = u.sqft {
            d.push(format!("{s} sq ft"));
        }
        fact_rows.push(("The home", d.join(", ")));
    }
    if property.year_built > 0 {
        fact_rows.push(("Built", property.year_built.to_string()));
    }
    parties.push(facts(fact_rows));
    art("parties", "Parties and premises", parties);

    art(
        "term",
        "Term",
        vec![facts(vec![
            ("Starts", lease.start_date.clone()),
            ("Ends", end.clone()),
        ])],
    );

    let mut rent_rows: Vec<Vec<String>> = vec![vec![
        "Base rent".into(),
        format!("{} / month", usd(lease.rent_cents)),
    ]];
    for c in charges.iter().filter(|c| c.recurring) {
        let sign = if c.amount_cents < 0 { "-" } else { "" };
        rent_rows.push(vec![
            c.label.clone(),
            format!("{sign}{} / month", usd(c.amount_cents.abs())),
        ]);
    }
    rent_rows.push(vec![
        "Total monthly".into(),
        format!("{} / month", usd(monthly)),
    ]);
    let mut rent_blocks = vec![Block::Table {
        head: vec!["Item".into(), "Amount".into()],
        rows: rent_rows,
    }];
    let one_time: Vec<&lease_charge::Model> = charges.iter().filter(|c| !c.recurring).collect();
    if !one_time.is_empty() || lease.deposit_cents.is_some() {
        let mut rows: Vec<Vec<String>> = one_time
            .iter()
            .map(|c| vec![c.label.clone(), usd(c.amount_cents)])
            .collect();
        if let Some(dep) = lease.deposit_cents {
            rows.push(vec!["Security deposit".into(), usd(dep)]);
        }
        rent_blocks.push(Block::Table {
            head: vec!["Due once".into(), "Amount".into()],
            rows,
        });
    }
    art("rent", "Rent and charges", rent_blocks);

    // Utilities: the utility agreement, in the lease itself.
    if !utilities.is_empty() {
        let rows = utilities
            .iter()
            .map(|u| {
                vec![
                    u.label.clone(),
                    u.paid_by_label.clone(),
                    u.provider.clone().unwrap_or_else(|| "—".into()),
                ]
            })
            .collect();
        let mut blocks = vec![
            p("Who pays for each service at the premises:"),
            Block::Table {
                head: vec!["Service".into(), "Paid by".into(), "Provider".into()],
                rows,
            },
        ];
        let notes: Vec<String> = utilities
            .iter()
            .filter_map(|u| u.note.as_ref().map(|n| format!("{}: {n}", u.label)))
            .collect();
        if !notes.is_empty() {
            blocks.push(Block::List { items: notes });
        }
        blocks.push(p("The full terms are in the Utility Agreement addendum."));
        art("utilities", "Utilities and services", blocks);
    }

    if !equipment.is_empty() {
        art(
            "equipment",
            "Appliances and equipment",
            vec![
                p("These stay with the home. Resident keeps them clean and tells the landlord when one stops working."),
                Block::List {
                    items: equipment
                        .iter()
                        .map(|a| match a.make.as_deref().filter(|m| !m.is_empty()) {
                            Some(m) => format!("{} ({m})", a.name),
                            None => a.name.clone(),
                        })
                        .collect(),
                },
            ],
        );
    }

    if let Some(r) = resident {
        let mut blocks = Vec::new();
        if !r.occupants.is_empty() {
            blocks.push(p(
                "Only the resident and these people may live at the premises:",
            ));
            blocks.push(Block::List {
                items: r
                    .occupants
                    .iter()
                    .map(|o| match (&o.relation, o.age) {
                        (Some(rel), Some(a)) => format!("{} ({rel}, {a})", o.name),
                        (Some(rel), None) => format!("{} ({rel})", o.name),
                        (None, Some(a)) => format!("{} ({a})", o.name),
                        _ => o.name.clone(),
                    })
                    .collect(),
            });
        }
        if r.has_emergency_contact() {
            blocks.push(facts(vec![
                (
                    "Emergency contact",
                    r.emergency_contact_name.clone().unwrap_or_default(),
                ),
                (
                    "Relationship",
                    r.emergency_contact_relation.clone().unwrap_or_default(),
                ),
                (
                    "Phone",
                    r.emergency_contact_phone.clone().unwrap_or_default(),
                ),
            ]));
        }
        if !blocks.is_empty() {
            art("household", "Occupants and emergency contact", blocks);
        }
    }

    let with_text: Vec<&lease_charge::Model> =
        charges.iter().filter(|c| c.verbiage.is_some()).collect();
    if !with_text.is_empty() {
        art(
            "additional",
            "Additional terms",
            with_text
                .iter()
                .map(|c| {
                    let mut cvars = vars.clone();
                    cvars.insert("amount", usd(c.amount_cents.abs()));
                    p(interpolate(c.verbiage.as_ref().unwrap(), &cvars))
                })
                .collect(),
        );
    }

    if has_pets {
        art(
            "pets",
            "Pets",
            vec![p(format!(
                "Resident may keep the following pet(s): {pet_text}."
            ))],
        );
    }
    if !vehicles.is_empty() {
        art(
            "vehicles",
            "Vehicles",
            vec![p(format!("Registered vehicle(s): {vehicles_desc}."))],
        );
    }

    art(
        "late",
        "Late payments",
        vec![p(template_str(templates, "late_fee")
            .map(|t| interpolate(&t, &vars))
            .unwrap_or_else(|| {
                "A late fee applies after a 5-day grace period.".into()
            }))],
    );
    if let Some(t) = template_str(templates, "privacy") {
        art("privacy", "Privacy", vec![p(interpolate(&t, &vars))]);
    }

    // ---- addenda ----
    let built_before_1978 = property.year_built > 0 && property.year_built < 1978;
    let mut add = |key: &str, title: &str, blocks: Vec<Block>| {
        sections.push(Section {
            key: key.into(),
            title: title.into(),
            kind: "addendum".into(),
            blocks,
        });
    };
    if !utilities.is_empty() {
        let mut blocks = vec![
            p(format!(
                "This Utility Agreement is part of the lease for {premises}."
            )),
            Block::Table {
                head: vec!["Service".into(), "Paid by".into(), "Meter".into()],
                rows: utilities
                    .iter()
                    .map(|u| {
                        vec![
                            u.label.clone(),
                            u.paid_by_label.clone(),
                            if u.meters.is_empty() {
                                "—".into()
                            } else {
                                u.meters.join(", ")
                            },
                        ]
                    })
                    .collect(),
            },
        ];
        let tenant_paid: Vec<String> = utilities
            .iter()
            .filter(|u| u.paid_by != "landlord")
            .map(|u| u.label.to_lowercase())
            .collect();
        let mut items = Vec::new();
        if !tenant_paid.is_empty() {
            items.push(format!(
                "Resident pays for {} and keeps the account in their name for the whole term.",
                tenant_paid.join(", ")
            ));
            items.push(
                "Resident sets up those accounts so service starts on the move-in date.".into(),
            );
        }
        if utilities.iter().any(|u| u.paid_by == "landlord") {
            items.push("Services the landlord pays for are included in rent. Resident uses them with care and does not waste them.".into());
        }
        if utilities.iter().any(|u| u.paid_by == "shared") {
            items.push("Shared services are split as described in the notes above.".into());
        }
        items.push(
            "The landlord and resident record each meter reading at move-in and move-out.".into(),
        );
        items.push(
            "If a tenant-paid service is shut off for non-payment, that is a breach of the lease."
                .into(),
        );
        blocks.push(Block::List { items });
        add("utility_agreement", "Utility Agreement", blocks);
    }
    if has_pets {
        let mut blocks = vec![p(format!(
            "Resident may keep the following at {premises}: {pet_text}."
        ))];
        if let Some(fee) = charges
            .iter()
            .find(|c| c.label.to_lowercase().contains("pet"))
        {
            blocks.push(p(format!(
                "A pet charge of {} applies ({}).",
                usd(fee.amount_cents.abs()),
                if fee.recurring { "monthly" } else { "one time" }
            )));
        }
        blocks.push(Block::List {
            items: vec![
                "Resident keeps pets under control and on a leash in common areas.".into(),
                "Resident cleans up after pets right away and pays for any damage they cause."
                    .into(),
                "Pets must be kept current on vaccines and licensing.".into(),
                "No other animals may live at the premises without the landlord's written consent."
                    .into(),
            ],
        });
        add("pet_addendum", "Pet Addendum", blocks);
    }
    if built_before_1978 {
        add(
            "lead_paint",
            "Lead-Based Paint Disclosure",
            vec![
                p(format!(
                    "The home was built in {}. Homes built before 1978 may contain lead-based paint, which can be a health hazard, especially to young children and pregnant women.",
                    property.year_built
                )),
                Block::List {
                    items: vec![
                        "The landlord has no knowledge of lead-based paint at the premises, unless noted here.".into(),
                        "The landlord has given the resident the pamphlet Protect Your Family From Lead in Your Home.".into(),
                    ],
                },
            ],
        );
    }
    if let Some(list) = templates.get("addenda").and_then(|v| v.as_array()) {
        for (n, a) in list.iter().enumerate() {
            let title = a
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Addendum");
            let body = a.get("body").and_then(|v| v.as_str()).unwrap_or("");
            let when = a.get("when").and_then(|v| v.as_str()).unwrap_or("always");
            if body.trim().is_empty()
                || !addendum_applies(when, has_pets, !vehicles.is_empty(), built_before_1978)
            {
                continue;
            }
            add(
                &format!("custom_{n}"),
                title,
                body.split("\n\n")
                    .map(|para| p(interpolate(para.trim(), &vars)))
                    .collect(),
            );
        }
    }

    let body = to_text(&sections, &landlord, &lease.tenant_name);
    Built { sections, body }
}

/// The signable plain-text form of the lease.
pub fn to_text(sections: &[Section], landlord: &str, resident: &str) -> String {
    let mut doc = String::new();
    doc.push_str("RESIDENTIAL LEASE AGREEMENT\n");
    doc.push_str("===========================\n\n");
    let (mut n, mut a) = (0, 0);
    for s in sections {
        let head = if s.kind == "addendum" {
            a += 1;
            format!("ADDENDUM {a}: {}", s.title.to_uppercase())
        } else {
            n += 1;
            format!("{n}. {}", s.title.to_uppercase())
        };
        doc.push_str(&head);
        doc.push('\n');
        for b in &s.blocks {
            match b {
                Block::P { text } | Block::Note { text } => {
                    doc.push_str(&format!("   {text}\n"));
                }
                Block::Facts { rows } => {
                    for (k, v) in rows {
                        doc.push_str(&format!("   {k}: {v}\n"));
                    }
                }
                Block::Table { head, rows } => {
                    doc.push_str(&format!("   {}\n", head.join(" | ")));
                    for r in rows {
                        doc.push_str(&format!("   {}\n", r.join(" | ")));
                    }
                }
                Block::List { items } => {
                    for i in items {
                        doc.push_str(&format!("     • {i}\n"));
                    }
                }
            }
        }
        doc.push('\n');
    }
    doc.push_str("SIGNATURES\n");
    doc.push_str(&format!(
        "   Landlord: {landlord} ____________________  Date: __________\n"
    ));
    doc.push_str(&format!(
        "   Resident: {resident} ____________________  Date: __________\n"
    ));
    doc
}

/// A human label for a rent change, e.g. `"+$150.00 / month (+8.3%)"` or
/// `"no change"`. The percentage is omitted when the prior rent is zero.
pub fn rent_change_label(current_cents: i64, new_cents: i64) -> String {
    let delta = new_cents - current_cents;
    if delta == 0 {
        return "no change".to_string();
    }
    let sign = if delta > 0 { "+" } else { "-" };
    let amount = format!("{sign}{} / month", usd(delta.abs()));
    if current_cents > 0 {
        // One decimal place, computed in basis points to avoid float rounding.
        let bps = (delta.abs() * 10_000) / current_cents;
        let whole = bps / 100;
        let frac = (bps % 100) / 10;
        format!("{amount} ({sign}{whole}.{frac}%)")
    } else {
        amount
    }
}

/// Render a **lease renewal addendum** (plain text) — the document a resident
/// e-signs to accept renewed terms (typically a rent increase + extended end
/// date). It modifies, rather than replaces, the original lease agreement.
pub fn render_renewal_addendum(
    lease: &lease::Model,
    property: &property::Model,
    unit: Option<&unit::Model>,
    renewal: &lease_renewal::Model,
) -> String {
    let landlord = if property.manager.trim().is_empty() {
        "Landlord".to_string()
    } else {
        property.manager.clone()
    };
    let premises = {
        let mut s = format!("{}, {}", property.address, property.city);
        if let Some(u) = unit {
            s.push_str(&format!(", Unit {}", u.unit_number));
        }
        s
    };
    let new_end = renewal
        .new_end_date
        .clone()
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| "month-to-month".into());

    let mut doc = String::new();
    doc.push_str("LEASE RENEWAL ADDENDUM\n");
    doc.push_str("======================\n\n");
    doc.push_str(&format!(
        "This Lease Renewal Addendum (\"Addendum\") modifies and extends the \
         Residential Lease Agreement between {landlord} and {} for the premises \
         at {premises}.\n\n",
        lease.tenant_name
    ));

    doc.push_str("1. EXISTING LEASE\n");
    doc.push_str(&format!(
        "   The parties entered into a lease at a monthly rent of {}. All terms \
         of the existing lease remain in full force except as modified below.\n\n",
        usd(renewal.current_rent_cents)
    ));

    doc.push_str("2. RENEWED TERM\n");
    doc.push_str(&format!(
        "   Effective {}, the lease is renewed through {new_end}.\n",
        renewal.new_start_date
    ));
    if let Some(months) = renewal.term_months {
        doc.push_str(&format!("   Renewal term: {months} months.\n"));
    }
    doc.push('\n');

    doc.push_str("3. RENT\n");
    doc.push_str(&format!(
        "   Beginning {}, the monthly rent is {} (previously {}) — {}.\n\n",
        renewal.new_start_date,
        usd(renewal.new_rent_cents),
        usd(renewal.current_rent_cents),
        rent_change_label(renewal.current_rent_cents, renewal.new_rent_cents)
    ));

    doc.push_str("4. ALL OTHER TERMS\n");
    doc.push_str(
        "   Except as expressly modified by this Addendum, every term and \
         condition of the original lease remains unchanged and in effect.\n\n",
    );

    if let Some(notes) = renewal.notes.as_deref().filter(|n| !n.trim().is_empty()) {
        doc.push_str("5. ADDITIONAL NOTES\n");
        doc.push_str(&format!("   {notes}\n\n"));
    }

    doc.push_str("SIGNATURES\n");
    doc.push_str(&format!(
        "   Landlord: {landlord} ____________________  Date: __________\n"
    ));
    doc.push_str(&format!(
        "   Resident: {} ____________________  Date: __________\n",
        lease.tenant_name
    ));

    doc
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_lease() -> lease::Model {
        lease::Model {
            id: uuid::Uuid::nil(),
            tenant_id: uuid::Uuid::nil(),
            property_id: uuid::Uuid::nil(),
            unit_id: None,
            application_id: None,
            tenant_name: "Priya Nair".into(),
            tenant_email: Some("priya@example.com".into()),
            tenant_phone: None,
            rent_cents: 215_000,
            deposit_cents: Some(215_000),
            start_date: "2026-01-01".into(),
            end_date: None,
            status: "draft".into(),
            payment_status: "current".into(),
            balance_cents: 0,
            has_pet: false,
            pet_details: None,
            is_military: false,
            notes: None,
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        }
    }

    fn sample_property(year: i32) -> property::Model {
        property::Model {
            id: uuid::Uuid::nil(),
            tenant_id: uuid::Uuid::nil(),
            llc_id: None,
            portfolio_id: None,
            name: "14 Willow Bend".into(),
            address: "14 Willow Bend".into(),
            city: "Austin, TX".into(),
            units: 1,
            occupied_units: 1,
            monthly_rent_cents: 0,
            status: "Stabilized".into(),
            year_built: year,
            manager: "Theo Grant".into(),
            property_type: "single_family".into(),
            strategy: "rental".into(),
            workflow_stage: "managing".into(),
            purchase_price_cents: None,
            acquired_on: None,
            image_url: None,
            state: String::new(),
            postal_code: String::new(),
            photo_status: "none".into(),
            photo_attempted_at: None,
            photo_error: None,
            created_at: chrono::Utc::now().into(),
        }
    }

    fn term(kind: &str, paid_by: &str) -> crate::utilities::UtilityTerm {
        crate::utilities::UtilityTerm {
            kind: kind.into(),
            label: crate::utilities::kind_label(kind).into(),
            paid_by: paid_by.into(),
            paid_by_label: crate::utilities::payer_words(paid_by).into(),
            provider: Some("City".into()),
            meters: vec![],
            note: None,
        }
    }

    #[test]
    fn a_lease_carries_utilities_pets_and_the_old_house_disclosure() {
        let l = sample_lease();
        let prop = sample_property(1962);
        let extras = crate::resident::Extras {
            pets: vec![crate::resident::Pet {
                name: "Biscuit".into(),
                kind: "dog".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let utilities = vec![term("electric", "tenant"), term("trash", "landlord")];
        let t = serde_json::json!({ "addenda": [
            { "title": "Yard care", "body": "Resident mows for {tenant}.", "when": "always" },
            { "title": "Parking", "body": "Park in the garage.", "when": "has_vehicle" }
        ]});
        let built = build(&LeaseInput {
            templates: &t,
            lease: &l,
            property: &prop,
            unit: None,
            charges: &[],
            vehicles: &[],
            resident: Some(&extras),
            utilities: &utilities,
            equipment: &[],
        });
        let keys: Vec<&str> = built.sections.iter().map(|s| s.key.as_str()).collect();
        for k in [
            "utilities",
            "pets",
            "utility_agreement",
            "pet_addendum",
            "lead_paint",
            "custom_0",
        ] {
            assert!(keys.contains(&k), "{k} missing from {keys:?}");
        }
        assert!(
            !keys.contains(&"custom_1"),
            "the parking addendum needs a vehicle"
        );
        assert!(built.body.contains("ADDENDUM 1: UTILITY AGREEMENT"));
        assert!(built.body.contains("Biscuit (dog)"));
        assert!(built.body.contains("Resident mows for Priya Nair."));
        assert!(built.body.contains("Electricity | Tenant"));
    }

    #[test]
    fn a_new_house_with_no_utilities_or_pets_has_no_addenda() {
        let l = sample_lease();
        let prop = sample_property(2015);
        let t = serde_json::json!({});
        let built = build(&LeaseInput {
            templates: &t,
            lease: &l,
            property: &prop,
            unit: None,
            charges: &[],
            vehicles: &[],
            resident: None,
            utilities: &[],
            equipment: &[],
        });
        assert!(built.sections.iter().all(|s| s.kind == "article"));
        assert!(built.body.contains("SIGNATURES"));
    }

    #[test]
    fn rent_change_label_formats() {
        // +8.3% on a $150 bump over $1800.
        assert_eq!(rent_change_label(180_000, 195_000), "+$150 / month (+8.3%)");
        assert_eq!(rent_change_label(180_000, 180_000), "no change");
        assert_eq!(rent_change_label(200_000, 190_000), "-$100 / month (-5.0%)");
        // No prior rent → percentage omitted.
        assert_eq!(rent_change_label(0, 150_000), "+$1,500 / month");
    }

    #[test]
    fn interpolate_replaces_known_and_keeps_unknown() {
        let mut vars = HashMap::new();
        vars.insert("tenant", "Jordan".to_string());
        let out = interpolate("Hello {tenant}, your {missing} is here", &vars);
        assert_eq!(out, "Hello Jordan, your {missing} is here");
    }

    #[test]
    fn interpolate_handles_amount() {
        let mut vars = HashMap::new();
        vars.insert("amount", "$50.00".to_string());
        assert_eq!(
            interpolate("Pet rent of {amount}.", &vars),
            "Pet rent of $50.00."
        );
    }
}
