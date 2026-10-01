//! A used single-sign-on assertion id: each signed sign-in token works once.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sso_assertion")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub jti: String,
    pub tenant_id: Option<Uuid>,
    pub issuer: String,
    pub subject: String,
    pub used_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
