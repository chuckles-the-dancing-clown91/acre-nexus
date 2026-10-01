//! A workspace's **business profile**: public contact details, hours, social
//! links, its Google Business Profile place, and how reviews are shown.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "business_profile")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub business_name: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub website: Option<String>,
    pub address: Option<String>,
    pub hours: Option<String>,
    pub description: Option<String>,
    pub facebook_url: Option<String>,
    pub instagram_url: Option<String>,
    pub yelp_url: Option<String>,
    pub nextdoor_url: Option<String>,
    /// The only Google data kept: the place id (and its name, for display).
    pub google_place_id: Option<String>,
    pub google_place_name: Option<String>,
    /// Overrides the generated "write a review" link.
    pub google_review_url: Option<String>,
    pub show_reviews: bool,
    /// Reviews under this many stars are not shown on the site.
    pub min_rating: i32,
    pub max_reviews: i32,
    /// How long fetched reviews stay cached.
    pub refresh_minutes: i32,
    /// Widgets may be embedded on the client's own website.
    pub embed_enabled: bool,
    /// Sites allowed to show the widgets, one `https://host` per line.
    pub embed_origins: Option<String>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
