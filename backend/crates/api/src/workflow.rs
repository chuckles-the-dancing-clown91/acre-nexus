//! Investment **workflow catalog** — the stage templates each strategy follows.
//!
//! Workflows are code-defined (like the RBAC catalog): a strategy maps to an
//! ordered list of stages, and a property tracks its `workflow_stage` through
//! them. Transitions are recorded in `workflow_event` for history. This keeps the
//! set of strategies/stages coherent and greppable while still being data the
//! frontend can render generically.

/// A stage in a workflow: a stable key plus a human label.
pub struct Stage {
    pub key: &'static str,
    pub label: &'static str,
}

/// One investment strategy and its ordered stages.
pub struct Strategy {
    pub key: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub stages: &'static [Stage],
}

const FLIP_STAGES: &[Stage] = &[
    Stage {
        key: "sourcing",
        label: "Sourcing",
    },
    Stage {
        key: "under_contract",
        label: "Under contract",
    },
    Stage {
        key: "rehab",
        label: "Rehab",
    },
    Stage {
        key: "listed",
        label: "Listed",
    },
    Stage {
        key: "sold",
        label: "Sold",
    },
];

const RENTAL_STAGES: &[Stage] = &[
    Stage {
        key: "acquisition",
        label: "Acquisition",
    },
    Stage {
        key: "rehab",
        label: "Rehab / turn",
    },
    Stage {
        key: "stabilize",
        label: "Stabilize",
    },
    Stage {
        key: "leased",
        label: "Leased",
    },
    Stage {
        key: "managing",
        label: "Managing",
    },
];

const BRRRR_STAGES: &[Stage] = &[
    Stage {
        key: "acquisition",
        label: "Acquisition",
    },
    Stage {
        key: "rehab",
        label: "Rehab",
    },
    Stage {
        key: "rent",
        label: "Rent",
    },
    Stage {
        key: "refinance",
        label: "Refinance",
    },
    Stage {
        key: "repeat",
        label: "Repeat",
    },
];

const HOLD_STAGES: &[Stage] = &[
    Stage {
        key: "acquisition",
        label: "Acquisition",
    },
    Stage {
        key: "managing",
        label: "Managing",
    },
];

const WHOLESALE_STAGES: &[Stage] = &[
    Stage {
        key: "sourcing",
        label: "Sourcing",
    },
    Stage {
        key: "under_contract",
        label: "Under contract",
    },
    Stage {
        key: "assigned",
        label: "Assigned",
    },
    Stage {
        key: "closed",
        label: "Closed",
    },
];

/// Every strategy the platform ships.
pub const STRATEGIES: &[Strategy] = &[
    Strategy {
        key: "rental",
        label: "Buy & hold rental",
        description: "Acquire, stabilize, lease, and manage for cash flow.",
        stages: RENTAL_STAGES,
    },
    Strategy {
        key: "flip",
        label: "Fix & flip",
        description: "Acquire, renovate, and resell for profit.",
        stages: FLIP_STAGES,
    },
    Strategy {
        key: "brrrr",
        label: "BRRRR",
        description: "Buy, rehab, rent, refinance, repeat.",
        stages: BRRRR_STAGES,
    },
    Strategy {
        key: "hold",
        label: "Land / long-term hold",
        description: "Acquire and hold with minimal operations.",
        stages: HOLD_STAGES,
    },
    Strategy {
        key: "wholesale",
        label: "Wholesale",
        description: "Contract and assign without taking title.",
        stages: WHOLESALE_STAGES,
    },
];

/// Look up a strategy by key.
pub fn strategy(key: &str) -> Option<&'static Strategy> {
    STRATEGIES.iter().find(|s| s.key == key)
}

/// The first stage of a strategy (where a new property starts).
pub fn first_stage(strategy_key: &str) -> Option<&'static str> {
    strategy(strategy_key).and_then(|s| s.stages.first().map(|st| st.key))
}

/// Whether `stage_key` is a valid stage for `strategy_key`.
pub fn is_valid_stage(strategy_key: &str, stage_key: &str) -> bool {
    strategy(strategy_key)
        .map(|s| s.stages.iter().any(|st| st.key == stage_key))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strategies_have_stages_and_first() {
        for s in STRATEGIES {
            assert!(!s.stages.is_empty(), "{} has no stages", s.key);
            assert_eq!(first_stage(s.key), Some(s.stages[0].key));
        }
    }

    #[test]
    fn stage_validation() {
        assert!(is_valid_stage("flip", "rehab"));
        assert!(!is_valid_stage("flip", "leased"));
        assert!(!is_valid_stage("nonsense", "rehab"));
    }
}
