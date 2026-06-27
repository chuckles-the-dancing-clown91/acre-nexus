//! Shapes for the property workflow endpoints + a builder that renders a
//! property's current position against its strategy's stage template.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct StageDto {
    pub key: String,
    pub label: String,
    /// True for stages at or before the current stage.
    pub reached: bool,
    /// True for the current stage.
    pub current: bool,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct WorkflowEventDto {
    pub id: Uuid,
    pub strategy: String,
    pub from_stage: Option<String>,
    pub to_stage: String,
    pub note: Option<String>,
    pub actor_user_id: Option<Uuid>,
    pub created_at: String,
}

impl From<entity::workflow_event::Model> for WorkflowEventDto {
    fn from(e: entity::workflow_event::Model) -> Self {
        WorkflowEventDto {
            id: e.id,
            strategy: e.strategy,
            from_stage: e.from_stage,
            to_stage: e.to_stage,
            note: e.note,
            actor_user_id: e.actor_user_id,
            created_at: e.created_at.to_rfc3339(),
        }
    }
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct WorkflowResp {
    pub strategy: String,
    pub strategy_label: String,
    pub strategy_description: String,
    pub current_stage: String,
    pub stages: Vec<StageDto>,
    pub history: Vec<WorkflowEventDto>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct AdvanceReq {
    /// The stage to move to (must be a valid stage for the property's strategy).
    pub to_stage: String,
    pub note: Option<String>,
}

/// Render a [`WorkflowResp`] from a property's strategy + current stage + history.
pub(crate) fn build(
    strategy_key: &str,
    current_stage: &str,
    history: Vec<entity::workflow_event::Model>,
) -> WorkflowResp {
    let strat = crate::workflow::strategy(strategy_key);
    let strategy_label = strat.map(|s| s.label.to_string()).unwrap_or_default();
    let strategy_description = strat.map(|s| s.description.to_string()).unwrap_or_default();
    let stage_keys: Vec<&str> = strat
        .map(|s| s.stages.iter().map(|st| st.key).collect())
        .unwrap_or_default();
    let current_idx = stage_keys.iter().position(|k| *k == current_stage);

    let stages = strat
        .map(|s| {
            s.stages
                .iter()
                .enumerate()
                .map(|(i, st)| StageDto {
                    key: st.key.to_string(),
                    label: st.label.to_string(),
                    reached: current_idx.map(|ci| i <= ci).unwrap_or(false),
                    current: Some(i) == current_idx,
                })
                .collect()
        })
        .unwrap_or_default();

    WorkflowResp {
        strategy: strategy_key.to_string(),
        strategy_label,
        strategy_description,
        current_stage: current_stage.to_string(),
        stages,
        history: history.into_iter().map(WorkflowEventDto::from).collect(),
    }
}
