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

/// Render the full lease agreement body (plain text).
pub fn render(
    templates: &serde_json::Value,
    lease: &lease::Model,
    property: &property::Model,
    unit: Option<&unit::Model>,
    charges: &[lease_charge::Model],
    vehicles: &[vehicle::Model],
) -> String {
    let landlord = if property.manager.trim().is_empty() {
        "Landlord".to_string()
    } else {
        property.manager.clone()
    };
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

    // Base interpolation vars shared by the boilerplate templates.
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
    vars.insert(
        "end_date",
        lease
            .end_date
            .clone()
            .unwrap_or_else(|| "month-to-month".into()),
    );
    vars.insert("grace_days", "5".into());
    vars.insert("late_fee", usd(5000));
    vars.insert(
        "pet_details",
        lease.pet_details.clone().unwrap_or_else(|| "N/A".into()),
    );
    vars.insert("vehicles", vehicles_desc.clone());

    let mut doc = String::new();
    doc.push_str("RESIDENTIAL LEASE AGREEMENT\n");
    doc.push_str("===========================\n\n");

    if let Some(intro) = template_str(templates, "lease_intro") {
        doc.push_str(&interpolate(&intro, &vars));
        doc.push_str("\n\n");
    } else {
        doc.push_str(&format!(
            "This Residential Lease Agreement is entered into between {landlord} and {}.\n\n",
            lease.tenant_name
        ));
    }

    doc.push_str("1. PARTIES & PREMISES\n");
    doc.push_str(&format!("   Landlord: {landlord}\n"));
    doc.push_str(&format!("   Resident: {}\n", lease.tenant_name));
    if let Some(email) = &lease.tenant_email {
        doc.push_str(&format!("   Resident email: {email}\n"));
    }
    doc.push_str(&format!(
        "   Premises: {}, {}",
        property.address, property.city
    ));
    if let Some(u) = unit {
        doc.push_str(&format!(", Unit {}", u.unit_number));
    }
    doc.push_str("\n\n");

    doc.push_str("2. TERM\n");
    doc.push_str(&format!(
        "   Start: {}    End: {}\n\n",
        lease.start_date,
        lease
            .end_date
            .clone()
            .unwrap_or_else(|| "month-to-month".into())
    ));

    doc.push_str("3. RENT & CHARGES\n");
    doc.push_str(&format!(
        "   Base rent: {} / month\n",
        usd(lease.rent_cents)
    ));
    for c in charges.iter().filter(|c| c.recurring) {
        let sign = if c.amount_cents < 0 { "-" } else { "+" };
        doc.push_str(&format!(
            "   {} {}: {}{} / month\n",
            sign,
            c.label,
            sign,
            usd(c.amount_cents.abs())
        ));
    }
    doc.push_str(&format!("   = Total monthly: {}\n", usd(monthly)));
    let one_time: Vec<&lease_charge::Model> = charges.iter().filter(|c| !c.recurring).collect();
    if !one_time.is_empty() {
        doc.push_str("   One-time charges:\n");
        for c in one_time {
            doc.push_str(&format!("     • {}: {}\n", c.label, usd(c.amount_cents)));
        }
    }
    if let Some(dep) = lease.deposit_cents {
        doc.push_str(&format!("   Security deposit: {}\n", usd(dep)));
    }
    doc.push('\n');

    // Per-charge verbiage (interpolating the charge's own amount + shared vars).
    let charges_with_verbiage: Vec<&lease_charge::Model> =
        charges.iter().filter(|c| c.verbiage.is_some()).collect();
    if !charges_with_verbiage.is_empty() {
        doc.push_str("4. ADDITIONAL TERMS\n");
        for (n, c) in charges_with_verbiage.iter().enumerate() {
            let mut cvars = vars.clone();
            cvars.insert("amount", usd(c.amount_cents.abs()));
            let text = interpolate(c.verbiage.as_ref().unwrap(), &cvars);
            doc.push_str(&format!("   4.{}. {}\n", n + 1, text));
        }
        doc.push('\n');
    }

    if lease.has_pet {
        doc.push_str("5. PETS\n");
        doc.push_str(&format!(
            "   Resident is permitted the following pet(s): {}.\n\n",
            lease
                .pet_details
                .clone()
                .unwrap_or_else(|| "as disclosed".into())
        ));
    }

    if !vehicles.is_empty() {
        doc.push_str("6. VEHICLES\n");
        doc.push_str(&format!("   Registered vehicle(s): {vehicles_desc}.\n\n"));
    }

    doc.push_str("7. LATE PAYMENTS\n");
    if let Some(late) = template_str(templates, "late_fee") {
        doc.push_str(&format!("   {}\n\n", interpolate(&late, &vars)));
    } else {
        doc.push_str("   A late fee applies after a 5-day grace period.\n\n");
    }

    if let Some(privacy) = template_str(templates, "privacy") {
        doc.push_str("8. PRIVACY\n");
        doc.push_str(&format!("   {}\n\n", interpolate(&privacy, &vars)));
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
