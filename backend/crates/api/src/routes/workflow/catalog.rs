//! `GET /workflows/catalog` — every investment strategy and its ordered stage
//! template, independent of any property. Powers the workflow board, which needs
//! the columns for a strategy even when no property currently sits in a stage.

use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use rocket::serde::json::Json;
use rocket::get;
use serde::Serialize;

#[derive(Serialize, schemars::JsonSchema)]
pub struct StageBrief {
    pub key: String,
    pub label: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct StrategyCatalog {
    pub key: String,
    pub label: String,
    pub description: String,
    pub stages: Vec<StageBrief>,
}

/// `GET /workflows/catalog` — the strategy + stage templates.
#[rocket_okapi::openapi(tag = "Workflow")]
#[get("/workflows/catalog")]
pub async fn catalog(user: AuthUser) -> ApiResult<Json<Vec<StrategyCatalog>>> {
    user.require(Permission::PropertyRead)?;
    let out = crate::workflow::STRATEGIES
        .iter()
        .map(|s| StrategyCatalog {
            key: s.key.to_string(),
            label: s.label.to_string(),
            description: s.description.to_string(),
            stages: s
                .stages
                .iter()
                .map(|st| StageBrief {
                    key: st.key.to_string(),
                    label: st.label.to_string(),
                })
                .collect(),
        })
        .collect();
    Ok(Json(out))
}
