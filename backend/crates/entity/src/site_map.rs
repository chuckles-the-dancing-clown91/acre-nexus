//! A **site map**: one property laid out on a map (an apartment complex, a
//! campground). Its drawn things are [`super::site_feature`]s.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, DeriveEntityModel)]
#[sea_orm(table_name = "site_map")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub property_id: Uuid,
    pub name: String,
    /// `apartment` | `campground` | `rv_park` | `other`.
    pub kind: String,
    /// `satellite` | `streets` | `plan` | `grid`.
    pub base_layer: String,
    pub center_lng: Option<f64>,
    pub center_lat: Option<f64>,
    pub zoom: f64,
    /// The uploaded plan image (a document with owner type `site_map`).
    pub plan_document_id: Option<Uuid>,
    /// Four `[lng, lat]` corners, clockwise from the image's top-left.
    pub plan_corners: Option<Json>,
    /// Shown on the public site.
    pub published: bool,
    pub notes: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
