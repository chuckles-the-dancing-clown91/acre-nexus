//! A **portfolio** is a logical grouping of properties — by investor, strategy,
//! or region — feeding the investor/flip workflows. A property points at an
//! optional `portfolio_id`; grouping is orthogonal to which legal entity holds
//! title (that is `property.llc_id`).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "portfolio")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    /// Free-form strategy/grouping label (e.g. `flip`, `cashflow`, `pacific-nw`).
    pub strategy: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
