//! The records that make up a full property profile beyond the building
//! itself: permits, insurance policies, the schools it's zoned for, and
//! action items, plus the "needs attention" list that turns what's on file
//! into to-dos.
//!
//! Every route sits under `/properties/<id>/…`, so the property-reach gate
//! covers it; handlers check that a child record belongs to that property.

pub mod action_items;
pub mod attention;
pub mod insurance;
pub mod permits;
pub mod schools;

use crate::error::{ApiError, ApiResult};
use chrono::NaiveDate;
use entity::prelude::{Document, Property};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter};
use uuid::Uuid;

pub(crate) fn parse_id(id: &str) -> ApiResult<Uuid> {
    Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))
}

/// The property, in this workspace.
pub(crate) async fn property_in<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    id: &str,
) -> ApiResult<entity::property::Model> {
    Property::find_by_id(parse_id(id)?)
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))
}

/// Trimmed text, or nothing for blank.
pub(crate) fn text(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// A `YYYY-MM-DD` date, or nothing for blank.
pub(crate) fn date(label: &str, v: Option<String>) -> ApiResult<Option<String>> {
    match text(v) {
        None => Ok(None),
        Some(d) => NaiveDate::parse_from_str(&d, "%Y-%m-%d")
            .map(|_| Some(d))
            .map_err(|_| ApiError::BadRequest(format!("{label} must be a date (YYYY-MM-DD)"))),
    }
}

/// One of a fixed set, or the default when blank.
pub(crate) fn one_of(
    label: &str,
    v: Option<String>,
    allowed: &[&str],
    default: &str,
) -> ApiResult<String> {
    match text(v).map(|s| s.to_lowercase()) {
        None => Ok(default.to_string()),
        Some(s) if allowed.contains(&s.as_str()) => Ok(s),
        Some(s) => Err(ApiError::BadRequest(format!(
            "{label} \"{s}\" isn't one of: {}",
            allowed.join(", ")
        ))),
    }
}

/// A money amount that can't be negative.
pub(crate) fn cents(label: &str, v: Option<i64>) -> ApiResult<Option<i64>> {
    match v {
        Some(c) if c < 0 => Err(ApiError::BadRequest(format!("{label} can't be negative"))),
        v => Ok(v),
    }
}

/// Files attached to a record must be this property's documents.
pub(crate) async fn property_documents<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    property_id: Uuid,
    ids: &[Uuid],
) -> ApiResult<serde_json::Value> {
    let mut ids = ids.to_vec();
    ids.sort();
    ids.dedup();
    if !ids.is_empty() {
        let found = Document::find()
            .filter(entity::document::Column::TenantId.eq(tenant_id))
            .filter(entity::document::Column::OwnerType.eq("property"))
            .filter(entity::document::Column::OwnerId.eq(property_id))
            .filter(entity::document::Column::Id.is_in(ids.clone()))
            .count(db)
            .await?;
        if found as usize != ids.len() {
            return Err(ApiError::BadRequest(
                "attach files uploaded to this property".into(),
            ));
        }
    }
    Ok(serde_json::json!(ids))
}

pub(crate) fn ids_of(v: &serde_json::Value) -> Vec<Uuid> {
    serde_json::from_value(v.clone()).unwrap_or_default()
}

pub(crate) fn money(c: Option<i64>) -> Option<String> {
    c.map(crate::dto::usd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_inputs() {
        assert_eq!(text(Some("  ".into())), None);
        assert_eq!(text(Some(" A ".into())), Some("A".into()));
        assert!(date("d", Some("2026-02-30".into())).is_err());
        assert_eq!(
            date("d", Some("2026-02-28".into())).unwrap(),
            Some("2026-02-28".into())
        );
        assert_eq!(one_of("k", None, &["a", "b"], "a").unwrap(), "a");
        assert_eq!(
            one_of("k", Some(" B ".into()), &["a", "b"], "a").unwrap(),
            "b"
        );
        assert!(one_of("k", Some("c".into()), &["a", "b"], "a").is_err());
        assert!(cents("fee", Some(-1)).is_err());
    }
}
