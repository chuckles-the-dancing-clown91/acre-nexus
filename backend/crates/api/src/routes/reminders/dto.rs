//! Request/response shapes for the calendar / reminders engine.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct ReminderDto {
    pub id: Uuid,
    pub subject_type: String,
    pub subject_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub due_date: String,
    pub lead_days: Vec<i64>,
    pub recipients: Vec<String>,
    /// Lead times that have already fired.
    pub fired: Vec<i64>,
    pub status: String,
    /// Days until due as of the request (negative = overdue). `None` when the
    /// stored date fails to parse.
    pub days_left: Option<i64>,
    pub completed_at: Option<String>,
    pub created_at: String,
}

impl ReminderDto {
    pub fn from_model(r: entity::reminder::Model, today: chrono::NaiveDate) -> Self {
        let days_left = crate::reminders::days_until(&r.due_date, today);
        ReminderDto {
            id: r.id,
            subject_type: r.subject_type,
            subject_id: r.subject_id,
            title: r.title,
            description: r.description,
            due_date: r.due_date,
            lead_days: r
                .lead_days
                .as_array()
                .map(|a| a.iter().filter_map(|x| x.as_i64()).collect())
                .unwrap_or_default(),
            recipients: r
                .recipients
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
            fired: r
                .fired
                .as_array()
                .map(|a| a.iter().filter_map(|x| x.as_i64()).collect())
                .unwrap_or_default(),
            status: r.status,
            days_left,
            completed_at: r.completed_at.map(|x| x.to_rfc3339()),
            created_at: r.created_at.to_rfc3339(),
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateReminderReq {
    /// `lease` | `license` | `insurance` | `tour` | `inspection` | `custom`.
    pub subject_type: String,
    pub subject_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    /// `YYYY-MM-DD`.
    pub due_date: String,
    /// Days before the due date to notify; defaults to the workspace's
    /// `calendar.default_lead_days` setting.
    pub lead_days: Option<Vec<i64>>,
    /// External recipient email addresses.
    #[serde(default)]
    pub recipients: Vec<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateReminderReq {
    pub title: Option<String>,
    pub description: Option<String>,
    /// `YYYY-MM-DD`. Re-dating an active reminder re-arms its lead times.
    pub due_date: Option<String>,
    pub lead_days: Option<Vec<i64>>,
    pub recipients: Option<Vec<String>>,
    /// `active` | `done` | `cancelled`.
    pub status: Option<String>,
}
