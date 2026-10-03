//! The property in words: a description and its features, grouped the way a
//! listing's "Facts and features" are.

use super::{property_in, text};
use crate::auth::AuthUser;
use crate::enrichment::runner::load_or_init_detail;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use rocket::serde::json::Json;
use rocket::{put, State};
use schemars::JsonSchema;
use sea_orm::{ActiveModelTrait, Set};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const GROUPS: &[&str] = &[
    "interior",
    "exterior",
    "construction",
    "utilities",
    "community",
];
const MAX_PER_GROUP: usize = 40;
const MAX_ENTRY: usize = 120;
const MAX_DESCRIPTION: usize = 4000;

#[derive(Deserialize, JsonSchema)]
pub struct StoryReq {
    /// The property in the team's words. Blank clears it.
    pub description: Option<String>,
    /// Entries per group; a group left out is cleared. An entry may read
    /// `Label: value` ("Flooring: Hardwood").
    pub features: Option<BTreeMap<String, Vec<String>>>,
}

#[derive(Serialize, JsonSchema)]
pub struct StoryResp {
    pub description: Option<String>,
    pub features: BTreeMap<String, Vec<String>>,
}

/// Trim, drop blanks and repeats (ignoring case), keep order, and check the
/// groups and sizes.
pub fn clean_features(
    raw: BTreeMap<String, Vec<String>>,
) -> Result<BTreeMap<String, Vec<String>>, String> {
    let mut out = BTreeMap::new();
    for (group, entries) in raw {
        let g = group.trim().to_lowercase();
        if !GROUPS.contains(&g.as_str()) {
            return Err(format!(
                "\"{group}\" isn't a feature group (use {})",
                GROUPS.join(", ")
            ));
        }
        let mut seen = Vec::<String>::new();
        let mut list = Vec::new();
        for e in entries {
            let e = e.split_whitespace().collect::<Vec<_>>().join(" ");
            if e.is_empty() {
                continue;
            }
            if e.chars().count() > MAX_ENTRY {
                return Err(format!("keep each feature under {MAX_ENTRY} characters"));
            }
            if seen.contains(&e.to_lowercase()) {
                continue;
            }
            seen.push(e.to_lowercase());
            list.push(e);
        }
        if list.len() > MAX_PER_GROUP {
            return Err(format!("{g} has more than {MAX_PER_GROUP} features"));
        }
        if !list.is_empty() {
            out.insert(g, list);
        }
    }
    Ok(out)
}

/// `PUT /properties/<id>/story` — set the description and features.
#[rocket_okapi::openapi(tag = "Property Profile")]
#[put("/properties/<id>/story", data = "<body>")]
pub async fn put_story(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<StoryReq>,
) -> ApiResult<Json<StoryResp>> {
    user.require(Permission::PropertyWrite)?;
    let property = property_in(&db, scope.tenant_id, id).await?;
    let b = body.into_inner();
    let detail = load_or_init_detail(&db, &property)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!(e.to_string())))?;
    let mut am: entity::property_detail::ActiveModel = detail.clone().into();
    if let Some(d) = b.description {
        let d = text(Some(d));
        if d.as_deref()
            .is_some_and(|d| d.chars().count() > MAX_DESCRIPTION)
        {
            return Err(ApiError::BadRequest(format!(
                "keep the description under {MAX_DESCRIPTION} characters"
            )));
        }
        am.description = Set(d);
    }
    if let Some(f) = b.features {
        let f = clean_features(f).map_err(ApiError::BadRequest)?;
        am.features = Set(serde_json::to_value(f).unwrap_or_default());
    }
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&db).await?;
    Ok(Json(StoryResp {
        description: saved.description,
        features: serde_json::from_value(saved.features).unwrap_or_default(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(g: &str, v: &[&str]) -> BTreeMap<String, Vec<String>> {
        BTreeMap::from([(g.to_string(), v.iter().map(|s| s.to_string()).collect())])
    }

    #[test]
    fn tidies_entries() {
        let out = clean_features(m(
            " Interior ",
            &[
                " Flooring:   Hardwood ",
                "",
                "flooring: hardwood",
                "Gas fireplace",
            ],
        ))
        .unwrap();
        assert_eq!(out["interior"], ["Flooring: Hardwood", "Gas fireplace"]);
    }

    #[test]
    fn refuses_unknown_groups_and_long_entries() {
        assert!(clean_features(m("garage", &["x"])).is_err());
        assert!(clean_features(m("exterior", &[&"x".repeat(121)])).is_err());
        let many: Vec<String> = (0..41).map(|i| format!("f{i}")).collect();
        let many: Vec<&str> = many.iter().map(String::as_str).collect();
        assert!(clean_features(m("exterior", &many)).is_err());
    }

    #[test]
    fn an_empty_group_is_dropped() {
        assert!(clean_features(m("utilities", &["  "])).unwrap().is_empty());
    }
}
