//! One drawn thing on a [`super::site_map`]: a building, unit, campsite,
//! amenity, road, boundary, parking area, water body, or label. Geometry is
//! GeoJSON in WGS84 `[lng, lat]`.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, DeriveEntityModel)]
#[sea_orm(table_name = "site_feature")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub map_id: Uuid,
    /// `building` | `unit` | `site` | `amenity` | `road` | `boundary` | `parking` | `water` | `label`.
    pub kind: String,
    pub name: Option<String>,
    pub geometry: Json,
    /// The unit record this polygon stands for (a `unit` feature).
    pub unit_id: Option<Uuid>,
    /// Kind-specific attributes (a campsite's power, length, rates, ...).
    pub attrs: Json,
    pub position: i32,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
