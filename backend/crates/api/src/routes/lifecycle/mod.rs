//! Tenant lifecycle routes (roadmap Phase 5, issue #9): move-in / move-out
//! **inspections** with a condition checklist (photos ride the document
//! service, `owner_type = "inspection"`), and the **security-deposit
//! disposition** at move-out (itemized deductions → refund via the payments
//! provider → statement PDF filed on the lease). Mounted by the `rentals`
//! module; reads gate on `lease:read`, writes on `lease:manage`, and the
//! money-moving finalize on `payout:manage`.

pub mod deposits;
pub mod dto;
pub mod inspections;

/// The default move-in / move-out checklist, generated per inspection as
/// editable rows: `(area, item)`.
pub const DEFAULT_CHECKLIST: &[(&str, &str)] = &[
    ("Entry & living areas", "Doors, locks & hardware"),
    ("Entry & living areas", "Walls, ceiling & trim"),
    ("Entry & living areas", "Flooring / carpet"),
    ("Entry & living areas", "Windows, screens & blinds"),
    ("Entry & living areas", "Lighting & switches"),
    ("Kitchen", "Cabinets & countertops"),
    ("Kitchen", "Sink & faucet"),
    ("Kitchen", "Appliances (range, fridge, dishwasher)"),
    ("Kitchen", "Walls & flooring"),
    ("Bathrooms", "Toilet, tub & shower"),
    ("Bathrooms", "Sink, vanity & mirror"),
    ("Bathrooms", "Caulking & ventilation"),
    ("Bedrooms", "Walls, ceiling & trim"),
    ("Bedrooms", "Flooring / carpet"),
    ("Bedrooms", "Closets & doors"),
    ("Systems & safety", "Smoke / CO detectors"),
    ("Systems & safety", "HVAC (heating & cooling)"),
    ("Systems & safety", "Water heater & plumbing"),
    ("Systems & safety", "Electrical panel & outlets"),
    ("Exterior", "Entry, porch & railings"),
    ("Exterior", "Yard / landscaping (if applicable)"),
];

/// Inspection item conditions the API accepts.
pub const CONDITIONS: &[&str] = &["unrated", "good", "fair", "poor", "damaged"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_checklist_is_nonempty_and_grouped() {
        assert!(DEFAULT_CHECKLIST.len() >= 15);
        // Every area appears in contiguous runs (stable display grouping).
        let areas: Vec<&str> = DEFAULT_CHECKLIST.iter().map(|(a, _)| *a).collect();
        let mut seen = Vec::new();
        for area in areas {
            if seen.last() != Some(&area) {
                assert!(
                    !seen.contains(&area),
                    "area {area} appears twice non-contiguously"
                );
                seen.push(area);
            }
        }
    }
}
