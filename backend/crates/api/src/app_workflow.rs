//! **Application workflow catalog** — the pipeline a rental application moves
//! through, and the legal transitions between stages.
//!
//! Like the property [`crate::workflow`] catalog, this is code-defined: the
//! application's `status` string *is* its stage, and every change is recorded in
//! `application_event`. The stage keys are the existing status strings
//! (`New` / `Screening` / `Approved` / `Leased`, plus the `Declined` / `Withdrawn`
//! off-ramps) so this layers a validated state machine on top of the data the
//! rest of the code (e.g. application→lease conversion) already uses.

/// A stage in the applications pipeline.
pub struct StageDef {
    pub key: &'static str,
    pub label: &'static str,
    /// A terminal stage has no forward progress on the main path.
    pub terminal: bool,
}

/// The ordered main pipeline.
pub const STAGES: &[StageDef] = &[
    StageDef {
        key: "New",
        label: "New",
        terminal: false,
    },
    StageDef {
        key: "Screening",
        label: "Screening",
        terminal: false,
    },
    StageDef {
        key: "Approved",
        label: "Approved",
        terminal: false,
    },
    StageDef {
        key: "Leased",
        label: "Leased",
        terminal: true,
    },
];

/// Off-ramp stages an application can end (or pause) at, off the main path.
pub const OFFRAMPS: &[StageDef] = &[
    StageDef {
        key: "Declined",
        label: "Declined",
        terminal: true,
    },
    StageDef {
        key: "Withdrawn",
        label: "Withdrawn",
        terminal: true,
    },
];

/// The stages a given status may transition to.
pub fn allowed_transitions(from: &str) -> &'static [&'static str] {
    match from {
        "New" => &["Screening", "Approved", "Declined", "Withdrawn"],
        "Screening" => &["Approved", "Declined", "Withdrawn"],
        "Approved" => &["Leased", "Declined", "Withdrawn"],
        // Off-ramps can be re-opened back into screening.
        "Declined" | "Withdrawn" => &["Screening"],
        // Leased is terminal.
        _ => &[],
    }
}

/// Whether `to` is a recognized stage at all (main or off-ramp).
pub fn is_known_stage(key: &str) -> bool {
    STAGES.iter().chain(OFFRAMPS).any(|s| s.key == key)
}

/// Whether moving `from` → `to` is a legal transition.
pub fn is_valid_transition(from: &str, to: &str) -> bool {
    allowed_transitions(from).contains(&to)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_stages() {
        assert!(is_known_stage("New"));
        assert!(is_known_stage("Leased"));
        assert!(is_known_stage("Withdrawn"));
        assert!(!is_known_stage("Nonsense"));
    }

    #[test]
    fn transitions() {
        assert!(is_valid_transition("Screening", "Approved"));
        assert!(is_valid_transition("Approved", "Leased"));
        assert!(is_valid_transition("Declined", "Screening")); // reopen
        assert!(!is_valid_transition("Leased", "Approved")); // terminal
        assert!(!is_valid_transition("Screening", "Leased")); // must approve first
        assert!(!is_valid_transition("New", "New")); // no self-loop
    }
}
