//! **Acquisition pipeline** stage catalog (roadmap Phase 7, issue #42).
//!
//! A [`deal`](entity::deal) moves through the buy-side stages before it becomes
//! an owned [`property`](entity::property). Like the [`crate::workflow`] catalog
//! this is code-defined so the set stays coherent and greppable while remaining
//! data the frontend can render generically. Stage order is meaningful (the
//! board renders columns in this order); `dead` is a terminal off-ramp for
//! passed / lost deals and sits outside the linear flow.

/// A stage in the acquisition pipeline: a stable key plus a human label.
pub struct DealStage {
    pub key: &'static str,
    pub label: &'static str,
}

/// The ordered acquisition pipeline. `owned` is the success terminal (the deal
/// has converted into a property); `dead` is the off-ramp.
pub const DEAL_STAGES: &[DealStage] = &[
    DealStage {
        key: "prospecting",
        label: "Prospecting",
    },
    DealStage {
        key: "offer",
        label: "Offer",
    },
    DealStage {
        key: "under_contract",
        label: "Under contract",
    },
    DealStage {
        key: "closing",
        label: "Closing",
    },
    DealStage {
        key: "owned",
        label: "Owned",
    },
    DealStage {
        key: "dead",
        label: "Dead",
    },
];

/// The stage a freshly-created deal starts at.
pub const FIRST_STAGE: &str = "prospecting";

/// Whether `stage` is a valid pipeline stage.
pub fn is_valid_stage(stage: &str) -> bool {
    DEAL_STAGES.iter().any(|s| s.key == stage)
}

/// The human label for a stage key (falls back to the key itself).
pub fn stage_label(stage: &str) -> &str {
    DEAL_STAGES
        .iter()
        .find(|s| s.key == stage)
        .map(|s| s.label)
        .unwrap_or(stage)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_stage_is_valid_and_is_prospecting() {
        assert!(is_valid_stage(FIRST_STAGE));
        assert_eq!(FIRST_STAGE, DEAL_STAGES[0].key);
    }

    #[test]
    fn stage_validation() {
        assert!(is_valid_stage("under_contract"));
        assert!(is_valid_stage("owned"));
        assert!(is_valid_stage("dead"));
        assert!(!is_valid_stage("nonsense"));
        assert!(!is_valid_stage(""));
    }

    #[test]
    fn labels_resolve() {
        assert_eq!(stage_label("under_contract"), "Under contract");
        assert_eq!(stage_label("mystery"), "mystery");
    }
}
